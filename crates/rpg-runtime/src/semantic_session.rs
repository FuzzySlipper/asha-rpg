use std::{collections::BTreeSet, fmt};

use rpg_compiler::{CompiledRpgRuleset, CompiledRulesetBundle};
use rpg_core::{
    DeterministicRandomStream, RpgCapabilityState, RpgIntent, RpgModifierTurnChange,
    RpgRandomEvidence, RpgReactionDecision, RpgReactionRequest, RpgResolutionReceipt,
    RpgResolutionRejection, RpgTraceStep,
};
use rpg_ir::CompiledRulesetArtifact;
use serde::{Deserialize, Serialize};

use crate::encounter::{
    action_view, build_encounter, encounter_outcome, living_intent_rejection, participant_view,
    random_failure, runtime_board_rejection, RpgActionProposal, RpgEncounterAuthority,
    RpgEncounterOutcomeView, RpgEncounterSetup, RpgEncounterSetupFailure, RpgEncounterView,
    RpgRandomSource, RpgRandomSourceFailure, RpgReactionProposal, RpgSchemaIdentity,
    RpgTurnControl, RpgTurnControlProposal, RpgTurnControlView, RPG_ENCOUNTER_VIEW_SCHEMA_ID,
    RPG_ENCOUNTER_VIEW_SCHEMA_VERSION,
};
use crate::{RpgReplayEntry, RpgReplayFailure};

const MAXIMUM_AUTOMATIC_RANDOM_REQUESTS: usize = 64;
const MAXIMUM_AUTOMATIC_RANDOM_VALUES: usize = 4_096;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnControlReceipt {
    pub control: RpgTurnControl,
    pub actor_id: String,
    pub events: Vec<rpg_core::RpgDomainEvent>,
    pub state_revision: u64,
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
    pub(crate) random_values: Vec<u32>,
    pub(crate) pending: RpgPendingReaction,
}

/// Owner of one compiled artifact's persistent capability state and staged
/// reaction transaction.
#[derive(Debug, Clone)]
pub struct RpgAuthoritySession {
    pub(crate) artifact: Option<CompiledRulesetArtifact>,
    pub(crate) ruleset: CompiledRpgRuleset,
    pub(crate) state: RpgCapabilityState,
    pub(crate) pending: Option<PendingTransaction>,
    pub(crate) accepted_random_values: u64,
    pub(crate) encounter: RpgEncounterAuthority,
}

impl RpgAuthoritySession {
    pub fn from_setup(
        bundle: CompiledRulesetBundle,
        setup: RpgEncounterSetup,
    ) -> Result<Self, RpgEncounterSetupFailure> {
        let (state, encounter) = build_encounter(&bundle, setup)?;
        Ok(Self {
            artifact: Some(bundle.artifact().clone()),
            ruleset: bundle.ruleset().clone(),
            state,
            pending: None,
            accepted_random_values: 0,
            encounter,
        })
    }

    pub fn artifact(&self) -> Option<&CompiledRulesetArtifact> {
        self.artifact.as_ref()
    }

    pub fn ruleset(&self) -> &CompiledRpgRuleset {
        &self.ruleset
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

    pub fn setup(&self) -> &RpgEncounterSetup {
        &self.encounter.setup
    }

    pub fn turn(&self) -> &crate::RpgTurnState {
        &self.encounter.turn
    }

    pub fn encounter_view(&self) -> RpgEncounterView {
        let actor_id = self.encounter.current_actor_id();
        let action_definitions = self
            .encounter
            .participant_definitions
            .get(actor_id)
            .cloned()
            .unwrap_or_default();
        let actions = self
            .ruleset
            .actions()
            .filter(|action| action_definitions.contains(&action.id))
            .map(|action| {
                let actor_intent = RpgIntent {
                    action_id: action.id.clone(),
                    actor_id: actor_id.to_owned(),
                    target_ids: Vec::new(),
                };
                let actor_rejection = living_intent_rejection(&self.state, &actor_intent);
                let candidates = self
                    .ruleset
                    .candidate_ids(&self.state, actor_id, &action.id)
                    .unwrap_or_default()
                    .into_iter();
                let mut first_rejection = actor_rejection;
                let legal_candidates = candidates
                    .filter(|target_id| {
                        let intent = RpgIntent {
                            action_id: action.id.clone(),
                            actor_id: actor_id.to_owned(),
                            target_ids: vec![target_id.clone()],
                        };
                        if let Some(rejection) = living_intent_rejection(&self.state, &intent) {
                            if first_rejection.is_none() {
                                first_rejection = Some(rejection);
                            }
                            return false;
                        }
                        match self.ruleset.preflight(&self.state, &intent) {
                            Ok(()) => true,
                            Err(rejection) => {
                                if first_rejection.is_none() {
                                    first_rejection = Some(rejection);
                                }
                                false
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                let unavailable = legal_candidates
                    .is_empty()
                    .then_some(first_rejection)
                    .flatten();
                action_view(action, legal_candidates, unavailable)
            })
            .collect();
        let participants = self
            .state
            .entities()
            .map(|entity| {
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
                )
            })
            .collect();
        let control_unavailable = if self.pending.is_some() {
            Some(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.control",
                "resolve the pending reaction before ending the turn",
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
            artifact_id: self
                .artifact
                .as_ref()
                .map(|artifact| artifact.artifact_id.clone())
                .unwrap_or_default(),
            state_revision: self.state.revision(),
            accepted_random_position: self.accepted_random_values,
            random_source: self.encounter.setup.random_source.clone(),
            board: self.encounter.setup.board.clone(),
            participants,
            turn: self.encounter.turn.clone(),
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
            log: self.encounter.log.clone(),
            outcome: encounter_outcome(&self.state),
        }
    }

    pub(crate) fn submit(&mut self, command: RpgAuthorityCommand) -> RpgCommandOutcome {
        if self.pending.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.command",
                "resolve the pending reaction before submitting another command",
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
        if let Some(rejection) = living_intent_rejection(&self.state, &command.intent) {
            return RpgCommandOutcome::Rejected(rejection);
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

        let mut staged_state = self.state.clone();
        let mut random = DeterministicRandomStream::new(command.random_values.clone());
        match self
            .ruleset
            .resolve(&mut staged_state, &mut random, &command.intent)
        {
            Ok(mut receipt) => {
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                if let Some(rejection) =
                    runtime_board_rejection(&self.encounter.setup.board, &staged_state)
                {
                    return RpgCommandOutcome::Rejected(rejection);
                }
                let advances_turn = matches!(
                    encounter_outcome(&staged_state),
                    RpgEncounterOutcomeView::InProgress
                );
                if advances_turn {
                    append_modifier_turn_events(&self.state, &mut staged_state, &mut receipt);
                }
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                self.encounter.record(&receipt);
                if advances_turn {
                    self.encounter.advance_turn(&self.state);
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
                    random_values: command.random_values,
                    pending: pending.clone(),
                });
                RpgCommandOutcome::AwaitingReaction(pending)
            }
        }
    }

    pub(crate) fn react(&mut self, command: RpgReactionCommand) -> RpgCommandOutcome {
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
        match self.ruleset.resolve_with_reaction_decision(
            &mut staged_state,
            &mut random,
            &transaction.intent,
            &decision,
        ) {
            Ok(mut receipt) => {
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                if let Some(rejection) =
                    runtime_board_rejection(&self.encounter.setup.board, &staged_state)
                {
                    return RpgCommandOutcome::Rejected(rejection);
                }
                let advances_turn = matches!(
                    encounter_outcome(&staged_state),
                    RpgEncounterOutcomeView::InProgress
                );
                if advances_turn {
                    append_modifier_turn_events(&self.state, &mut staged_state, &mut receipt);
                }
                self.pending = None;
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                self.encounter.record(&receipt);
                if advances_turn {
                    self.encounter.advance_turn(&self.state);
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

        let mut staged_state = self.state.clone();
        let events = modifier_turn_events(&self.state, &mut staged_state, &BTreeSet::new());
        let state_revision = staged_state.advance_revision();
        let receipt = RpgTurnControlReceipt {
            control: command.control,
            actor_id: command.actor_id,
            events,
            state_revision,
        };
        self.state = staged_state;
        self.encounter.record_control(&receipt);
        self.encounter.advance_turn(&self.state);
        RpgCommandOutcome::ControlAccepted(receipt)
    }

    pub fn submit_with_random_source_recorded(
        &mut self,
        proposal: RpgActionProposal,
        source: &mut dyn RpgRandomSource,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgAutomaticCommandFailure> {
        self.require_random_source(source)?;
        let baseline = self.clone();
        let mut random_values = Vec::new();
        for _ in 0..MAXIMUM_AUTOMATIC_RANDOM_REQUESTS {
            let mut probe = baseline.clone();
            let command = RpgAuthorityCommand {
                expected_revision: proposal.expected_revision,
                intent: RpgIntent {
                    action_id: proposal.action_id.clone(),
                    actor_id: proposal.actor_id.clone(),
                    target_ids: proposal.target_ids.clone(),
                },
                random_values: random_values.clone(),
            };
            let outcome = probe.submit(command.clone());
            let Some(request) = required_random_request(&outcome) else {
                return self
                    .submit_recorded(command)
                    .map_err(RpgAutomaticCommandFailure::Replay);
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
        let baseline = self.clone();
        let mut additional_random_values = Vec::new();
        for _ in 0..MAXIMUM_AUTOMATIC_RANDOM_REQUESTS {
            let mut probe = baseline.clone();
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
        })
    }

    fn require_random_source(
        &self,
        source: &dyn RpgRandomSource,
    ) -> Result<(), RpgAutomaticCommandFailure> {
        if source.binding() == &self.encounter.setup.random_source {
            return Ok(());
        }
        Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_SOURCE_BINDING_MISMATCH",
            "$.randomSource",
            format!(
                "source binding {:?} does not match encounter binding {:?}",
                source.binding(),
                self.encounter.setup.random_source
            ),
        )))
    }

    #[cfg(test)]
    fn for_test(ruleset: CompiledRpgRuleset, state: RpgCapabilityState) -> Self {
        let participant_ids = state
            .entities()
            .map(|entity| entity.id().to_owned())
            .collect::<Vec<_>>();
        let action_ids = ruleset.action_ids().map(str::to_owned).collect::<Vec<_>>();
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
        let setup = RpgEncounterSetup {
            schema: RpgEncounterSetup::schema(),
            artifact_id: String::new(),
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
        Self {
            artifact: None,
            ruleset,
            state,
            pending: None,
            accepted_random_values: 0,
            encounter: RpgEncounterAuthority {
                setup,
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
                log: Vec::new(),
            },
        }
    }
}

fn append_modifier_turn_events(
    previous_state: &RpgCapabilityState,
    staged_state: &mut RpgCapabilityState,
    receipt: &mut RpgResolutionReceipt,
) {
    let refreshed_modifiers = receipt
        .events
        .iter()
        .filter_map(|event| match event {
            rpg_core::RpgDomainEvent::ModifierApplied {
                target_id,
                stacking_group,
                ..
            } => Some((target_id.clone(), stacking_group.clone())),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    receipt.events.extend(modifier_turn_events(
        previous_state,
        staged_state,
        &refreshed_modifiers,
    ));
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
    if values
        .iter()
        .any(|value| *value == 0 || *value > request.sides)
    {
        return Err(RpgAutomaticCommandFailure::RandomSource(random_failure(
            "RPG_RANDOM_SOURCE_VALUE_OUT_OF_RANGE",
            &request.path,
            "random source returned evidence outside the authority die bounds",
        )));
    }
    random_values.extend(values);
    Ok(())
}

fn revision_rejection(expected: u64, actual: u64) -> RpgResolutionRejection {
    rejection(
        "RPG_SESSION_REVISION_MISMATCH",
        "$.expectedRevision",
        format!("expected state revision {expected}, but active revision is {actual}"),
    )
}

fn unused_random_rejection(remaining: usize) -> RpgResolutionRejection {
    rejection(
        "RPG_RANDOM_EVIDENCE_UNUSED",
        "$.randomValues",
        format!("{remaining} supplied random value(s) were not consumed"),
    )
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
    }
}

#[cfg(test)]
mod tests {
    use rpg_compiler::compile_normalized_rpg_json;
    use rpg_core::{GridPosition, RpgDomainEvent, RpgEntityState, Team};

    use super::*;

    fn reaction_ruleset() -> CompiledRpgRuleset {
        let source = br#"{
          "schema":{"identity":"asha.rpg.ir","major":1},
          "package":{"id":"session.test","version":"1.0.0"},
          "catalogs":{"resources":["focus"],"capabilities":[
            "capability.random","capability.reactions","capability.resources","capability.vitality"
          ]},
          "requirements":[
            {"kind":"operation","id":"operation.damage","version":1},
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
              {"kind":"operation","operation":{"kind":"damage","amount":{"kind":"dice","count":5,"sides":4,"bonus":0},"damageType":"force"}}
            ]}}
          }]
        }"#;
        compile_normalized_rpg_json(source).expect("reaction ruleset compiles")
    }

    fn reaction_session() -> RpgAuthoritySession {
        let ruleset = reaction_ruleset();
        let actor = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 0, y: 0 }, 20)
            .with_resource("focus", 2, 2);
        let target =
            RpgEntityState::new("guardian", Team::enemy(), GridPosition { x: 1, y: 0 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(target);
        RpgAuthoritySession::for_test(ruleset, state)
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
            },
            random_values: Vec::new(),
        }
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
        assert!(receipt
            .events
            .iter()
            .any(|event| matches!(event, RpgDomainEvent::DamageApplied { amount: 7, .. })));
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
