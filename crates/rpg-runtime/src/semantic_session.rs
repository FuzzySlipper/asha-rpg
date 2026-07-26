use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rpg_compiler::{CompiledPlayBundle, CompiledRpgRules};
use rpg_core::{
    DeterministicRandomStream, RpgCapabilityState, RpgIntent, RpgIntentCellTarget,
    RpgModifierTurnChange, RpgRandomEvidence, RpgReactionDecision, RpgReactionRequest,
    RpgResolutionContext, RpgResolutionReceipt, RpgResolutionRejection, RpgTraceStep,
};
use rpg_ir::{
    CompiledPlayBundleArtifact, RpgIrLineOfEffectRequirement, RpgIrTargetKind,
    RulesetActivationBudgetResetBoundary,
};
use serde::{Deserialize, Serialize};

use crate::encounter::{
    action_view, area_projection, build_encounter, encounter_outcome, line_of_effect_projection,
    living_intent_rejection, movement_paths, participant_view, random_failure,
    runtime_board_rejection, RpgActionOptionBindingView, RpgActionOptionsView, RpgActionProposal,
    RpgAreaActionProposal, RpgAreaBoardIndex, RpgAreaOptionView, RpgAreaSubmissionResult,
    RpgBoundActionProposal, RpgBoundActionSubmissionResult, RpgEncounterAuthority,
    RpgEncounterOutcomeView, RpgEncounterView, RpgFilteredCellOptionView,
    RpgFilteredParticipantOptionView, RpgParticipantProjectionCatalogs, RpgRandomSource,
    RpgRandomSourceFailure, RpgReactionProposal, RpgScenario, RpgScenarioFailure,
    RpgSchemaIdentity, RpgTurnControl, RpgTurnControlProposal, RpgTurnControlView,
    RPG_ENCOUNTER_VIEW_SCHEMA_ID, RPG_ENCOUNTER_VIEW_SCHEMA_VERSION,
};
use crate::{RpgReplayEntry, RpgReplayFailure};

const MAXIMUM_AUTOMATIC_RANDOM_REQUESTS: usize = 64;
const MAXIMUM_AUTOMATIC_RANDOM_VALUES: usize = 4_096;
static NEXT_SESSION_BINDING_ID: AtomicU64 = AtomicU64::new(1);
static SESSION_BINDING_PROCESS_NONCE: OnceLock<u128> = OnceLock::new();

fn next_session_binding_id() -> String {
    let process_nonce = SESSION_BINDING_PROCESS_NONCE.get_or_init(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos();
        timestamp ^ (u128::from(std::process::id()) << 96)
    });
    let sequence = NEXT_SESSION_BINDING_ID.fetch_add(1, Ordering::Relaxed);
    format!("rpg-session-{process_nonce:032x}-{sequence:016x}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAuthorityCommand {
    pub expected_revision: u64,
    pub intent: RpgIntent,
    pub random_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionCommand {
    pub expected_revision: u64,
    pub reaction_id: String,
    pub option_id: Option<String>,
    pub additional_random_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnControlCommand {
    pub expected_revision: u64,
    pub actor_id: String,
    pub control: RpgTurnControl,
    pub random_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnControlReceipt {
    pub control: RpgTurnControl,
    pub actor_id: String,
    pub events: Vec<rpg_core::RpgDomainEvent>,
    pub random_evidence: Vec<rpg_core::RpgRandomEvidence>,
    pub random_consumed: u64,
    pub state_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgEffectSaveCandidate {
    pub target_id: String,
    pub instance_id: String,
    pub definition_id: String,
    pub definition_version: u32,
    pub source_entity_id: String,
    pub request: rpg_core::RpgRandomRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPendingTurnSave {
    pub expected_revision: u64,
    pub actor_id: String,
    pub control: RpgTurnControl,
    pub candidates: Vec<RpgEffectSaveCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPendingReaction {
    pub expected_revision: u64,
    pub request: RpgReactionRequest,
    pub trace: Vec<RpgTraceStep>,
    pub random_evidence: Vec<RpgRandomEvidence>,
    pub random_attempted: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", content = "result", rename_all = "camelCase")]
pub enum RpgCommandOutcome {
    Accepted(RpgResolutionReceipt),
    ControlAccepted(RpgTurnControlReceipt),
    AwaitingReaction(RpgPendingReaction),
    AwaitingTurnSave(RpgPendingTurnSave),
    Rejected(RpgResolutionRejection),
}

#[derive(Debug)]
pub enum RpgAutomaticCommandFailure {
    RandomSource(RpgRandomSourceFailure),
    Replay(RpgReplayFailure),
}

impl fmt::Display for RpgAutomaticCommandFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RandomSource(failure) => failure.fmt(formatter),
            Self::Replay(failure) => failure.fmt(formatter),
        }
    }
}

impl std::error::Error for RpgAutomaticCommandFailure {}

#[derive(Debug, Clone)]
pub(crate) struct PendingTransaction {
    pub(crate) expected_revision: u64,
    pub(crate) intent: RpgIntent,
    pub(crate) portable_intent: RpgIntent,
    pub(crate) random_values: Vec<u32>,
    pub(crate) pending: RpgPendingReaction,
    pub(crate) area_event: Option<rpg_core::RpgDomainEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedAreaCommand {
    pub(crate) resolved_intent: RpgIntent,
    pub(crate) event: rpg_core::RpgDomainEvent,
}

/// Owner of one compiled artifact's persistent capability state and staged
/// reaction transaction.
#[derive(Debug)]
pub struct RpgAuthoritySession {
    pub(crate) artifact: Option<CompiledPlayBundleArtifact>,
    pub(crate) rules: CompiledRpgRules,
    pub(crate) state: RpgCapabilityState,
    pub(crate) pending: Option<PendingTransaction>,
    pub(crate) pending_turn_save: Option<RpgPendingTurnSave>,
    pub(crate) accepted_random_values: u64,
    pub(crate) encounter: RpgEncounterAuthority,
    pub(crate) session_binding_id: String,
    pub(crate) scenario_fingerprint: rpg_core::StateFingerprint,
}

impl Clone for RpgAuthoritySession {
    fn clone(&self) -> Self {
        let mut clone = self.probe_snapshot();
        clone.session_binding_id = next_session_binding_id();
        clone
    }
}

impl RpgAuthoritySession {
    fn probe_snapshot(&self) -> Self {
        Self {
            artifact: self.artifact.clone(),
            rules: self.rules.clone(),
            state: self.state.clone(),
            pending: self.pending.clone(),
            pending_turn_save: self.pending_turn_save.clone(),
            accepted_random_values: self.accepted_random_values,
            encounter: self.encounter.clone(),
            session_binding_id: self.session_binding_id.clone(),
            scenario_fingerprint: self.scenario_fingerprint.clone(),
        }
    }

    pub fn from_scenario(
        bundle: CompiledPlayBundle,
        scenario: RpgScenario,
    ) -> Result<Self, RpgScenarioFailure> {
        let scenario_fingerprint = crate::replay::scenario_fingerprint(&scenario)
            .expect("validated scenarios have a canonical fingerprint");
        let (state, encounter) = build_encounter(&bundle, scenario)?;
        Ok(Self {
            artifact: Some(bundle.artifact().clone()),
            rules: bundle.rules().clone(),
            state,
            pending: None,
            pending_turn_save: None,
            accepted_random_values: 0,
            encounter,
            session_binding_id: next_session_binding_id(),
            scenario_fingerprint,
        })
    }

    pub fn session_binding_id(&self) -> &str {
        &self.session_binding_id
    }

    pub fn artifact(&self) -> Option<&CompiledPlayBundleArtifact> {
        self.artifact.as_ref()
    }

    pub fn rules(&self) -> &CompiledRpgRules {
        &self.rules
    }

    pub fn state(&self) -> &RpgCapabilityState {
        &self.state
    }

    pub fn pending_reaction(&self) -> Option<&RpgPendingReaction> {
        self.pending
            .as_ref()
            .map(|transaction| &transaction.pending)
    }

    pub fn accepted_random_values(&self) -> u64 {
        self.accepted_random_values
    }

    pub fn scenario(&self) -> &RpgScenario {
        &self.encounter.scenario
    }

    pub fn turn(&self) -> &crate::RpgTurnState {
        &self.encounter.turn
    }

    fn item_bindings_for_actor(
        &self,
        actor_id: &str,
        action_id: &str,
        item_definition_id: &str,
    ) -> Vec<rpg_core::RpgIntentItemBinding> {
        let Some(requirement) = self.rules.binding_requirement(action_id) else {
            return Vec::new();
        };
        let Some(participant) = self
            .encounter
            .scenario
            .participants
            .iter()
            .find(|participant| participant.id == actor_id)
        else {
            return Vec::new();
        };
        participant
            .equipment
            .iter()
            .filter(|equipment| {
                requirement
                    .slot_ids
                    .binary_search(&equipment.slot_id)
                    .is_ok()
            })
            .filter_map(|equipment| {
                participant
                    .items
                    .iter()
                    .find(|item| {
                        item.id == equipment.item_instance_id
                            && item.definition_id == item_definition_id
                    })
                    .map(|item| rpg_core::RpgIntentItemBinding {
                        binding_id: requirement.id.clone(),
                        item_instance_id: item.id.clone(),
                        item_definition_id: item.definition_id.clone(),
                        slot_id: equipment.slot_id.clone(),
                    })
            })
            .collect()
    }

    pub fn encounter_view(&self) -> RpgEncounterView {
        let actor_id = self.encounter.current_actor_id();
        let area_board_index = RpgAreaBoardIndex::new(&self.encounter.scenario.board, &self.state);
        let action_definitions = self
            .encounter
            .participant_definitions
            .get(actor_id)
            .cloned()
            .unwrap_or_default();
        let actions = self
            .rules
            .actions()
            .filter(|action| action_definitions.contains(&action.id))
            .flat_map(|action| {
                let Some(compiled_binding) = &action.binding else {
                    return vec![(action, None, None, None)];
                };
                let bindings = self
                    .item_bindings_for_actor(
                        actor_id,
                        &action.id,
                        &compiled_binding.item_definition_id,
                    )
                    .into_iter()
                    .map(|binding| {
                        let label = self
                            .encounter
                            .item_definitions
                            .get(&binding.item_definition_id)
                            .map(|item| item.label.clone());
                        (action.clone(), Some(binding), label, None)
                    })
                    .collect::<Vec<_>>();
                if bindings.is_empty() {
                    vec![(
                        action,
                        None,
                        None,
                        Some(rejection(
                            "RPG_ACTION_ITEM_BINDING_UNAVAILABLE",
                            "$.action.itemBinding",
                            "the action requires a compatible equipped item",
                        )),
                    )]
                } else {
                    bindings
                }
            })
            .map(|(action, item_binding, item_label, binding_unavailable)| {
                let actor_intent = RpgIntent {
                    action_id: action.id.clone(),
                    actor_id: actor_id.to_owned(),
                    target_ids: Vec::new(),
                    cell_targets: Vec::new(),
                    item_binding: item_binding.clone(),
                };
                let mut first_rejection = binding_unavailable
                    .or_else(|| living_intent_rejection(&self.state, &actor_intent));
                let target_kind = action.targets.kind;
                let option_binding = RpgActionOptionBindingView {
                    session_binding_id: self.session_binding_id.clone(),
                    artifact_id: self
                        .artifact
                        .as_ref()
                        .map(|artifact| artifact.artifact_id.clone())
                        .unwrap_or_default(),
                    scenario_fingerprint: self.scenario_fingerprint.clone(),
                    authority_revision: self.state.revision(),
                    round: self.encounter.turn.round,
                    turn: self.encounter.turn.turn,
                    current_actor_id: actor_id.to_owned(),
                    action_id: action.id.clone(),
                    item_binding: item_binding.clone(),
                };
                let mut options = match target_kind {
                    RpgIrTargetKind::Participant => {
                        let mut filtered_participants = Vec::new();
                        let legal_candidates = self
                            .rules
                            .candidate_ids_for_binding(
                                &self.state,
                                actor_id,
                                &action.id,
                                item_binding
                                    .as_ref()
                                    .map(|binding| binding.item_definition_id.as_str()),
                            )
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|target_id| {
                                if action.targets.line_of_effect
                                    == RpgIrLineOfEffectRequirement::Required
                                {
                                    let Some(actor) = self.state.entity(actor_id) else {
                                        return false;
                                    };
                                    let Some(target) = self.state.entity(target_id) else {
                                        return false;
                                    };
                                    let projection = line_of_effect_projection(
                                        &area_board_index,
                                        actor.position(),
                                        target.position(),
                                    );
                                    if !projection.clear {
                                        filtered_participants.push(
                                            RpgFilteredParticipantOptionView {
                                                participant_id: target_id.clone(),
                                                reason: projection
                                                    .reason
                                                    .unwrap_or("lineOfEffectBlocked")
                                                    .to_owned(),
                                                blocking_cell_ids: projection.blocking_cell_ids,
                                            },
                                        );
                                        return false;
                                    }
                                }
                                let intent = RpgIntent {
                                    action_id: action.id.clone(),
                                    actor_id: actor_id.to_owned(),
                                    target_ids: vec![target_id.clone()],
                                    cell_targets: Vec::new(),
                                    item_binding: item_binding.clone(),
                                };
                                if let Some(rejection) =
                                    living_intent_rejection(&self.state, &intent)
                                {
                                    if first_rejection.is_none() {
                                        first_rejection = Some(rejection);
                                    }
                                    return false;
                                }
                                match self.rules.preflight(&self.state, &intent) {
                                    Ok(()) => true,
                                    Err(rejection) => {
                                        if first_rejection.is_none() {
                                            first_rejection = Some(rejection);
                                        }
                                        false
                                    }
                                }
                            })
                            .collect();
                        RpgActionOptionsView {
                            binding: option_binding,
                            participant_ids: legal_candidates,
                            cell_paths: Vec::new(),
                            filtered_participants,
                            filtered_cells: Vec::new(),
                            area_options: Vec::new(),
                        }
                    }
                    RpgIrTargetKind::Cell => {
                        let mut filtered_cells = Vec::new();
                        let legal_paths = action
                            .selected_destination_maximum_distance
                            .map(|maximum_distance| {
                                movement_paths(
                                    &self.encounter.scenario.board,
                                    &self.state,
                                    actor_id,
                                    maximum_distance,
                                )
                            })
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|path| {
                                let Some(cell) = self
                                    .encounter
                                    .scenario
                                    .board
                                    .cells
                                    .iter()
                                    .find(|cell| cell.id == path.destination_cell_id)
                                else {
                                    return false;
                                };
                                if action.targets.line_of_effect
                                    == RpgIrLineOfEffectRequirement::Required
                                {
                                    let Some(actor) = self.state.entity(actor_id) else {
                                        return false;
                                    };
                                    let projection = line_of_effect_projection(
                                        &area_board_index,
                                        actor.position(),
                                        cell.position,
                                    );
                                    if !projection.clear {
                                        filtered_cells.push(RpgFilteredCellOptionView {
                                            cell_id: cell.id.clone(),
                                            reason: projection
                                                .reason
                                                .unwrap_or("lineOfEffectBlocked")
                                                .to_owned(),
                                            blocking_cell_ids: projection.blocking_cell_ids,
                                        });
                                        return false;
                                    }
                                }
                                let mut intent = cell_intent(&action.id, actor_id, cell);
                                intent.item_binding = item_binding.clone();
                                if let Err(rejection) = self.rules.preflight(&self.state, &intent) {
                                    if first_rejection.is_none() {
                                        first_rejection = Some(rejection);
                                    }
                                    return false;
                                }
                                true
                            })
                            .collect();
                        RpgActionOptionsView {
                            binding: option_binding,
                            participant_ids: Vec::new(),
                            cell_paths: legal_paths,
                            filtered_participants: Vec::new(),
                            filtered_cells,
                            area_options: Vec::new(),
                        }
                    }
                    RpgIrTargetKind::Area => {
                        let mut filtered_cells = Vec::new();
                        let area_options = self
                            .encounter
                            .scenario
                            .board
                            .cells
                            .iter()
                            .filter_map(|anchor| {
                                if action.targets.line_of_effect
                                    == RpgIrLineOfEffectRequirement::Required
                                {
                                    let actor = self.state.entity(actor_id)?;
                                    let projection = line_of_effect_projection(
                                        &area_board_index,
                                        actor.position(),
                                        anchor.position,
                                    );
                                    if !projection.clear {
                                        filtered_cells.push(RpgFilteredCellOptionView {
                                            cell_id: anchor.id.clone(),
                                            reason: projection
                                                .reason
                                                .unwrap_or("lineOfEffectBlocked")
                                                .to_owned(),
                                            blocking_cell_ids: projection.blocking_cell_ids,
                                        });
                                        return None;
                                    }
                                }
                                let projection = area_projection(
                                    &area_board_index,
                                    &self.state,
                                    actor_id,
                                    &action.targets,
                                    &anchor.id,
                                )?;
                                let intent = RpgIntent {
                                    action_id: action.id.clone(),
                                    actor_id: actor_id.to_owned(),
                                    target_ids: projection.included_participant_ids.clone(),
                                    cell_targets: projection
                                        .included_cells
                                        .iter()
                                        .map(|cell| RpgIntentCellTarget {
                                            id: cell.id.clone(),
                                            position: cell.position,
                                        })
                                        .collect(),
                                    item_binding: item_binding.clone(),
                                };
                                if let Err(rejection) = self.rules.preflight(&self.state, &intent) {
                                    if first_rejection.is_none() {
                                        first_rejection = Some(rejection);
                                    }
                                    return None;
                                }
                                Some(RpgAreaOptionView {
                                    session_binding_id: self.session_binding_id.clone(),
                                    artifact_id: self
                                        .artifact
                                        .as_ref()
                                        .map(|artifact| artifact.artifact_id.clone())
                                        .unwrap_or_default(),
                                    scenario_fingerprint: self.scenario_fingerprint.clone(),
                                    authority_revision: self.state.revision(),
                                    round: self.encounter.turn.round,
                                    turn: self.encounter.turn.turn,
                                    current_actor_id: actor_id.to_owned(),
                                    action_id: action.id.clone(),
                                    item_binding: item_binding.clone(),
                                    origin: projection.origin,
                                    shape: projection.shape,
                                    origin_cell_id: projection.origin_cell_id,
                                    anchor_cell_id: projection.anchor_cell_id,
                                    included_cell_ids: projection
                                        .included_cells
                                        .into_iter()
                                        .map(|cell| cell.id)
                                        .collect(),
                                    filtered_cells: projection.filtered_cells,
                                    included_participant_ids: projection.included_participant_ids,
                                    filtered_participants: projection.filtered_participants,
                                })
                            })
                            .collect();
                        RpgActionOptionsView {
                            binding: option_binding,
                            participant_ids: Vec::new(),
                            cell_paths: Vec::new(),
                            filtered_participants: Vec::new(),
                            filtered_cells,
                            area_options,
                        }
                    }
                };
                if self.pending.is_some() || self.pending_turn_save.is_some() {
                    options.participant_ids.clear();
                    options.cell_paths.clear();
                    options.area_options.clear();
                    first_rejection = Some(if self.pending_turn_save.is_some() {
                        rejection(
                            "RPG_SESSION_TURN_SAVE_PENDING",
                            "$.action",
                            "resolve the pending end-turn saves before choosing an action",
                        )
                    } else {
                        rejection(
                            "RPG_SESSION_REACTION_PENDING",
                            "$.action",
                            "resolve the pending reaction before choosing an action",
                        )
                    });
                }
                let has_options = !options.participant_ids.is_empty()
                    || !options.cell_paths.is_empty()
                    || !options.area_options.is_empty();
                let unavailable = (!has_options).then(|| {
                    first_rejection.unwrap_or_else(|| {
                        rejection(
                            "RPG_ACTION_NO_LEGAL_OPTIONS",
                            "$.action.options",
                            "the action has no legal authority options in the current state",
                        )
                    })
                });
                action_view(
                    action,
                    item_binding,
                    item_label.as_deref(),
                    options,
                    unavailable,
                )
            })
            .collect();
        let activation_budget_definitions =
            self.rules.activation_budgets().cloned().collect::<Vec<_>>();
        let participants = self
            .state
            .entities()
            .map(|entity| {
                let setup = self
                    .encounter
                    .scenario
                    .participants
                    .iter()
                    .find(|participant| participant.id == entity.id());
                let items = setup
                    .map(|participant| participant.items.as_slice())
                    .unwrap_or_default();
                let equipment = setup
                    .map(|participant| participant.equipment.as_slice())
                    .unwrap_or_default();
                participant_view(
                    entity,
                    self.encounter
                        .participant_labels
                        .get(entity.id())
                        .cloned()
                        .unwrap_or_else(|| entity.id().to_owned()),
                    self.encounter
                        .participant_definitions
                        .get(entity.id())
                        .cloned()
                        .unwrap_or_default(),
                    items,
                    equipment,
                    RpgParticipantProjectionCatalogs {
                        item_definitions: &self.encounter.item_definitions,
                        effect_definitions: &self.encounter.effect_definitions,
                        activation_budget_definitions: &activation_budget_definitions,
                    },
                )
            })
            .collect();
        let control_unavailable = if self.pending.is_some() {
            Some(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.control",
                "resolve the pending reaction before ending the turn",
            ))
        } else if self.pending_turn_save.is_some() {
            Some(rejection(
                "RPG_SESSION_TURN_SAVE_PENDING",
                "$.control",
                "submit the required d20 save evidence to finish ending the turn",
            ))
        } else if !matches!(
            encounter_outcome(&self.state),
            RpgEncounterOutcomeView::InProgress
        ) {
            Some(rejection(
                "RPG_ENCOUNTER_COMPLETED",
                "$.control",
                "the encounter has already completed",
            ))
        } else if self
            .state
            .entity(actor_id)
            .map(|participant| participant.vitality().current <= 0)
            .unwrap_or(true)
        {
            Some(rejection(
                "RPG_TURN_ACTOR_INACTIVE",
                "$.control.actorId",
                "the current actor must have positive vitality",
            ))
        } else {
            None
        };
        RpgEncounterView {
            schema: RpgSchemaIdentity {
                id: RPG_ENCOUNTER_VIEW_SCHEMA_ID.to_owned(),
                version: RPG_ENCOUNTER_VIEW_SCHEMA_VERSION,
            },
            session_binding_id: self.session_binding_id.clone(),
            artifact_id: self
                .artifact
                .as_ref()
                .map(|artifact| artifact.artifact_id.clone())
                .unwrap_or_default(),
            scenario_fingerprint: self.scenario_fingerprint.clone(),
            state_revision: self.state.revision(),
            accepted_random_position: self.accepted_random_values,
            random_source: self.encounter.scenario.random_source.clone(),
            board: self.encounter.scenario.board.clone(),
            participants,
            turn: self.encounter.turn.clone(),
            accepted_activations_this_turn: self.state.accepted_activations_this_turn(),
            accepted_activation_ceiling: self.rules.accepted_activation_ceiling(),
            actions,
            controls: vec![RpgTurnControlView {
                control: RpgTurnControl::EndTurn,
                label: "End turn".to_owned(),
                available: control_unavailable.is_none(),
                unavailable: control_unavailable,
            }],
            pending_reaction: self
                .pending_reaction()
                .map(|pending| pending.request.clone()),
            pending_turn_save: self.pending_turn_save.clone(),
            log: self.encounter.log.clone(),
            outcome: encounter_outcome(&self.state),
        }
    }

    pub(crate) fn submit(&mut self, command: RpgAuthorityCommand) -> RpgCommandOutcome {
        self.submit_with_prepared_area(command, None)
    }

    pub(crate) fn submit_with_prepared_area(
        &mut self,
        mut command: RpgAuthorityCommand,
        prepared_area: Option<PreparedAreaCommand>,
    ) -> RpgCommandOutcome {
        let portable_intent = command.intent.clone();
        if self.pending.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.command",
                "resolve the pending reaction before submitting another command",
            ));
        }
        if self.pending_turn_save.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_TURN_SAVE_PENDING",
                "$.command",
                "resolve the pending end-turn saves before submitting another command",
            ));
        }
        if command.expected_revision != self.state.revision() {
            return RpgCommandOutcome::Rejected(revision_rejection(
                command.expected_revision,
                self.state.revision(),
            ));
        }
        if !matches!(
            encounter_outcome(&self.state),
            RpgEncounterOutcomeView::InProgress
        ) {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_ENCOUNTER_COMPLETED",
                "$.command",
                "the encounter has already completed",
            ));
        }
        if command.intent.actor_id != self.encounter.current_actor_id() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_TURN_ACTOR_MISMATCH",
                "$.command.intent.actorId",
                format!("current actor is {}", self.encounter.current_actor_id()),
            ));
        }
        let actor_definitions = self
            .encounter
            .participant_definitions
            .get(&command.intent.actor_id);
        if !actor_definitions
            .map(|definitions| definitions.contains(&command.intent.action_id))
            .unwrap_or(false)
        {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_ACTION_NOT_OWNED",
                "$.command.intent.actionId",
                format!(
                    "participant {} does not reference action {}",
                    command.intent.actor_id, command.intent.action_id
                ),
            ));
        }
        if let Some(rejection) = self.item_binding_rejection(&command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }
        let area_event = match prepared_area {
            Some(prepared) => {
                command.intent = prepared.resolved_intent;
                Some(prepared.event)
            }
            None => match self.prepare_area_command(command.clone()) {
                Ok((prepared, event)) => {
                    command = prepared;
                    event
                }
                Err(rejection) => return RpgCommandOutcome::Rejected(rejection),
            },
        };
        if let Some(rejection) = living_intent_rejection(&self.state, &command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }
        if let Some(rejection) = self.cell_binding_rejection(&command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }
        if let Err(rejection) = self.rules.preflight(&self.state, &command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }
        if let Some(rejection) = self.movement_path_rejection(&command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }
        if let Some(rejection) = self.line_of_effect_rejection(&command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
        }

        let mut staged_state = self.state.clone();
        let mut random = DeterministicRandomStream::new(command.random_values.clone());
        let resolution_context =
            encounter_resolution_context(&self.encounter.scenario, &staged_state);
        match self.rules.resolve_with_context(
            &mut staged_state,
            &mut random,
            &command.intent,
            &resolution_context,
        ) {
            Ok(mut receipt) => {
                if let Some(area_event) = area_event {
                    prepend_area_evidence(&mut receipt, area_event);
                }
                if let Some(rejection) =
                    runtime_board_rejection(&self.encounter.scenario.board, &staged_state)
                {
                    return RpgCommandOutcome::Rejected(rejection);
                }
                let advances_turn = !self.rules.uses_variable_activation_budgets()
                    && matches!(
                        encounter_outcome(&staged_state),
                        RpgEncounterOutcomeView::InProgress
                    );
                let refreshed = refreshed_modifiers(&receipt.events);
                if advances_turn {
                    if let Err(rejection) = append_automatic_turn_saves(
                        &self.encounter,
                        &mut staged_state,
                        &mut random,
                        &mut receipt,
                    ) {
                        return RpgCommandOutcome::Rejected(rejection);
                    }
                }
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                let next_turn = advances_turn.then(|| {
                    append_turn_events(
                        &self.rules,
                        &self.encounter,
                        &self.state,
                        &mut staged_state,
                        &mut receipt.events,
                        refreshed,
                        receipt.state_revision,
                    )
                });
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                self.encounter.record(&receipt);
                if let Some(next_turn) = next_turn {
                    self.encounter.set_turn(next_turn);
                }
                RpgCommandOutcome::Accepted(receipt)
            }
            Err(mut error) => {
                let Some(request) = error.reaction_request.take() else {
                    return RpgCommandOutcome::Rejected(error);
                };
                let pending = RpgPendingReaction {
                    expected_revision: command.expected_revision,
                    request: *request,
                    trace: *error.trace,
                    random_evidence: *error.random_evidence,
                    random_attempted: error.random_attempted,
                };
                self.pending = Some(PendingTransaction {
                    expected_revision: command.expected_revision,
                    intent: command.intent,
                    portable_intent,
                    random_values: command.random_values,
                    pending: pending.clone(),
                    area_event,
                });
                RpgCommandOutcome::AwaitingReaction(pending)
            }
        }
    }

    fn item_binding_rejection(&self, intent: &RpgIntent) -> Option<RpgResolutionRejection> {
        let requirement = self.rules.binding_requirement(&intent.action_id);
        match (requirement, &intent.item_binding) {
            (None, None) => None,
            (None, Some(_)) => Some(rejection(
                "RPG_ACTION_ITEM_BINDING_UNEXPECTED",
                "$.command.intent.itemBinding",
                "this action does not accept an equipped item binding",
            )),
            (Some(_), None) => Some(rejection(
                "RPG_ACTION_ITEM_BINDING_REQUIRED",
                "$.command.intent.itemBinding",
                "this action requires a compatible equipped item binding",
            )),
            (Some(requirement), Some(binding)) if binding.binding_id != requirement.id => {
                Some(rejection(
                    "RPG_ACTION_ITEM_BINDING_ID_MISMATCH",
                    "$.command.intent.itemBinding.bindingId",
                    format!("expected item binding {}", requirement.id),
                ))
            }
            (Some(_), Some(binding)) => {
                let valid = self
                    .item_bindings_for_actor(
                        &intent.actor_id,
                        &intent.action_id,
                        &binding.item_definition_id,
                    )
                    .contains(binding);
                (!valid).then(|| {
                    rejection(
                        "RPG_ACTION_ITEM_BINDING_STALE",
                        "$.command.intent.itemBinding",
                        "the submitted item binding is not the actor's current compatible equipment",
                    )
                })
            }
        }
    }

    pub(crate) fn react(&mut self, command: RpgReactionCommand) -> RpgCommandOutcome {
        if self.pending_turn_save.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_TURN_SAVE_PENDING",
                "$.reaction",
                "resolve the pending end-turn saves before submitting a reaction",
            ));
        }
        let Some(transaction) = self.pending.clone() else {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_ABSENT",
                "$.reaction",
                "there is no pending reaction to resolve",
            ));
        };
        if command.expected_revision != transaction.expected_revision
            || command.expected_revision != self.state.revision()
        {
            return RpgCommandOutcome::Rejected(revision_rejection(
                command.expected_revision,
                self.state.revision(),
            ));
        }
        if command.reaction_id != transaction.pending.request.reaction_id {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_REACTION_ID_MISMATCH",
                "$.reaction.reactionId",
                format!(
                    "expected reaction {}",
                    transaction.pending.request.reaction_id
                ),
            ));
        }

        let mut evidence = transaction.random_values.clone();
        evidence.extend(command.additional_random_values);
        let mut staged_state = self.state.clone();
        let mut random = DeterministicRandomStream::new(evidence.clone());
        let decision = RpgReactionDecision {
            reaction_id: command.reaction_id,
            option_id: command.option_id,
        };
        let resolution_context =
            encounter_resolution_context(&self.encounter.scenario, &staged_state);
        match self.rules.resolve_with_reaction_decision_and_context(
            &mut staged_state,
            &mut random,
            &transaction.intent,
            &decision,
            &resolution_context,
        ) {
            Ok(mut receipt) => {
                if let Some(area_event) = transaction.area_event {
                    prepend_area_evidence(&mut receipt, area_event);
                }
                if let Some(rejection) =
                    runtime_board_rejection(&self.encounter.scenario.board, &staged_state)
                {
                    return RpgCommandOutcome::Rejected(rejection);
                }
                let advances_turn = !self.rules.uses_variable_activation_budgets()
                    && matches!(
                        encounter_outcome(&staged_state),
                        RpgEncounterOutcomeView::InProgress
                    );
                let refreshed = refreshed_modifiers(&receipt.events);
                if advances_turn {
                    if let Err(rejection) = append_automatic_turn_saves(
                        &self.encounter,
                        &mut staged_state,
                        &mut random,
                        &mut receipt,
                    ) {
                        return RpgCommandOutcome::Rejected(rejection);
                    }
                }
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                let next_turn = advances_turn.then(|| {
                    append_turn_events(
                        &self.rules,
                        &self.encounter,
                        &self.state,
                        &mut staged_state,
                        &mut receipt.events,
                        refreshed,
                        receipt.state_revision,
                    )
                });
                self.pending = None;
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                self.encounter.record(&receipt);
                if let Some(next_turn) = next_turn {
                    self.encounter.set_turn(next_turn);
                }
                RpgCommandOutcome::Accepted(receipt)
            }
            Err(error) => RpgCommandOutcome::Rejected(error),
        }
    }

    pub(crate) fn control(&mut self, command: RpgTurnControlCommand) -> RpgCommandOutcome {
        if self.pending.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.control",
                "resolve the pending reaction before ending the turn",
            ));
        }
        if let Some(pending) = self.pending_turn_save.clone() {
            return self.resolve_pending_turn_save(command, pending);
        }
        if command.expected_revision != self.state.revision() {
            return RpgCommandOutcome::Rejected(revision_rejection(
                command.expected_revision,
                self.state.revision(),
            ));
        }
        if !matches!(
            encounter_outcome(&self.state),
            RpgEncounterOutcomeView::InProgress
        ) {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_ENCOUNTER_COMPLETED",
                "$.control",
                "the encounter has already completed",
            ));
        }
        if command.actor_id != self.encounter.current_actor_id() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_TURN_ACTOR_MISMATCH",
                "$.control.actorId",
                format!("current actor is {}", self.encounter.current_actor_id()),
            ));
        }
        if self
            .state
            .entity(&command.actor_id)
            .map(|participant| participant.vitality().current <= 0)
            .unwrap_or(true)
        {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_TURN_ACTOR_INACTIVE",
                "$.control.actorId",
                "the current actor must have positive vitality",
            ));
        }
        let save_candidates = match self.turn_save_candidates(&command.actor_id) {
            Ok(candidates) => candidates,
            Err(rejection) => return RpgCommandOutcome::Rejected(rejection),
        };
        if !save_candidates.is_empty() {
            if !command.random_values.is_empty() {
                return RpgCommandOutcome::Rejected(rejection(
                    "RPG_EFFECT_SAVE_PHASE_REQUIRED",
                    "$.control.randomValues",
                    "end-turn save evidence may only be submitted after authority opens the typed save phase",
                ));
            }
            let pending = RpgPendingTurnSave {
                expected_revision: command.expected_revision,
                actor_id: command.actor_id,
                control: command.control,
                candidates: save_candidates,
            };
            self.pending_turn_save = Some(pending.clone());
            return RpgCommandOutcome::AwaitingTurnSave(pending);
        }
        if !command.random_values.is_empty() {
            return RpgCommandOutcome::Rejected(unused_random_rejection(
                command.random_values.len(),
            ));
        }

        let mut staged_state = self.state.clone();
        let mut events = Vec::new();
        let next_turn = append_turn_events(
            &self.rules,
            &self.encounter,
            &self.state,
            &mut staged_state,
            &mut events,
            BTreeSet::new(),
            self.state.revision().saturating_add(1),
        );
        let state_revision = staged_state.advance_revision();
        let receipt = RpgTurnControlReceipt {
            control: command.control,
            actor_id: command.actor_id,
            events,
            random_evidence: Vec::new(),
            random_consumed: 0,
            state_revision,
        };
        self.state = staged_state;
        self.encounter.record_control(&receipt);
        self.encounter.set_turn(next_turn);
        RpgCommandOutcome::ControlAccepted(receipt)
    }

    fn turn_save_candidates(
        &self,
        actor_id: &str,
    ) -> Result<Vec<RpgEffectSaveCandidate>, RpgResolutionRejection> {
        turn_save_candidates_for(&self.state, actor_id, &self.encounter.effect_definitions)
    }

    fn resolve_pending_turn_save(
        &mut self,
        command: RpgTurnControlCommand,
        pending: RpgPendingTurnSave,
    ) -> RpgCommandOutcome {
        if command.expected_revision != pending.expected_revision
            || command.actor_id != pending.actor_id
            || command.control != pending.control
        {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_EFFECT_SAVE_PENDING_MISMATCH",
                "$.control",
                "turn-save evidence does not match the authority-owned pending transition",
            ));
        }
        let current = match self.turn_save_candidates(&command.actor_id) {
            Ok(candidates) => candidates,
            Err(rejection) => return RpgCommandOutcome::Rejected(rejection),
        };
        if current != pending.candidates {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_EFFECT_SAVE_PENDING_STALE",
                "$.control.saves",
                "the authority-owned save candidate set no longer matches current effect state",
            ));
        }
        if command.random_values.len() < pending.candidates.len() {
            let mut rejection = rejection(
                "RPG_EFFECT_SAVE_EVIDENCE_UNDERFLOW",
                "$.control.randomValues",
                format!(
                    "end-turn saves require exactly {} d20 values",
                    pending.candidates.len()
                ),
            );
            rejection.random_request = pending
                .candidates
                .get(command.random_values.len())
                .map(|candidate| Box::new(candidate.request.clone()));
            return RpgCommandOutcome::Rejected(rejection);
        }
        if command.random_values.len() > pending.candidates.len() {
            return RpgCommandOutcome::Rejected(unused_random_rejection(
                command
                    .random_values
                    .len()
                    .saturating_sub(pending.candidates.len()),
            ));
        }
        for (index, value) in command.random_values.iter().enumerate() {
            if !(1..=20).contains(value) {
                return RpgCommandOutcome::Rejected(rejection(
                    "RPG_RANDOM_VALUE_OUT_OF_RANGE",
                    format!("$.control.randomValues[{index}]"),
                    format!("save value {value} is outside 1..=20"),
                ));
            }
        }

        let mut staged_state = self.state.clone();
        let mut events = Vec::new();
        let mut random_evidence = Vec::with_capacity(pending.candidates.len());
        for (candidate, roll) in pending
            .candidates
            .iter()
            .zip(command.random_values.iter().copied())
        {
            let saved = roll >= 10;
            events.push(rpg_core::RpgDomainEvent::EffectSaveResolved {
                target_id: candidate.target_id.clone(),
                instance_id: candidate.instance_id.clone(),
                definition_id: candidate.definition_id.clone(),
                definition_version: candidate.definition_version,
                source_id: candidate.source_entity_id.clone(),
                roll,
                difficulty: 10,
                saved,
            });
            random_evidence.push(rpg_core::RpgRandomEvidence {
                request: candidate.request.clone(),
                values: vec![roll],
                heterogeneous_values: Vec::new(),
            });
            if saved {
                let removed = staged_state
                    .effects_owner()
                    .remove_instance(&candidate.target_id, &candidate.instance_id)
                    .expect("validated save target remains available")
                    .expect("validated save effect remains active");
                events.push(rpg_core::RpgDomainEvent::EffectExpired {
                    target_id: candidate.target_id.clone(),
                    instance_id: removed.instance_id().to_owned(),
                    definition_id: removed.definition_id().to_owned(),
                    definition_version: removed.definition_version(),
                    source_id: removed.source_entity_id().to_owned(),
                    duration_anchor: removed.duration_anchor(),
                    tenure: self
                        .encounter
                        .effect_definitions
                        .get(&candidate.definition_id)
                        .map(|definition| definition.tenure)
                        .unwrap_or_else(|| removed.tenure()),
                });
            }
        }
        let next_turn = append_turn_events(
            &self.rules,
            &self.encounter,
            &self.state,
            &mut staged_state,
            &mut events,
            BTreeSet::new(),
            self.state.revision().saturating_add(1),
        );
        let state_revision = staged_state.advance_revision();
        let random_consumed = u64::try_from(random_evidence.len()).unwrap_or(u64::MAX);
        let receipt = RpgTurnControlReceipt {
            control: command.control,
            actor_id: command.actor_id,
            events,
            random_evidence,
            random_consumed,
            state_revision,
        };
        self.pending_turn_save = None;
        self.state = staged_state;
        self.accepted_random_values = self.accepted_random_values.saturating_add(random_consumed);
        self.encounter.record_control(&receipt);
        self.encounter.set_turn(next_turn);
        RpgCommandOutcome::ControlAccepted(receipt)
    }

    pub fn submit_with_random_source_recorded(
        &mut self,
        proposal: RpgActionProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgAutomaticCommandFailure> {
        self.require_random_source(source)?;
        let cell_targets = self.proposal_cell_targets(&proposal);
        let command = RpgAuthorityCommand {
            expected_revision: proposal.expected_revision,
            intent: RpgIntent {
                action_id: proposal.action_id,
                actor_id: proposal.actor_id,
                target_ids: proposal.target_ids,
                cell_targets,
                item_binding: proposal.item_binding,
            },
            random_values: Vec::new(),
        };
        self.submit_command_with_random_source_recorded(command, source, None)
    }

    pub fn submit_bound_with_random_source_recorded(
        &mut self,
        proposal: RpgBoundActionProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<RpgBoundActionSubmissionResult, RpgAutomaticCommandFailure> {
        if let Some(rejection) = self.action_option_binding_rejection(&proposal.binding) {
            return Ok(RpgBoundActionSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        }
        let cell_targets = self.proposal_cell_targets(&RpgActionProposal {
            expected_revision: proposal.binding.authority_revision,
            action_id: proposal.binding.action_id.clone(),
            actor_id: proposal.binding.current_actor_id.clone(),
            target_ids: proposal.target_ids.clone(),
            item_binding: proposal.binding.item_binding.clone(),
        });
        let command = RpgAuthorityCommand {
            expected_revision: proposal.binding.authority_revision,
            intent: RpgIntent {
                action_id: proposal.binding.action_id,
                actor_id: proposal.binding.current_actor_id,
                target_ids: proposal.target_ids,
                cell_targets,
                item_binding: proposal.binding.item_binding,
            },
            random_values: Vec::new(),
        };
        self.require_random_source(source)?;
        let (outcome, replay_entry) =
            self.submit_command_with_random_source_recorded(command, source, None)?;
        Ok(RpgBoundActionSubmissionResult {
            outcome,
            replay_entry: Some(replay_entry),
            encounter: self.encounter_view(),
        })
    }

    pub fn submit_area_with_random_source_recorded(
        &mut self,
        proposal: RpgAreaActionProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<RpgAreaSubmissionResult, RpgAutomaticCommandFailure> {
        if proposal.session_binding_id != self.session_binding_id {
            return Ok(RpgAreaSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection(
                    "RPG_AREA_OPTION_STALE",
                    "$.proposal.sessionBindingId",
                    "the area option belongs to a different authority session",
                )),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        }
        if proposal.authority_revision != self.state.revision() {
            return Ok(RpgAreaSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection(
                    "RPG_AREA_OPTION_STALE",
                    "$.proposal.authorityRevision",
                    format!(
                        "the area option is at revision {}, but authority is at {}",
                        proposal.authority_revision,
                        self.state.revision()
                    ),
                )),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        }
        let item_definition_id = proposal
            .item_binding
            .as_ref()
            .map(|binding| binding.item_definition_id.as_str());
        let Ok(targets) = self
            .rules
            .target_selector_for_binding(&proposal.action_id, item_definition_id)
        else {
            return Ok(RpgAreaSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection(
                    "RPG_AREA_OPTION_INVALID",
                    "$.proposal.actionId",
                    "the selected action does not expose an area option",
                )),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        };
        let anchor = self
            .encounter
            .scenario
            .board
            .cells
            .iter()
            .find(|cell| cell.id == proposal.anchor_cell_id);
        if targets.kind != RpgIrTargetKind::Area || anchor.is_none() {
            return Ok(RpgAreaSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection(
                    "RPG_AREA_OPTION_INVALID",
                    "$.proposal.anchorCellId",
                    "the selected area anchor is not projectable at this authority revision",
                )),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        }
        let anchor = anchor.expect("validated area anchor exists");
        let command = RpgAuthorityCommand {
            expected_revision: proposal.authority_revision,
            intent: RpgIntent {
                action_id: proposal.action_id,
                actor_id: proposal.actor_id,
                target_ids: Vec::new(),
                cell_targets: vec![RpgIntentCellTarget {
                    id: anchor.id.clone(),
                    position: anchor.position,
                }],
                item_binding: proposal.item_binding,
            },
            random_values: Vec::new(),
        };
        let prepared_area = match self.prepare_area_command(command.clone()) {
            Ok((prepared, Some(event))) => PreparedAreaCommand {
                resolved_intent: prepared.intent,
                event,
            },
            Ok((_, None)) | Err(_) => {
                return Ok(RpgAreaSubmissionResult {
                    outcome: RpgCommandOutcome::Rejected(rejection(
                        "RPG_AREA_OPTION_INVALID",
                        "$.proposal.anchorCellId",
                        "the selected area anchor is not projectable at this authority revision",
                    )),
                    replay_entry: None,
                    encounter: self.encounter_view(),
                });
            }
        };
        self.require_random_source(source)?;
        let (outcome, replay_entry) =
            self.submit_command_with_random_source_recorded(command, source, Some(prepared_area))?;
        Ok(RpgAreaSubmissionResult {
            outcome,
            replay_entry: Some(replay_entry),
            encounter: self.encounter_view(),
        })
    }

    pub fn submit_area_option_with_random_source_recorded(
        &mut self,
        option: RpgAreaOptionView,
        source: &mut dyn RpgRandomSource,
    ) -> Result<RpgAreaSubmissionResult, RpgAutomaticCommandFailure> {
        let current = self.encounter_view().actions.into_iter().any(|action| {
            action.definition_id == option.action_id
                && action.item_binding == option.item_binding
                && action
                    .options
                    .area_options
                    .iter()
                    .any(|candidate| candidate == &option)
        });
        if !current {
            return Ok(RpgAreaSubmissionResult {
                outcome: RpgCommandOutcome::Rejected(rejection(
                    "RPG_AREA_OPTION_STALE",
                    "$.option",
                    "the complete area option does not match current authority readback",
                )),
                replay_entry: None,
                encounter: self.encounter_view(),
            });
        }
        self.submit_area_with_random_source_recorded(
            RpgAreaActionProposal {
                session_binding_id: option.session_binding_id,
                authority_revision: option.authority_revision,
                action_id: option.action_id,
                actor_id: option.current_actor_id,
                anchor_cell_id: option.anchor_cell_id,
                item_binding: option.item_binding,
            },
            source,
        )
    }

    fn submit_command_with_random_source_recorded(
        &mut self,
        command: RpgAuthorityCommand,
        source: &mut dyn RpgRandomSource,
        prepared_area: Option<PreparedAreaCommand>,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgAutomaticCommandFailure> {
        let baseline = self.probe_snapshot();
        let mut random_values = Vec::new();
        for _ in 0..MAXIMUM_AUTOMATIC_RANDOM_REQUESTS {
            let mut probe = baseline.probe_snapshot();
            let mut attempted_command = command.clone();
            attempted_command.random_values = random_values.clone();
            let outcome =
                probe.submit_with_prepared_area(attempted_command.clone(), prepared_area.clone());
            let Some(request) = required_random_request(&outcome) else {
                return match prepared_area {
                    Some(prepared) => self
                        .submit_prepared_area_recorded(attempted_command, prepared)
                        .map_err(RpgAutomaticCommandFailure::Replay),
                    None => self
                        .submit_recorded(attempted_command)
                        .map_err(RpgAutomaticCommandFailure::Replay),
                };
            };
            extend_random_values(&mut random_values, request, source)?;
        }
        Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_REQUEST_LIMIT_EXCEEDED",
            "$.randomRequest",
            "authority did not reach a terminal result within the random request limit",
        )))
    }

    pub fn react_with_random_source_recorded(
        &mut self,
        proposal: RpgReactionProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgAutomaticCommandFailure> {
        self.require_random_source(source)?;
        let baseline = self.probe_snapshot();
        let mut additional_random_values = Vec::new();
        for _ in 0..MAXIMUM_AUTOMATIC_RANDOM_REQUESTS {
            let mut probe = baseline.probe_snapshot();
            let command = RpgReactionCommand {
                expected_revision: proposal.expected_revision,
                reaction_id: proposal.reaction_id.clone(),
                option_id: proposal.option_id.clone(),
                additional_random_values: additional_random_values.clone(),
            };
            let outcome = probe.react(command.clone());
            let Some(request) = required_random_request(&outcome) else {
                return self
                    .react_recorded(command)
                    .map_err(RpgAutomaticCommandFailure::Replay);
            };
            extend_random_values(&mut additional_random_values, request, source)?;
        }
        Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_REQUEST_LIMIT_EXCEEDED",
            "$.randomRequest",
            "authority did not reach a terminal result within the random request limit",
        )))
    }

    pub fn control_recorded(
        &mut self,
        proposal: RpgTurnControlProposal,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_turn_control(RpgTurnControlCommand {
            expected_revision: proposal.expected_revision,
            actor_id: proposal.actor_id,
            control: proposal.control,
            random_values: Vec::new(),
        })
    }

    pub fn control_with_random_values_recorded(
        &mut self,
        proposal: RpgTurnControlProposal,
        random_values: Vec<u32>,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_turn_control(RpgTurnControlCommand {
            expected_revision: proposal.expected_revision,
            actor_id: proposal.actor_id,
            control: proposal.control,
            random_values,
        })
    }

    pub fn control_with_random_source_recorded(
        &mut self,
        proposal: RpgTurnControlProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<(RpgCommandOutcome, Vec<RpgReplayEntry>), RpgAutomaticCommandFailure> {
        self.require_random_source(source)?;
        let command = RpgTurnControlCommand {
            expected_revision: proposal.expected_revision,
            actor_id: proposal.actor_id,
            control: proposal.control,
            random_values: Vec::new(),
        };
        let (outcome, first_entry) = self
            .record_turn_control(command.clone())
            .map_err(RpgAutomaticCommandFailure::Replay)?;
        let RpgCommandOutcome::AwaitingTurnSave(pending) = outcome else {
            return Ok((outcome, vec![first_entry]));
        };
        let mut random_values = Vec::with_capacity(pending.candidates.len());
        for candidate in &pending.candidates {
            extend_random_values(&mut random_values, &candidate.request, source)?;
        }
        let (outcome, second_entry) = self
            .record_turn_control(RpgTurnControlCommand {
                random_values,
                ..command
            })
            .map_err(RpgAutomaticCommandFailure::Replay)?;
        Ok((outcome, vec![first_entry, second_entry]))
    }

    fn proposal_cell_targets(&self, proposal: &RpgActionProposal) -> Vec<RpgIntentCellTarget> {
        if self.rules.target_kind_for_binding(
            &proposal.action_id,
            proposal
                .item_binding
                .as_ref()
                .map(|binding| binding.item_definition_id.as_str()),
        ) != Ok(RpgIrTargetKind::Cell)
        {
            return Vec::new();
        }
        proposal
            .target_ids
            .iter()
            .filter_map(|target_id| {
                self.encounter
                    .scenario
                    .board
                    .cells
                    .iter()
                    .find(|cell| cell.id == *target_id)
                    .map(|cell| RpgIntentCellTarget {
                        id: cell.id.clone(),
                        position: cell.position,
                    })
            })
            .collect()
    }

    fn cell_binding_rejection(&self, intent: &RpgIntent) -> Option<RpgResolutionRejection> {
        let Ok(target_kind) = self.rules.target_kind_for_binding(
            &intent.action_id,
            intent
                .item_binding
                .as_ref()
                .map(|binding| binding.item_definition_id.as_str()),
        ) else {
            return None;
        };
        if target_kind == RpgIrTargetKind::Participant {
            return (!intent.cell_targets.is_empty()).then(|| {
                rejection(
                    "RPG_INTENT_CELL_BINDING_UNEXPECTED",
                    "$.command.intent.cellTargets",
                    "participant-target actions cannot include cell bindings",
                )
            });
        }
        let bindings = if target_kind == RpgIrTargetKind::Cell {
            intent
                .target_ids
                .iter()
                .filter_map(|target_id| {
                    intent
                        .cell_targets
                        .iter()
                        .find(|binding| binding.id == *target_id)
                })
                .collect::<Vec<_>>()
        } else {
            intent.cell_targets.iter().collect::<Vec<_>>()
        };
        if target_kind == RpgIrTargetKind::Cell && bindings.len() != intent.target_ids.len() {
            return Some(rejection(
                "RPG_INTENT_CELL_BINDING_MISSING",
                "$.command.intent.cellTargets",
                "every selected cell requires one authoritative position binding",
            ));
        }
        for (index, binding) in bindings.into_iter().enumerate() {
            let Some(cell) = self
                .encounter
                .scenario
                .board
                .cells
                .iter()
                .find(|cell| cell.id == binding.id)
            else {
                return Some(rejection(
                    "RPG_INTENT_CELL_UNKNOWN",
                    format!("$.command.intent.cellTargets[{index}].id"),
                    format!("unknown encounter cell {}", binding.id),
                ));
            };
            if binding.position != cell.position {
                return Some(rejection(
                    "RPG_INTENT_CELL_BINDING_MISMATCH",
                    format!("$.command.intent.cellTargets[{index}].position"),
                    format!(
                        "selected cell {} does not match the encounter board",
                        binding.id
                    ),
                ));
            }
        }
        None
    }

    fn prepare_area_command(
        &self,
        mut command: RpgAuthorityCommand,
    ) -> Result<(RpgAuthorityCommand, Option<rpg_core::RpgDomainEvent>), RpgResolutionRejection>
    {
        let item_definition_id = command
            .intent
            .item_binding
            .as_ref()
            .map(|binding| binding.item_definition_id.as_str());
        let targets = self
            .rules
            .target_selector_for_binding(&command.intent.action_id, item_definition_id)?;
        if targets.kind != RpgIrTargetKind::Area {
            return Ok((command, None));
        }
        if !command.intent.target_ids.is_empty() || command.intent.cell_targets.len() != 1 {
            return Err(rejection(
                "RPG_AREA_COMMAND_BINDING_INVALID",
                "$.command.intent",
                "portable area commands carry one anchor binding and no caller-derived participants",
            ));
        }
        let anchor = &command.intent.cell_targets[0];
        let board_anchor = self
            .encounter
            .scenario
            .board
            .cells
            .iter()
            .find(|cell| cell.id == anchor.id)
            .filter(|cell| cell.position == anchor.position)
            .ok_or_else(|| {
                rejection(
                    "RPG_AREA_OPTION_INVALID",
                    "$.command.intent.cellTargets[0]",
                    "the area anchor does not match the current encounter board",
                )
            })?;
        let projection = area_projection(
            &RpgAreaBoardIndex::new(&self.encounter.scenario.board, &self.state),
            &self.state,
            &command.intent.actor_id,
            &targets,
            &board_anchor.id,
        )
        .ok_or_else(|| {
            rejection(
                "RPG_AREA_OPTION_INVALID",
                "$.command.intent.cellTargets[0]",
                "the area anchor is not projectable in the current authority state",
            )
        })?;
        let event = rpg_core::RpgDomainEvent::AreaTargetsDerived {
            actor_id: command.intent.actor_id.clone(),
            action_id: command.intent.action_id.clone(),
            proposal_revision: command.expected_revision,
            shape: match projection.shape {
                rpg_ir::RpgIrAreaShape::Diamond { radius } => {
                    rpg_core::RpgAreaShape::Diamond { radius }
                }
                rpg_ir::RpgIrAreaShape::OrthogonalLine { length } => {
                    rpg_core::RpgAreaShape::OrthogonalLine { length }
                }
            },
            origin: match projection.origin {
                rpg_ir::RpgIrAreaOrigin::Anchor => rpg_core::RpgAreaOrigin::Anchor,
                rpg_ir::RpgIrAreaOrigin::Actor => rpg_core::RpgAreaOrigin::Actor,
            },
            origin_cell_id: projection.origin_cell_id.clone(),
            anchor_cell_id: projection.anchor_cell_id.clone(),
            included_cell_ids: projection
                .included_cells
                .iter()
                .map(|cell| cell.id.clone())
                .collect(),
            filtered_cells: projection
                .filtered_cells
                .iter()
                .map(|cell| rpg_core::RpgAreaFilteredCell {
                    x: cell.x,
                    y: cell.y,
                    reason: cell.reason.clone(),
                    blocking_cell_ids: cell.blocking_cell_ids.clone(),
                })
                .collect(),
            included_participant_ids: projection.included_participant_ids.clone(),
            filtered_participants: projection
                .filtered_participants
                .iter()
                .map(|participant| rpg_core::RpgAreaFilteredParticipant {
                    participant_id: participant.participant_id.clone(),
                    reason: participant.reason.clone(),
                    blocking_cell_ids: participant.blocking_cell_ids.clone(),
                })
                .collect(),
        };
        command.intent.target_ids = projection.included_participant_ids;
        command.intent.cell_targets = projection
            .included_cells
            .into_iter()
            .map(|cell| RpgIntentCellTarget {
                id: cell.id,
                position: cell.position,
            })
            .collect();
        Ok((command, Some(event)))
    }

    fn movement_path_rejection(&self, intent: &RpgIntent) -> Option<RpgResolutionRejection> {
        let maximum_distance = self
            .rules
            .selected_destination_maximum_distance_for_binding(
                &intent.action_id,
                intent
                    .item_binding
                    .as_ref()
                    .map(|binding| binding.item_definition_id.as_str()),
            )?;
        let paths = movement_paths(
            &self.encounter.scenario.board,
            &self.state,
            &intent.actor_id,
            maximum_distance,
        );
        intent.target_ids.iter().enumerate().find_map(|(index, target_id)| {
            (!paths
                .iter()
                .any(|path| path.destination_cell_id == *target_id))
            .then(|| {
                rejection(
                    "RPG_MOVEMENT_PATH_UNAVAILABLE",
                    format!("$.command.intent.targetIds[{index}]"),
                    format!(
                        "destination {target_id} has no traversable path within movement cost {maximum_distance}"
                    ),
                )
            })
        })
    }

    fn action_option_binding_rejection(
        &self,
        binding: &RpgActionOptionBindingView,
    ) -> Option<RpgResolutionRejection> {
        let artifact_id = self
            .artifact
            .as_ref()
            .map(|artifact| artifact.artifact_id.as_str())
            .unwrap_or_default();
        let mismatch = if binding.session_binding_id != self.session_binding_id {
            Some((
                "$.proposal.binding.sessionBindingId",
                "the option belongs to a different authority session",
            ))
        } else if binding.artifact_id != artifact_id {
            Some((
                "$.proposal.binding.artifactId",
                "the option belongs to a different compiled artifact",
            ))
        } else if binding.scenario_fingerprint != self.scenario_fingerprint {
            Some((
                "$.proposal.binding.scenarioFingerprint",
                "the option belongs to a different scenario",
            ))
        } else if binding.authority_revision != self.state.revision() {
            Some((
                "$.proposal.binding.authorityRevision",
                "the option belongs to a different authority revision",
            ))
        } else if binding.round != self.encounter.turn.round {
            Some((
                "$.proposal.binding.round",
                "the option belongs to a different round",
            ))
        } else if binding.turn != self.encounter.turn.turn {
            Some((
                "$.proposal.binding.turn",
                "the option belongs to a different turn",
            ))
        } else if binding.current_actor_id != self.encounter.turn.current_actor_id {
            Some((
                "$.proposal.binding.currentActorId",
                "the option belongs to a different current actor",
            ))
        } else {
            let available = self.encounter_view().actions.into_iter().any(|action| {
                action.definition_id == binding.action_id
                    && action.item_binding == binding.item_binding
                    && action.options.binding == *binding
            });
            (!available).then_some((
                "$.proposal.binding",
                "the action and item binding are not current authority options",
            ))
        };
        mismatch.map(|(path, message)| rejection("RPG_ACTION_OPTION_STALE", path, message))
    }

    fn line_of_effect_rejection(&self, intent: &RpgIntent) -> Option<RpgResolutionRejection> {
        let targets = self
            .rules
            .target_selector_for_binding(
                &intent.action_id,
                intent
                    .item_binding
                    .as_ref()
                    .map(|binding| binding.item_definition_id.as_str()),
            )
            .ok()?;
        if targets.line_of_effect != RpgIrLineOfEffectRequirement::Required
            || targets.kind == RpgIrTargetKind::Area
        {
            return None;
        }
        let board_index = RpgAreaBoardIndex::new(&self.encounter.scenario.board, &self.state);
        let actor = self.state.entity(&intent.actor_id)?;
        match targets.kind {
            RpgIrTargetKind::Participant => {
                intent
                    .target_ids
                    .iter()
                    .enumerate()
                    .find_map(|(index, target_id)| {
                        let target = self.state.entity(target_id)?;
                        let projection = line_of_effect_projection(
                            &board_index,
                            actor.position(),
                            target.position(),
                        );
                        (!projection.clear).then(|| {
                            rejection(
                                "RPG_LINE_OF_EFFECT_BLOCKED",
                                format!("$.command.intent.targetIds[{index}]"),
                                format!(
                                    "line of effect is blocked by [{}]",
                                    projection.blocking_cell_ids.join(", ")
                                ),
                            )
                        })
                    })
            }
            RpgIrTargetKind::Cell => {
                intent
                    .cell_targets
                    .iter()
                    .enumerate()
                    .find_map(|(index, cell)| {
                        let projection = line_of_effect_projection(
                            &board_index,
                            actor.position(),
                            cell.position,
                        );
                        (!projection.clear).then(|| {
                            rejection(
                                "RPG_LINE_OF_EFFECT_BLOCKED",
                                format!("$.command.intent.cellTargets[{index}]"),
                                format!(
                                    "line of effect is blocked by [{}]",
                                    projection.blocking_cell_ids.join(", ")
                                ),
                            )
                        })
                    })
            }
            RpgIrTargetKind::Area => None,
        }
    }

    fn require_random_source(
        &self,
        source: &dyn RpgRandomSource,
    ) -> Result<(), RpgAutomaticCommandFailure> {
        if source.binding() == &self.encounter.scenario.random_source {
            return Ok(());
        }
        Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_SOURCE_BINDING_MISMATCH",
            "$.randomSource",
            format!(
                "source binding {:?} does not match encounter binding {:?}",
                source.binding(),
                self.encounter.scenario.random_source
            ),
        )))
    }

    #[cfg(test)]
    fn for_test(rules: CompiledRpgRules, state: RpgCapabilityState) -> Self {
        let participant_ids = state
            .entities()
            .map(|entity| entity.id().to_owned())
            .collect::<Vec<_>>();
        let action_ids = rules.action_ids().map(str::to_owned).collect::<Vec<_>>();
        let width = state
            .entities()
            .map(|entity| entity.position().x)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let height = state
            .entities()
            .map(|entity| entity.position().y)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let current_actor_id = participant_ids
            .iter()
            .find(|id| id.as_str() == "hero" || id.as_str() == "actor")
            .cloned()
            .or_else(|| participant_ids.first().cloned())
            .unwrap_or_default();
        let scenario = RpgScenario {
            schema: RpgScenario::schema(),
            play_bundle_id: String::new(),
            board: crate::RpgBoardSetup {
                width,
                height,
                cells: Vec::new(),
            },
            participants: Vec::new(),
            turn: crate::RpgTurnInitialization {
                initiative_order: participant_ids.clone(),
                current_actor_id: current_actor_id.clone(),
                round: 1,
                turn: 1,
            },
            random_source: crate::RpgRandomSourceBinding {
                policy_id: "test.random-policy".to_owned(),
                policy_version: 1,
                source_id: "test.random-source".to_owned(),
                source_version: 1,
            },
        };
        let scenario_fingerprint =
            crate::replay::scenario_fingerprint(&scenario).expect("test scenario fingerprints");
        Self {
            session_binding_id: next_session_binding_id(),
            scenario_fingerprint,
            artifact: None,
            rules,
            state,
            pending: None,
            pending_turn_save: None,
            accepted_random_values: 0,
            encounter: RpgEncounterAuthority {
                scenario,
                turn: crate::RpgTurnState {
                    initiative_order: participant_ids.clone(),
                    current_actor_id,
                    round: 1,
                    turn: 1,
                },
                participant_definitions: participant_ids
                    .iter()
                    .map(|id| (id.clone(), action_ids.clone()))
                    .collect(),
                participant_labels: participant_ids
                    .iter()
                    .map(|id| (id.clone(), id.clone()))
                    .collect(),
                item_definitions: std::collections::BTreeMap::new(),
                effect_definitions: std::collections::BTreeMap::new(),
                log: Vec::new(),
            },
        }
    }
}

fn cell_intent(action_id: &str, actor_id: &str, cell: &crate::RpgCellSetup) -> RpgIntent {
    RpgIntent {
        action_id: action_id.to_owned(),
        actor_id: actor_id.to_owned(),
        target_ids: vec![cell.id.clone()],
        cell_targets: vec![RpgIntentCellTarget {
            id: cell.id.clone(),
            position: cell.position,
        }],
        item_binding: None,
    }
}

fn refreshed_modifiers(events: &[rpg_core::RpgDomainEvent]) -> BTreeSet<(String, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            rpg_core::RpgDomainEvent::ModifierApplied {
                target_id,
                stacking_group,
                ..
            } => Some((target_id.clone(), stacking_group.clone())),
            _ => None,
        })
        .collect()
}

fn turn_save_candidates_for(
    state: &RpgCapabilityState,
    actor_id: &str,
    effect_definitions: &BTreeMap<String, rpg_ir::CompiledEffectDefinition>,
) -> Result<Vec<RpgEffectSaveCandidate>, RpgResolutionRejection> {
    let actor = state.entity(actor_id).ok_or_else(|| {
        rejection(
            "RPG_TURN_ACTOR_UNKNOWN",
            "$.control.actorId",
            format!("turn actor {actor_id} is unavailable"),
        )
    })?;
    let mut candidates = Vec::new();
    for effect in actor.effects() {
        if effect.duration_anchor() != rpg_core::RpgEffectDurationAnchor::TargetTurnEndSave {
            continue;
        }
        let definition = effect_definitions
            .get(effect.definition_id())
            .ok_or_else(|| {
                rejection(
                    "RPG_RUNTIME_EFFECT_DEFINITION_UNKNOWN",
                    "$.control.saves",
                    format!(
                        "active effect {} references unavailable definition {}",
                        effect.instance_id(),
                        effect.definition_id()
                    ),
                )
            })?;
        if definition.definition_version != effect.definition_version()
            || definition.tenure != (rpg_core::RpgEffectTenure::TargetTurnEndSave {})
        {
            return Err(rejection(
                "RPG_RUNTIME_EFFECT_TENURE_MISMATCH",
                "$.control.saves",
                format!(
                    "active effect {} does not match the compiled save-ends tenure",
                    effect.instance_id()
                ),
            ));
        }
        candidates.push(RpgEffectSaveCandidate {
            target_id: actor_id.to_owned(),
            instance_id: effect.instance_id().to_owned(),
            definition_id: effect.definition_id().to_owned(),
            definition_version: effect.definition_version(),
            source_entity_id: effect.source_entity_id().to_owned(),
            request: rpg_core::RpgRandomRequest {
                kind: rpg_core::RpgRandomRequestKind::EffectSave,
                count: 1,
                sides: 20,
                path: String::new(),
                heterogeneous_terms: Vec::new(),
            },
        });
    }
    candidates.sort_by(|left, right| {
        (
            left.definition_id.as_str(),
            left.source_entity_id.as_str(),
            left.instance_id.as_str(),
        )
            .cmp(&(
                right.definition_id.as_str(),
                right.source_entity_id.as_str(),
                right.instance_id.as_str(),
            ))
    });
    if candidates.len() > 32 {
        return Err(rejection(
            "RPG_EFFECT_SAVE_CANDIDATE_LIMIT_EXCEEDED",
            "$.control.saves",
            "one end-turn transition may resolve at most 32 save-ends effects",
        ));
    }
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.request.path = format!("$.control.saves[{index}].roll");
    }
    Ok(candidates)
}

fn append_automatic_turn_saves(
    encounter: &RpgEncounterAuthority,
    staged_state: &mut RpgCapabilityState,
    random: &mut DeterministicRandomStream,
    receipt: &mut RpgResolutionReceipt,
) -> Result<(), RpgResolutionRejection> {
    let candidates = turn_save_candidates_for(
        staged_state,
        encounter.current_actor_id(),
        &encounter.effect_definitions,
    )?;
    for candidate in candidates {
        let Some(roll) = random.take() else {
            let mut rejection = rejection(
                "RPG_RANDOM_EXHAUSTED",
                &candidate.request.path,
                "automatic end-turn effect save requires one d20 value",
            );
            rejection.trace = Box::new(receipt.trace.clone());
            rejection.random_evidence = Box::new(receipt.random_evidence.clone());
            rejection.random_attempted = receipt.random_consumed;
            rejection.random_request = Some(Box::new(candidate.request));
            return Err(rejection);
        };
        if !(1..=20).contains(&roll) {
            let mut rejection = rejection(
                "RPG_RANDOM_VALUE_OUT_OF_RANGE",
                &candidate.request.path,
                format!("save value {roll} is outside 1..=20"),
            );
            rejection.trace = Box::new(receipt.trace.clone());
            rejection.random_evidence = Box::new(receipt.random_evidence.clone());
            rejection.random_attempted = receipt.random_consumed.saturating_add(1);
            return Err(rejection);
        }
        let saved = roll >= 10;
        receipt
            .events
            .push(rpg_core::RpgDomainEvent::EffectSaveResolved {
                target_id: candidate.target_id.clone(),
                instance_id: candidate.instance_id.clone(),
                definition_id: candidate.definition_id.clone(),
                definition_version: candidate.definition_version,
                source_id: candidate.source_entity_id.clone(),
                roll,
                difficulty: 10,
                saved,
            });
        receipt.random_evidence.push(rpg_core::RpgRandomEvidence {
            request: candidate.request.clone(),
            values: vec![roll],
            heterogeneous_values: Vec::new(),
        });
        receipt.random_consumed = receipt.random_consumed.saturating_add(1);
        receipt.trace.push(RpgTraceStep {
            path: candidate.request.path,
            code: "RPG_EFFECT_SAVE_RANDOM_CONSUMED".to_owned(),
            detail: format!("d20={roll}"),
        });
        if saved {
            let removed = staged_state
                .effects_owner()
                .remove_instance(&candidate.target_id, &candidate.instance_id)
                .expect("validated save target remains available")
                .expect("validated save effect remains active");
            receipt
                .events
                .push(rpg_core::RpgDomainEvent::EffectExpired {
                    target_id: candidate.target_id,
                    instance_id: removed.instance_id().to_owned(),
                    definition_id: removed.definition_id().to_owned(),
                    definition_version: removed.definition_version(),
                    source_id: removed.source_entity_id().to_owned(),
                    duration_anchor: removed.duration_anchor(),
                    tenure: encounter
                        .effect_definitions
                        .get(&candidate.definition_id)
                        .map(|definition| definition.tenure)
                        .unwrap_or_else(|| removed.tenure()),
                });
        }
    }
    Ok(())
}

fn append_turn_events(
    rules: &CompiledRpgRules,
    encounter: &RpgEncounterAuthority,
    previous_state: &RpgCapabilityState,
    staged_state: &mut RpgCapabilityState,
    events: &mut Vec<rpg_core::RpgDomainEvent>,
    refreshed_modifiers: BTreeSet<(String, String)>,
    transition_revision: u64,
) -> crate::RpgTurnState {
    let next_turn = encounter.next_turn(staged_state);
    if next_turn.round != encounter.turn.round {
        events.push(rpg_core::RpgDomainEvent::RoundTransitioned {
            previous_round: encounter.turn.round,
            current_round: next_turn.round,
        });
    }
    events.push(rpg_core::RpgDomainEvent::TurnTransitioned {
        previous_actor_id: encounter.turn.current_actor_id.clone(),
        current_actor_id: next_turn.current_actor_id.clone(),
        round: next_turn.round,
        turn: next_turn.turn,
    });
    events.extend(effect_boundary_events(
        &encounter.effect_definitions,
        staged_state,
        transition_revision,
        &encounter.turn.current_actor_id,
        &next_turn.current_actor_id,
        next_turn.round != encounter.turn.round,
    ));
    events.extend(modifier_turn_events(
        previous_state,
        staged_state,
        &refreshed_modifiers,
    ));

    let round_changed = next_turn.round != encounter.turn.round;
    let entity_ids = staged_state
        .entities()
        .map(|entity| entity.id().to_owned())
        .collect::<Vec<_>>();
    let mut resets = BTreeSet::new();
    for budget in rules.activation_budgets() {
        if budget.reset_boundary == RulesetActivationBudgetResetBoundary::RoundStart
            && round_changed
        {
            for entity_id in &entity_ids {
                resets.insert((entity_id.clone(), budget.id.clone(), budget.initial_amount));
            }
        }
        if budget.reset_boundary == RulesetActivationBudgetResetBoundary::OwnerTurnStart {
            resets.insert((
                next_turn.current_actor_id.clone(),
                budget.id.clone(),
                budget.initial_amount,
            ));
        }
    }
    staged_state
        .activation_budgets_owner()
        .reset_activation_count();
    for (entity_id, budget_id, initial_amount) in resets {
        let (previous, current) = staged_state
            .activation_budgets_owner()
            .reset_budget(&entity_id, &budget_id, initial_amount)
            .expect("compiled activation budget exists on every participant");
        events.push(rpg_core::RpgDomainEvent::ActivationBudgetReset {
            entity_id,
            budget_id,
            previous,
            current,
        });
    }
    next_turn
}

fn effect_boundary_events(
    effect_definitions: &BTreeMap<String, rpg_ir::CompiledEffectDefinition>,
    staged_state: &mut RpgCapabilityState,
    transition_revision: u64,
    previous_actor_id: &str,
    current_actor_id: &str,
    round_transitioned: bool,
) -> Vec<rpg_core::RpgDomainEvent> {
    staged_state
        .effects_owner()
        .advance_boundaries(
            transition_revision,
            previous_actor_id,
            current_actor_id,
            round_transitioned,
        )
        .into_iter()
        .map(|change| match change {
            rpg_core::RpgEffectBoundaryChange::Aged {
                target_entity_id,
                effect,
                previous_count,
            } => rpg_core::RpgDomainEvent::EffectDurationChanged {
                target_id: target_entity_id,
                instance_id: effect.instance_id().to_owned(),
                definition_id: effect.definition_id().to_owned(),
                definition_version: effect.definition_version(),
                duration_anchor: effect.duration_anchor(),
                tenure: effect_definitions
                    .get(effect.definition_id())
                    .map(|definition| definition.tenure)
                    .unwrap_or_else(|| effect.tenure()),
                previous_count,
                remaining_count: effect.remaining_count(),
            },
            rpg_core::RpgEffectBoundaryChange::Expired {
                target_entity_id,
                effect,
            } => rpg_core::RpgDomainEvent::EffectExpired {
                target_id: target_entity_id,
                instance_id: effect.instance_id().to_owned(),
                definition_id: effect.definition_id().to_owned(),
                definition_version: effect.definition_version(),
                source_id: effect.source_entity_id().to_owned(),
                duration_anchor: effect.duration_anchor(),
                tenure: effect_definitions
                    .get(effect.definition_id())
                    .map(|definition| definition.tenure)
                    .unwrap_or_else(|| effect.tenure()),
            },
        })
        .collect()
}

fn modifier_turn_events(
    previous_state: &RpgCapabilityState,
    staged_state: &mut RpgCapabilityState,
    refreshed_modifiers: &BTreeSet<(String, String)>,
) -> Vec<rpg_core::RpgDomainEvent> {
    staged_state
        .modifiers_owner()
        .advance_turn(previous_state, refreshed_modifiers)
        .into_iter()
        .map(|change| match change {
            RpgModifierTurnChange::Aged {
                entity_id,
                stacking_group,
                modifier_id,
                remaining_turns,
            } => rpg_core::RpgDomainEvent::ModifierDurationChanged {
                target_id: entity_id,
                modifier_id,
                stacking_group,
                remaining_turns,
            },
            RpgModifierTurnChange::Expired {
                entity_id,
                stacking_group,
                modifier_id,
            } => rpg_core::RpgDomainEvent::ModifierExpired {
                target_id: entity_id,
                modifier_id,
                stacking_group,
            },
        })
        .collect()
}

fn required_random_request(outcome: &RpgCommandOutcome) -> Option<&rpg_core::RpgRandomRequest> {
    let RpgCommandOutcome::Rejected(rejection) = outcome else {
        return None;
    };
    rejection.random_request.as_deref()
}

fn extend_random_values(
    random_values: &mut Vec<u32>,
    request: &rpg_core::RpgRandomRequest,
    source: &mut dyn RpgRandomSource,
) -> Result<(), RpgAutomaticCommandFailure> {
    let count = usize::try_from(request.count).map_err(|_| {
        RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_REQUEST_COUNT_INVALID",
            &request.path,
            "authority random request count exceeds the platform address space",
        ))
    })?;
    if count == 0 || random_values.len().saturating_add(count) > MAXIMUM_AUTOMATIC_RANDOM_VALUES {
        return Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_VALUE_LIMIT_EXCEEDED",
            &request.path,
            "authority random request exceeds the bounded automatic evidence limit",
        )));
    }
    let values = source
        .draw(request)
        .map_err(RpgAutomaticCommandFailure::RandomSource)?;
    if values.len() != count {
        return Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_SOURCE_COUNT_MISMATCH",
            &request.path,
            format!(
                "random source returned {} values for an authority request of {count}",
                values.len()
            ),
        )));
    }
    if random_values_out_of_range(request, &values) {
        return Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_SOURCE_VALUE_OUT_OF_RANGE",
            &request.path,
            "random source returned evidence outside the authority die bounds",
        )));
    }
    random_values.extend(values);
    Ok(())
}

fn random_values_out_of_range(request: &rpg_core::RpgRandomRequest, values: &[u32]) -> bool {
    if request.heterogeneous_terms.is_empty() {
        return values
            .iter()
            .any(|value| *value == 0 || *value > request.sides);
    }
    let mut offset = 0_usize;
    for term in &request.heterogeneous_terms {
        for _ in 0..term.count {
            let Some(value) = values.get(offset) else {
                return true;
            };
            if *value == 0 || *value > term.sides {
                return true;
            }
            offset = offset.saturating_add(1);
        }
    }
    offset != values.len()
}

fn revision_rejection(expected: u64, actual: u64) -> RpgResolutionRejection {
    rejection(
        "RPG_SESSION_REVISION_MISMATCH",
        "$.expectedRevision",
        format!("expected state revision {expected}, but active revision is {actual}"),
    )
}

fn prepend_area_evidence(receipt: &mut RpgResolutionReceipt, event: rpg_core::RpgDomainEvent) {
    if let rpg_core::RpgDomainEvent::AreaTargetsDerived {
        anchor_cell_id,
        included_cell_ids,
        included_participant_ids,
        ..
    } = &event
    {
        receipt.trace.insert(
            0,
            RpgTraceStep {
                path: "$.areaSelection".to_owned(),
                code: "RPG_AREA_TARGETS_DERIVED".to_owned(),
                detail: format!(
                    "anchor {anchor_cell_id}; cells {included_cell_ids:?}; participants {included_participant_ids:?}"
                ),
            },
        );
    }
    receipt.events.insert(0, event);
}

fn unused_random_rejection(remaining: usize) -> RpgResolutionRejection {
    rejection(
        "RPG_RANDOM_EVIDENCE_UNUSED",
        "$.randomValues",
        format!("{remaining} supplied random value(s) were not consumed"),
    )
}

fn encounter_resolution_context(
    scenario: &RpgScenario,
    state: &RpgCapabilityState,
) -> RpgResolutionContext {
    let capabilities_by_position = scenario
        .board
        .cells
        .iter()
        .map(|cell| {
            let mut capability_ids = cell
                .capabilities
                .iter()
                .filter_map(|capability| capability.definition_id.clone())
                .collect::<Vec<_>>();
            capability_ids.sort();
            capability_ids.dedup();
            (cell.position, capability_ids)
        })
        .collect::<BTreeMap<_, _>>();
    let entity_cell_capability_ids = state
        .entities()
        .map(|entity| {
            (
                entity.id().to_owned(),
                capabilities_by_position
                    .get(&entity.position())
                    .cloned()
                    .unwrap_or_default(),
            )
        })
        .collect();
    RpgResolutionContext {
        entity_cell_capability_ids,
    }
}

fn rejection(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> RpgResolutionRejection {
    RpgResolutionRejection {
        code: code.to_owned(),
        path: path.into(),
        message: message.into(),
        trace: Box::new(Vec::new()),
        random_evidence: Box::new(Vec::new()),
        random_attempted: 0,
        random_request: None,
        reaction_request: None,
        unavailable_source: None,
    }
}

#[cfg(test)]
mod tests {
    use rpg_compiler::compile_normalized_rpg_json;
    use rpg_core::{GridPosition, RpgDomainEvent, RpgEntityState, Team};

    use super::*;

    fn reaction_ruleset() -> CompiledRpgRules {
        let source = br#"{
          "schema":{"identity":"asha.rpg.ir","major":3},
          "package":{"id":"session.test","version":"1.0.0"},
          "catalogs":{"resources":["focus"],"capabilities":[
            "capability.random","capability.reactions","capability.resources","capability.vitality"
          ]},
          "requirements":[
            {"kind":"operation","id":"operation.damage","version":2},
            {"kind":"operation","id":"operation.openReaction","version":1},
            {"kind":"capability","id":"capability.random","version":1},
            {"kind":"capability","id":"capability.reactions","version":1},
            {"kind":"capability","id":"capability.resources","version":1},
            {"kind":"capability","id":"capability.vitality","version":1}
          ],
          "actions":[{
            "id":"action.reactive","name":"Reactive strike","sourcePath":"actions/reactive",
            "targets":{"team":"hostile","maximumRange":3,"maximumTargets":1},
            "check":{"kind":"noRoll"},"rollScope":"none",
            "costs":[{"resourceId":"focus","amount":1}],
            "program":{"kind":"atomic","body":{"kind":"sequence","steps":[
              {"kind":"operation","operation":{"kind":"openReaction","reactionId":"reaction.ward","options":[
                {"id":"ward","label":"Raise ward","damageReduction":3}
              ]}},
              {"kind":"operation","operation":{"kind":"damage","parts":[
                {"id":"damage","amount":{"kind":"dice","count":5,"sides":4,"bonus":0},"damageType":"force","tags":[]}
              ]}}
            ]}}
          }]
        }"#;
        compile_normalized_rpg_json(source).expect("reaction rules compiles")
    }

    fn movement_ruleset() -> CompiledRpgRules {
        let source = br#"{
          "schema":{"identity":"asha.rpg.ir","major":3},
          "package":{"id":"movement.test","version":"1.0.0"},
          "catalogs":{"capabilities":["capability.position","capability.vitality"]},
          "requirements":[
            {"kind":"operation","id":"operation.moveToCell","version":1},
            {"kind":"capability","id":"capability.position","version":1},
            {"kind":"capability","id":"capability.vitality","version":1}
          ],
          "actions":[{
            "id":"action.move","name":"Move","sourcePath":"actions/move",
            "targets":{"kind":"cell","team":"any","maximumRange":4,"maximumTargets":1},
            "check":{"kind":"noRoll"},"rollScope":"none","costs":[],
            "program":{"kind":"atomic","body":{"kind":"onCheck","noRoll":{
              "kind":"operation","operation":{"kind":"moveToCell","maximumDistance":4,"provokes":true}
            }}}
          }]
        }"#;
        compile_normalized_rpg_json(source).expect("movement rules compile")
    }

    fn movement_session() -> RpgAuthoritySession {
        let actor = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 0, y: 1 }, 20);
        let target =
            RpgEntityState::new("guardian", Team::enemy(), GridPosition { x: 3, y: 1 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(target);
        let mut session = RpgAuthoritySession::for_test(movement_ruleset(), state);
        session.encounter.scenario.board = crate::RpgBoardSetup {
            width: 5,
            height: 3,
            cells: (0..3)
                .flat_map(|y| {
                    (0..5).map(move |x| {
                        movement_cell(&format!("cell-{x}-{y}"), x, y, (x, y) != (1, 1), 1)
                    })
                })
                .collect(),
        };
        session
    }

    fn movement_cell(
        id: &str,
        x: u32,
        y: u32,
        passable: bool,
        movement_cost: u32,
    ) -> crate::RpgCellSetup {
        crate::RpgCellSetup {
            id: id.to_owned(),
            position: GridPosition { x, y },
            capabilities: vec![crate::RpgCellCapabilitySetup {
                id: "capability.traversal".to_owned(),
                version: 1,
                definition_id: None,
                value: crate::RpgCellCapabilityValue::Traversal {
                    passable,
                    movement_cost,
                },
            }],
        }
    }

    fn movement_command(cell_id: &str, position: GridPosition) -> RpgAuthorityCommand {
        RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.move".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec![cell_id.to_owned()],
                cell_targets: vec![RpgIntentCellTarget {
                    id: cell_id.to_owned(),
                    position,
                }],
                item_binding: None,
            },
            random_values: Vec::new(),
        }
    }

    fn reaction_session() -> RpgAuthoritySession {
        let rules = reaction_ruleset();
        let actor = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 0, y: 0 }, 20)
            .with_resource("focus", 2, 2);
        let target =
            RpgEntityState::new("guardian", Team::enemy(), GridPosition { x: 1, y: 0 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(target);
        RpgAuthoritySession::for_test(rules, state)
    }

    fn living_legality_session(actor_vitality: i32, target_vitality: i32) -> RpgAuthoritySession {
        let actor = RpgEntityState::new(
            "hero",
            Team::ally(),
            GridPosition { x: 0, y: 0 },
            actor_vitality,
        )
        .with_resource("focus", 2, 2);
        let ally = RpgEntityState::new("scout", Team::ally(), GridPosition { x: 0, y: 1 }, 20);
        let target = RpgEntityState::new(
            "guardian",
            Team::enemy(),
            GridPosition { x: 1, y: 0 },
            target_vitality,
        );
        let enemy = RpgEntityState::new("raider", Team::enemy(), GridPosition { x: 1, y: 1 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(ally);
        state.insert_entity(target);
        state.insert_entity(enemy);
        RpgAuthoritySession::for_test(reaction_ruleset(), state)
    }

    fn command() -> RpgAuthorityCommand {
        RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
                cell_targets: Vec::new(),
                item_binding: None,
            },
            random_values: Vec::new(),
        }
    }

    #[test]
    fn movement_projects_only_committable_cells_and_rejects_forged_board_bindings() {
        let session = movement_session();
        let view = session.encounter_view();
        let movement = view
            .actions
            .iter()
            .find(|action| action.definition_id == "action.move")
            .expect("movement action is projected");
        assert!(movement.available);
        let detour = movement
            .options
            .cell_paths
            .iter()
            .find(|path| path.destination_cell_id == "cell-2-1")
            .expect("detour destination is projected");
        assert_eq!(detour.movement_cost, 4);
        assert_eq!(
            detour.cell_ids,
            vec!["cell-0-0", "cell-1-0", "cell-2-0", "cell-2-1"]
        );
        assert!(!movement
            .options
            .cell_paths
            .iter()
            .any(|path| path.destination_cell_id == "cell-1-1"));
        assert!(!movement
            .options
            .cell_paths
            .iter()
            .any(|path| path.destination_cell_id == "cell-3-1"));
        assert!(movement.options.participant_ids.is_empty());

        for path in &movement.options.cell_paths {
            let cell_id = path.destination_cell_id.as_str();
            let position = session
                .scenario()
                .board
                .cells
                .iter()
                .find(|cell| cell.id == cell_id)
                .unwrap()
                .position;
            let mut committable_session = movement_session();
            let outcome = committable_session.submit(movement_command(cell_id, position));
            let RpgCommandOutcome::Accepted(receipt) = outcome else {
                panic!("projected destination {cell_id} must commit: {outcome:?}");
            };
            assert_eq!(receipt.random_consumed, 0);
            assert_eq!(
                committable_session
                    .state()
                    .entity("hero")
                    .unwrap()
                    .position(),
                position
            );
        }

        for (cell_id, position, code) in [
            (
                "cell-2-0",
                GridPosition { x: 3, y: 0 },
                "RPG_INTENT_CELL_BINDING_MISMATCH",
            ),
            (
                "missing",
                GridPosition { x: 2, y: 0 },
                "RPG_INTENT_CELL_UNKNOWN",
            ),
            (
                "cell-3-1",
                GridPosition { x: 3, y: 1 },
                "RPG_MOVEMENT_PATH_UNAVAILABLE",
            ),
            (
                "cell-1-1",
                GridPosition { x: 1, y: 1 },
                "RPG_MOVEMENT_PATH_UNAVAILABLE",
            ),
            (
                "cell-4-0",
                GridPosition { x: 4, y: 0 },
                "RPG_INTENT_TARGET_OUT_OF_RANGE",
            ),
        ] {
            let mut rejected_session = movement_session();
            let RpgCommandOutcome::Rejected(rejected) =
                rejected_session.submit(movement_command(cell_id, position))
            else {
                panic!("{cell_id} must be rejected");
            };
            assert_eq!(rejected.code, code);
            assert_eq!(rejected_session.state().revision(), 0);
            assert_eq!(
                rejected_session.state().entity("hero").unwrap().position(),
                GridPosition { x: 0, y: 1 }
            );
        }
    }

    #[test]
    fn movement_paths_cover_costs_obstacles_occupancy_bounds_and_ties() {
        let session = movement_session();
        let paths = movement_paths(&session.scenario().board, session.state(), "hero", 8);
        let straight = paths
            .iter()
            .find(|path| path.destination_cell_id == "cell-0-0")
            .unwrap();
        assert_eq!(straight.cell_ids, vec!["cell-0-0"]);
        assert_eq!(straight.movement_cost, 1);

        let equal_cost_detour = paths
            .iter()
            .find(|path| path.destination_cell_id == "cell-2-1")
            .unwrap();
        assert_eq!(
            equal_cost_detour.cell_ids,
            vec!["cell-0-0", "cell-1-0", "cell-2-0", "cell-2-1"]
        );
        assert_eq!(equal_cost_detour.movement_cost, 4);

        let around_occupied_cell = paths
            .iter()
            .find(|path| path.destination_cell_id == "cell-4-1")
            .unwrap();
        assert_eq!(
            around_occupied_cell.cell_ids,
            vec!["cell-0-0", "cell-1-0", "cell-2-0", "cell-3-0", "cell-4-0", "cell-4-1",]
        );
        assert!(!around_occupied_cell
            .cell_ids
            .contains(&"cell-3-1".to_owned()));

        let bounded = movement_paths(&session.scenario().board, session.state(), "hero", 3);
        assert!(!bounded
            .iter()
            .any(|path| path.destination_cell_id == "cell-2-1"));

        let mut weighted_board = session.scenario().board.clone();
        let top_exit = weighted_board
            .cells
            .iter_mut()
            .find(|cell| cell.id == "cell-0-0")
            .unwrap();
        top_exit.capabilities[0].value = crate::RpgCellCapabilityValue::Traversal {
            passable: true,
            movement_cost: 2,
        };
        let weighted = movement_paths(&weighted_board, session.state(), "hero", 8);
        let weighted_detour = weighted
            .iter()
            .find(|path| path.destination_cell_id == "cell-2-1")
            .unwrap();
        assert_eq!(
            weighted_detour.cell_ids,
            vec!["cell-0-2", "cell-1-2", "cell-2-2", "cell-2-1"]
        );
        assert_eq!(weighted_detour.movement_cost, 4);

        let mut trapped_board = session.scenario().board.clone();
        for cell_id in ["cell-0-0", "cell-0-2"] {
            let cell = trapped_board
                .cells
                .iter_mut()
                .find(|cell| cell.id == cell_id)
                .unwrap();
            cell.capabilities[0].value = crate::RpgCellCapabilityValue::Traversal {
                passable: false,
                movement_cost: 1,
            };
        }
        assert!(movement_paths(&trapped_board, session.state(), "hero", 8).is_empty());

        let mut default_traversal_board = session.scenario().board.clone();
        default_traversal_board
            .cells
            .iter_mut()
            .find(|cell| cell.id == "cell-0-0")
            .unwrap()
            .capabilities
            .clear();
        let default_paths = movement_paths(&default_traversal_board, session.state(), "hero", 1);
        assert_eq!(default_paths[0].destination_cell_id, "cell-0-0");
        assert_eq!(default_paths[0].movement_cost, 1);
    }

    #[test]
    fn accepted_movement_updates_position_log_and_turn_atomically() {
        let mut session = movement_session();
        let outcome = session.submit(movement_command("cell-2-1", GridPosition { x: 2, y: 1 }));
        let RpgCommandOutcome::Accepted(receipt) = outcome else {
            panic!("legal movement must commit: {outcome:?}");
        };
        assert_eq!(session.state().revision(), 1);
        assert_eq!(
            session.state().entity("hero").unwrap().position(),
            GridPosition { x: 2, y: 1 }
        );
        assert_eq!(session.turn().current_actor_id, "guardian");
        assert!(matches!(
            receipt.events.first(),
            Some(RpgDomainEvent::PositionChanged { current, .. })
                if *current == GridPosition { x: 2, y: 1 }
        ));
        assert!(receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::TurnTransitioned {
                previous_actor_id,
                current_actor_id,
                ..
            } if previous_actor_id == "hero" && current_actor_id == "guardian"
        )));
        let log = &session.encounter_view().log;
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].action_id, "action.move");
    }

    #[test]
    fn reaction_resumes_the_same_atomic_state_and_random_transaction() {
        let mut session = reaction_session();
        let RpgCommandOutcome::AwaitingReaction(pending) = session.submit(command()) else {
            panic!("command must suspend");
        };
        assert_eq!(pending.request.reaction_id, "reaction.ward");
        assert_eq!(session.state().revision(), 0);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .resource("focus")
                .unwrap()
                .current,
            2
        );

        let invalid = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("missing".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        assert!(matches!(invalid, RpgCommandOutcome::Rejected(_)));
        assert_eq!(session.state().revision(), 0);

        let accepted = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        let RpgCommandOutcome::Accepted(receipt) = accepted else {
            panic!("valid reaction must resume and commit: {accepted:?}");
        };
        assert_eq!(receipt.random_consumed, 5);
        assert!(receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::DamagePacketApplied {
                bounded_vitality_delta: 7,
                ..
            }
        )));
        assert_eq!(session.state().revision(), 1);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .resource("focus")
                .unwrap()
                .current,
            1
        );
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .vitality()
                .current,
            13
        );
        assert!(session.pending_reaction().is_none());
    }

    #[test]
    fn rejected_reaction_evidence_does_not_accumulate_between_retries() {
        let mut session = reaction_session();
        let RpgCommandOutcome::AwaitingReaction(_) = session.submit(command()) else {
            panic!("command must suspend");
        };

        let insufficient = RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2],
        };
        let first = session.react(insufficient.clone());
        let second = session.react(insufficient);

        assert_eq!(first, second);
        let RpgCommandOutcome::Rejected(rejection) = first else {
            panic!("insufficient evidence must reject");
        };
        assert_eq!(rejection.code, "RPG_RANDOM_EXHAUSTED");
        assert_eq!(rejection.random_attempted, 0);
        assert!(session.pending.as_ref().unwrap().random_values.is_empty());
        assert_eq!(session.state().revision(), 0);

        let accepted = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        assert!(matches!(accepted, RpgCommandOutcome::Accepted(_)));
        assert_eq!(session.state().revision(), 1);
    }

    #[test]
    fn inactive_current_actor_is_unavailable_and_cannot_submit() {
        let mut session = living_legality_session(0, 20);
        let before_state = session.state().clone();
        let before_turn = session.turn().clone();
        let view = session.encounter_view();
        assert_eq!(view.actions.len(), 1);
        assert!(!view.actions[0].available);
        assert_eq!(
            view.actions[0].unavailable.as_ref().unwrap().code,
            "RPG_TURN_ACTOR_INACTIVE"
        );

        let outcome = session.submit(command());
        let RpgCommandOutcome::Rejected(rejection) = outcome else {
            panic!("inactive actor must be rejected: {outcome:?}");
        };
        assert_eq!(rejection.code, "RPG_TURN_ACTOR_INACTIVE");
        assert_eq!(rejection.path, "$.command.intent.actorId");
        assert_eq!(session.state(), &before_state);
        assert_eq!(session.turn(), &before_turn);
        assert!(session.encounter.log.is_empty());
        assert!(session.pending_reaction().is_none());
    }

    #[test]
    fn target_omitted_from_living_candidates_cannot_be_submitted() {
        let mut session = living_legality_session(20, 0);
        let before_state = session.state().clone();
        let before_turn = session.turn().clone();
        let view = session.encounter_view();
        assert_eq!(view.actions.len(), 1);
        assert!(!view.actions[0]
            .options
            .participant_ids
            .contains(&"guardian".to_owned()));
        assert!(view.actions[0]
            .options
            .participant_ids
            .contains(&"raider".to_owned()));

        let outcome = session.submit(command());
        let RpgCommandOutcome::Rejected(rejection) = outcome else {
            panic!("inactive target must be rejected: {outcome:?}");
        };
        assert_eq!(rejection.code, "RPG_INTENT_TARGET_INACTIVE");
        assert_eq!(rejection.path, "$.command.intent.targetIds[0]");
        assert_eq!(session.state(), &before_state);
        assert_eq!(session.turn(), &before_turn);
        assert!(session.encounter.log.is_empty());
        assert!(session.pending_reaction().is_none());
    }
}
