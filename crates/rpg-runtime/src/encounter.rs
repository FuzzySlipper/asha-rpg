use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use rpg_compiler::{CompiledPlayBundle, CompiledRpgAction};
use rpg_core::{
    ActiveRpgModifier, BoundedValue, GridPosition, RpgCapabilityState, RpgEntityState, RpgIntent,
    RpgRandomRequest, RpgReactionRequest, RpgResolutionRejection, RpgTeamId,
    MAXIMUM_RPG_MODIFIER_TURNS,
};
use rpg_ir::{MaterializedContentDefinitionKind, MaterializedContentVisibility, RulesetValueKind};
use serde::{Deserialize, Serialize};

pub const RPG_SCENARIO_SCHEMA_ID: &str = "asha.rpg.scenario";
pub const RPG_SCENARIO_SCHEMA_VERSION: u32 = 1;
pub const RPG_ENCOUNTER_VIEW_SCHEMA_ID: &str = "asha.rpg.encounter.view";
pub const RPG_ENCOUNTER_VIEW_SCHEMA_VERSION: u32 = 2;
pub const RPG_END_TURN_CONTROL_ID: &str = "control.end-turn";

const MAXIMUM_BOARD_EXTENT: u32 = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgSchemaIdentity {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScenario {
    pub schema: RpgSchemaIdentity,
    pub play_bundle_id: String,
    pub board: RpgBoardSetup,
    pub participants: Vec<RpgParticipantSetup>,
    pub turn: RpgTurnInitialization,
    pub random_source: RpgRandomSourceBinding,
}

impl RpgScenario {
    pub fn schema() -> RpgSchemaIdentity {
        RpgSchemaIdentity {
            id: RPG_SCENARIO_SCHEMA_ID.to_owned(),
            version: RPG_SCENARIO_SCHEMA_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgBoardSetup {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<RpgCellSetup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgCellSetup {
    pub id: String,
    pub position: GridPosition,
    pub capabilities: Vec<RpgCellCapabilitySetup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgCellCapabilitySetup {
    pub id: String,
    pub version: u32,
    pub definition_id: Option<String>,
    pub value: RpgCellCapabilityValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgCellCapabilityValue {
    Traversal { passable: bool, movement_cost: u32 },
    Flag { value: bool },
    Integer { value: i32 },
    Identifier { value_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgParticipantSetup {
    pub id: String,
    pub label: String,
    pub team_id: RpgTeamId,
    pub position: GridPosition,
    pub definition_ids: Vec<String>,
    pub capabilities: Vec<RpgInitialCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "owner",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgInitialCapability {
    Vitality {
        value: BoundedValue,
    },
    Stat {
        id: String,
        value: i32,
    },
    Defense {
        id: String,
        value: i32,
    },
    Resource {
        id: String,
        value: BoundedValue,
    },
    Modifier {
        stacking_group: String,
        id: String,
        value: i32,
        remaining_turns: u32,
    },
}

impl RpgInitialCapability {
    fn owner_id(&self) -> &'static str {
        match self {
            Self::Vitality { .. } => "capability.vitality",
            Self::Stat { .. } => "capability.stats",
            Self::Defense { .. } => "capability.defenses",
            Self::Resource { .. } => "capability.resources",
            Self::Modifier { .. } => "capability.modifiers",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnInitialization {
    pub initiative_order: Vec<String>,
    pub current_actor_id: String,
    pub round: u64,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomSourceBinding {
    pub policy_id: String,
    pub policy_version: u32,
    pub source_id: String,
    pub source_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScenarioDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScenarioFailure {
    pub diagnostics: Vec<RpgScenarioDiagnostic>,
}

impl fmt::Display for RpgScenarioFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            self.diagnostics
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .unwrap_or("encounter scenario failed"),
        )
    }
}

impl std::error::Error for RpgScenarioFailure {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnState {
    pub initiative_order: Vec<String>,
    pub current_actor_id: String,
    pub round: u64,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgEncounterLogEntry {
    pub sequence: u64,
    pub state_revision: u64,
    pub actor_id: String,
    pub action_id: String,
    pub events: Vec<rpg_core::RpgDomainEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgNamedIntegerView {
    pub id: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgNamedBoundedView {
    pub id: String,
    pub value: BoundedValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgModifierView {
    pub stacking_group: String,
    pub id: String,
    pub value: i32,
    pub remaining_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgParticipantView {
    pub id: String,
    pub label: String,
    pub team_id: RpgTeamId,
    pub position: GridPosition,
    pub definition_ids: Vec<String>,
    pub vitality: BoundedValue,
    pub stats: Vec<RpgNamedIntegerView>,
    pub defenses: Vec<RpgNamedIntegerView>,
    pub resources: Vec<RpgNamedBoundedView>,
    pub modifiers: Vec<RpgModifierView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionOptionsView {
    pub participant_ids: Vec<String>,
    pub cell_ids: Vec<String>,
    pub area_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionView {
    pub definition_id: String,
    pub label: String,
    pub available: bool,
    pub unavailable: Option<RpgResolutionRejection>,
    pub maximum_targets: u32,
    pub options: RpgActionOptionsView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgTurnControl {
    EndTurn,
}

impl RpgTurnControl {
    pub fn id(&self) -> &'static str {
        match self {
            Self::EndTurn => RPG_END_TURN_CONTROL_ID,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnControlView {
    pub control: RpgTurnControl,
    pub label: String,
    pub available: bool,
    pub unavailable: Option<RpgResolutionRejection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgEncounterOutcomeView {
    InProgress,
    Completed { winning_team_ids: Vec<RpgTeamId> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgEncounterView {
    pub schema: RpgSchemaIdentity,
    pub artifact_id: String,
    pub state_revision: u64,
    pub accepted_random_position: u64,
    pub random_source: RpgRandomSourceBinding,
    pub board: RpgBoardSetup,
    pub participants: Vec<RpgParticipantView>,
    pub turn: RpgTurnState,
    pub actions: Vec<RpgActionView>,
    pub controls: Vec<RpgTurnControlView>,
    pub pending_reaction: Option<RpgReactionRequest>,
    pub log: Vec<RpgEncounterLogEntry>,
    pub outcome: RpgEncounterOutcomeView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionProposal {
    pub expected_revision: u64,
    pub action_id: String,
    pub actor_id: String,
    pub target_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionProposal {
    pub expected_revision: u64,
    pub reaction_id: String,
    pub option_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTurnControlProposal {
    pub expected_revision: u64,
    pub actor_id: String,
    pub control: RpgTurnControl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomSourceFailure {
    pub code: String,
    pub path: String,
    pub message: String,
    pub expected_request: Option<Box<RpgRandomRequest>>,
    pub actual_request: Option<Box<RpgRandomRequest>>,
}

impl fmt::Display for RpgRandomSourceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RpgRandomSourceFailure {}

pub trait RpgRandomSource: Send {
    fn binding(&self) -> &RpgRandomSourceBinding;

    fn draw(&mut self, request: &RpgRandomRequest) -> Result<Vec<u32>, RpgRandomSourceFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRollTapeEntry {
    pub request: RpgRandomRequest,
    pub values: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct RpgRollTapeSource {
    binding: RpgRandomSourceBinding,
    entries: VecDeque<RpgRollTapeEntry>,
    consumed_entries: u64,
    consumed_values: u64,
}

impl RpgRollTapeSource {
    pub fn new(
        binding: RpgRandomSourceBinding,
        entries: impl IntoIterator<Item = RpgRollTapeEntry>,
    ) -> Self {
        Self {
            binding,
            entries: entries.into_iter().collect(),
            consumed_entries: 0,
            consumed_values: 0,
        }
    }

    pub fn remaining_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn consumed_entries(&self) -> u64 {
        self.consumed_entries
    }

    pub fn consumed_values(&self) -> u64 {
        self.consumed_values
    }

    pub fn require_exhausted(&self) -> Result<(), RpgRandomSourceFailure> {
        if self.entries.is_empty() {
            return Ok(());
        }
        Err(random_failure(
            "RPG_RANDOM_TAPE_UNUSED_EVIDENCE",
            "$.rollTape",
            format!(
                "{} roll-tape request entrie(s) were not consumed",
                self.entries.len()
            ),
        ))
    }
}

impl RpgRandomSource for RpgRollTapeSource {
    fn binding(&self) -> &RpgRandomSourceBinding {
        &self.binding
    }

    fn draw(&mut self, request: &RpgRandomRequest) -> Result<Vec<u32>, RpgRandomSourceFailure> {
        let Some(entry) = self.entries.front() else {
            return Err(random_failure(
                "RPG_RANDOM_TAPE_EXHAUSTED",
                &request.path,
                format!(
                    "authority requested {}d{}, but the bounded roll tape is exhausted",
                    request.count, request.sides
                ),
            ));
        };
        if &entry.request != request {
            let mut failure = random_failure(
                "RPG_RANDOM_TAPE_REQUEST_ORDER_MISMATCH",
                &request.path,
                "the next roll-tape request does not match the authority request",
            );
            failure.expected_request = Some(Box::new(entry.request.clone()));
            failure.actual_request = Some(Box::new(request.clone()));
            return Err(failure);
        }
        let count = usize::try_from(request.count).map_err(|_| {
            random_failure(
                "RPG_RANDOM_REQUEST_COUNT_INVALID",
                &request.path,
                "authority random request count exceeds the platform address space",
            )
        })?;
        if entry.values.len() > count {
            return Err(random_failure(
                "RPG_RANDOM_TAPE_UNUSED_EVIDENCE",
                &request.path,
                format!(
                    "roll-tape entry contains {} value(s), but authority requested {count}",
                    entry.values.len()
                ),
            ));
        }
        if entry.values.len() < count {
            return Err(random_failure(
                "RPG_RANDOM_TAPE_EXHAUSTED",
                &request.path,
                format!(
                    "roll-tape entry contains {} value(s), but authority requested {count}",
                    entry.values.len()
                ),
            ));
        }
        if let Some((index, value)) = entry
            .values
            .iter()
            .enumerate()
            .find(|(_, value)| **value == 0 || **value > request.sides)
        {
            return Err(random_failure(
                "RPG_RANDOM_TAPE_VALUE_OUT_OF_RANGE",
                &request.path,
                format!(
                    "roll-tape value {value} at offset {index} is outside 1..={}",
                    request.sides
                ),
            ));
        }
        let entry = self
            .entries
            .pop_front()
            .expect("front entry remains available after validation");
        self.consumed_entries = self.consumed_entries.saturating_add(1);
        self.consumed_values = self
            .consumed_values
            .saturating_add(u64::from(request.count));
        Ok(entry.values)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RpgEncounterAuthority {
    pub(crate) scenario: RpgScenario,
    pub(crate) turn: RpgTurnState,
    pub(crate) participant_definitions: BTreeMap<String, Vec<String>>,
    pub(crate) participant_labels: BTreeMap<String, String>,
    pub(crate) log: Vec<RpgEncounterLogEntry>,
}

impl RpgEncounterAuthority {
    pub(crate) fn current_actor_id(&self) -> &str {
        &self.turn.current_actor_id
    }

    pub(crate) fn advance_turn(&mut self, state: &RpgCapabilityState) {
        if self.turn.initiative_order.is_empty() {
            return;
        }
        let current = self
            .turn
            .initiative_order
            .iter()
            .position(|id| id == &self.turn.current_actor_id)
            .unwrap_or(0);
        for offset in 1..=self.turn.initiative_order.len() {
            let next = (current + offset) % self.turn.initiative_order.len();
            let participant_id = &self.turn.initiative_order[next];
            let active = state
                .entity(participant_id)
                .map(|entity| entity.vitality().current > 0)
                .unwrap_or(false);
            if active {
                if next <= current {
                    self.turn.round = self.turn.round.saturating_add(1);
                }
                self.turn.turn = self.turn.turn.saturating_add(1);
                self.turn.current_actor_id = participant_id.clone();
                return;
            }
        }
    }

    pub(crate) fn record(&mut self, receipt: &rpg_core::RpgResolutionReceipt) {
        let sequence = u64::try_from(self.log.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        self.log.push(RpgEncounterLogEntry {
            sequence,
            state_revision: receipt.state_revision,
            actor_id: receipt.actor_id.clone(),
            action_id: receipt.action_id.clone(),
            events: receipt.events.clone(),
        });
    }

    pub(crate) fn record_control(&mut self, receipt: &crate::RpgTurnControlReceipt) {
        let sequence = u64::try_from(self.log.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        self.log.push(RpgEncounterLogEntry {
            sequence,
            state_revision: receipt.state_revision,
            actor_id: receipt.actor_id.clone(),
            action_id: receipt.control.id().to_owned(),
            events: receipt.events.clone(),
        });
    }
}

pub(crate) fn build_encounter(
    bundle: &CompiledPlayBundle,
    scenario: RpgScenario,
) -> Result<(RpgCapabilityState, RpgEncounterAuthority), RpgScenarioFailure> {
    let diagnostics = validate_scenario(bundle, &scenario);
    if !diagnostics.is_empty() {
        return Err(RpgScenarioFailure { diagnostics });
    }

    let mut state = RpgCapabilityState::default();
    for participant in &scenario.participants {
        let vitality = participant
            .capabilities
            .iter()
            .find_map(|capability| match capability {
                RpgInitialCapability::Vitality { value } => Some(*value),
                _ => None,
            })
            .expect("validated participant has vitality");
        let mut entity = RpgEntityState::restore(
            participant.id.clone(),
            participant.team_id.clone(),
            participant.position,
            vitality,
        )
        .expect("validated participant state restores");
        for capability in &participant.capabilities {
            match capability {
                RpgInitialCapability::Vitality { .. } => {}
                RpgInitialCapability::Stat { id, value } => entity
                    .restore_stat(id.clone(), *value)
                    .expect("validated stat restores"),
                RpgInitialCapability::Defense { id, value } => entity
                    .restore_defense(id.clone(), *value)
                    .expect("validated defense restores"),
                RpgInitialCapability::Resource { id, value } => entity
                    .restore_resource(id.clone(), *value)
                    .expect("validated resource restores"),
                RpgInitialCapability::Modifier {
                    stacking_group,
                    id,
                    value,
                    remaining_turns,
                } => entity
                    .restore_modifier(
                        stacking_group.clone(),
                        ActiveRpgModifier::restore(id.clone(), *value, *remaining_turns)
                            .expect("validated modifier restores"),
                    )
                    .expect("validated modifier restores"),
            }
        }
        state.insert_entity(entity);
    }

    let authority = RpgEncounterAuthority {
        turn: RpgTurnState {
            initiative_order: scenario.turn.initiative_order.clone(),
            current_actor_id: scenario.turn.current_actor_id.clone(),
            round: scenario.turn.round,
            turn: scenario.turn.turn,
        },
        participant_definitions: scenario
            .participants
            .iter()
            .map(|participant| (participant.id.clone(), participant.definition_ids.clone()))
            .collect(),
        participant_labels: scenario
            .participants
            .iter()
            .map(|participant| (participant.id.clone(), participant.label.clone()))
            .collect(),
        scenario,
        log: Vec::new(),
    };
    Ok((state, authority))
}

fn validate_scenario(
    bundle: &CompiledPlayBundle,
    scenario: &RpgScenario,
) -> Vec<RpgScenarioDiagnostic> {
    let mut diagnostics = Vec::new();
    if scenario.schema != RpgScenario::schema() {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_SCHEMA_UNSUPPORTED",
            "$.schema",
            format!(
                "expected {}@{}",
                RPG_SCENARIO_SCHEMA_ID, RPG_SCENARIO_SCHEMA_VERSION
            ),
        ));
    }
    if scenario.play_bundle_id != bundle.artifact().artifact_id {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_PLAY_BUNDLE_MISMATCH",
            "$.playBundleId",
            format!("expected PlayBundle {}", bundle.artifact().artifact_id),
        ));
    }
    validate_binding(&scenario.random_source, &mut diagnostics);
    validate_board(bundle, &scenario.board, &mut diagnostics);

    let definition_kinds = bundle
        .artifact()
        .materialized_definitions
        .iter()
        .map(|definition| {
            (
                definition.id.as_str(),
                (definition.kind, definition.visibility),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let action_ids = bundle.rules().action_ids().collect::<BTreeSet<_>>();
    let required_capabilities = bundle
        .rules()
        .required_capabilities()
        .map(|(id, _)| id)
        .collect::<BTreeSet<_>>();
    let numeric_domains = bundle
        .artifact()
        .ruleset
        .provides
        .numeric_domains
        .iter()
        .map(|domain| (domain.id.as_str(), (domain.minimum, domain.maximum)))
        .collect::<BTreeMap<_, _>>();
    let ruleset_values = bundle
        .artifact()
        .ruleset
        .provides
        .values
        .iter()
        .filter_map(|value| {
            numeric_domains
                .get(value.numeric_domain_id.as_str())
                .map(|bounds| ((value.kind, value.id.as_str()), *bounds))
        })
        .collect::<BTreeMap<_, _>>();
    let content_values = bundle
        .artifact()
        .materialized_definitions
        .iter()
        .filter(|definition| definition.kind == MaterializedContentDefinitionKind::Support)
        .filter_map(|definition| {
            let catalog = definition.semantic.get("catalog")?.as_str()?;
            let id = definition.semantic.get("id")?.as_str()?;
            Some((catalog, id))
        })
        .collect::<BTreeSet<_>>();
    let mut participant_ids = BTreeSet::new();
    let mut occupied = BTreeMap::new();
    for (index, participant) in scenario.participants.iter().enumerate() {
        let path = format!("$.participants[{index}]");
        if participant.id.trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_PARTICIPANT_ID_EMPTY",
                format!("{path}.id"),
                "participant identity must not be empty",
            ));
        } else if !participant_ids.insert(participant.id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_PARTICIPANT_DUPLICATE",
                format!("{path}.id"),
                format!("duplicate participant {}", participant.id),
            ));
        }
        if participant.label.trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_PARTICIPANT_LABEL_EMPTY",
                format!("{path}.label"),
                "participant label must not be empty",
            ));
        }
        if participant.team_id.as_str().trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_TEAM_ID_EMPTY",
                format!("{path}.teamId"),
                "team identity must not be empty",
            ));
        }
        if !position_in_board(&scenario.board, participant.position) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_POSITION_OUT_OF_BOUNDS",
                format!("{path}.position"),
                "participant position is outside the board extent",
            ));
        } else if let Some(previous) =
            occupied.insert(participant.position, participant.id.as_str())
        {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_POSITION_OCCUPIED",
                format!("{path}.position"),
                format!("participant position is already occupied by {previous}"),
            ));
        }
        if cell_blocks_position(&scenario.board, participant.position) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_POSITION_BLOCKED",
                format!("{path}.position"),
                "participant position is on an impassable cell",
            ));
        }

        let mut references = BTreeSet::new();
        let mut has_action = false;
        for (definition_index, definition_id) in participant.definition_ids.iter().enumerate() {
            let definition_path = format!("{path}.definitionIds[{definition_index}]");
            if !references.insert(definition_id.as_str()) {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_DEFINITION_DUPLICATE",
                    definition_path,
                    format!("duplicate definition reference {definition_id}"),
                ));
                continue;
            }
            let Some((kind, visibility)) = definition_kinds.get(definition_id.as_str()) else {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_DEFINITION_UNKNOWN",
                    definition_path,
                    format!("definition {definition_id} is not in the bound artifact"),
                ));
                continue;
            };
            if *visibility != MaterializedContentVisibility::Exported {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_DEFINITION_NOT_EXPORTED",
                    definition_path,
                    format!("definition {definition_id} is not exported by the bound artifact"),
                ));
                continue;
            }
            if *kind == MaterializedContentDefinitionKind::Action
                && action_ids.contains(definition_id.as_str())
            {
                has_action = true;
            }
        }
        if !has_action {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_PARTICIPANT_ACTION_REQUIRED",
                format!("{path}.definitionIds"),
                "each authority-controlled participant must reference an artifact action",
            ));
        }
        validate_participant_capabilities(
            participant,
            &path,
            &required_capabilities,
            &ruleset_values,
            &content_values,
            &mut diagnostics,
        );
    }
    if scenario.participants.is_empty() {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_PARTICIPANTS_REQUIRED",
            "$.participants",
            "encounter scenario requires at least one participant",
        ));
    }
    validate_turn(scenario, &participant_ids, &mut diagnostics);
    diagnostics
}

fn validate_binding(
    binding: &RpgRandomSourceBinding,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    for (path, value) in [
        ("$.randomSource.policyId", binding.policy_id.as_str()),
        ("$.randomSource.sourceId", binding.source_id.as_str()),
    ] {
        if value.trim().is_empty() {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_RANDOM_ID_EMPTY",
                path,
                "random source identity must not be empty",
            ));
        }
    }
    if binding.policy_version == 0 || binding.source_version == 0 {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_RANDOM_VERSION_INVALID",
            "$.randomSource",
            "random policy and source versions must be positive",
        ));
    }
}

fn validate_board(
    bundle: &CompiledPlayBundle,
    board: &RpgBoardSetup,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    if board.width == 0
        || board.height == 0
        || board.width > MAXIMUM_BOARD_EXTENT
        || board.height > MAXIMUM_BOARD_EXTENT
    {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_BOARD_EXTENT_INVALID",
            "$.board",
            format!("board width and height must be within 1..={MAXIMUM_BOARD_EXTENT}"),
        ));
    }
    let definitions = bundle
        .artifact()
        .materialized_definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition.kind))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for (index, cell) in board.cells.iter().enumerate() {
        let path = format!("$.board.cells[{index}]");
        if cell.id.trim().is_empty() || !ids.insert(cell.id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CELL_ID_INVALID",
                format!("{path}.id"),
                "cell identity must be non-empty and unique",
            ));
        }
        if !positions.insert(cell.position) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CELL_POSITION_DUPLICATE",
                format!("{path}.position"),
                "only one cell record may describe a board position",
            ));
        }
        if !position_in_board(board, cell.position) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CELL_OUT_OF_BOUNDS",
                format!("{path}.position"),
                "cell position is outside the board extent",
            ));
        }
        let mut capability_ids = BTreeSet::new();
        let mut traversal_seen = false;
        for (capability_index, capability) in cell.capabilities.iter().enumerate() {
            let capability_path = format!("{path}.capabilities[{capability_index}]");
            if capability.id.trim().is_empty() || !capability_ids.insert(capability.id.as_str()) {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_CELL_CAPABILITY_ID_INVALID",
                    format!("{capability_path}.id"),
                    "cell capability identity must be non-empty and unique per cell",
                ));
            }
            if capability.version == 0 {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_CELL_CAPABILITY_VERSION_INVALID",
                    format!("{capability_path}.version"),
                    "cell capability version must be positive",
                ));
            }
            if let Some(definition_id) = &capability.definition_id {
                match definitions.get(definition_id.as_str()) {
                    None => diagnostics.push(scenario_diagnostic(
                        "RPG_SCENARIO_DEFINITION_UNKNOWN",
                        format!("{capability_path}.definitionId"),
                        format!("definition {definition_id} is not in the bound artifact"),
                    )),
                    Some(MaterializedContentDefinitionKind::Action) => {
                        diagnostics.push(scenario_diagnostic(
                            "RPG_SCENARIO_CELL_DEFINITION_INCOMPATIBLE",
                            format!("{capability_path}.definitionId"),
                            "cell capabilities must reference an artifact support definition",
                        ))
                    }
                    Some(MaterializedContentDefinitionKind::Support) => {}
                }
            }
            match &capability.value {
                RpgCellCapabilityValue::Traversal { movement_cost, .. } => {
                    if traversal_seen || *movement_cost == 0 {
                        diagnostics.push(scenario_diagnostic(
                            "RPG_SCENARIO_CELL_TRAVERSAL_INVALID",
                            format!("{capability_path}.value"),
                            "a cell permits one traversal capability with positive movement cost",
                        ));
                    }
                    traversal_seen = true;
                }
                RpgCellCapabilityValue::Identifier { value_id } if value_id.trim().is_empty() => {
                    diagnostics.push(scenario_diagnostic(
                        "RPG_SCENARIO_CELL_VALUE_ID_EMPTY",
                        format!("{capability_path}.value.valueId"),
                        "cell capability value identity must not be empty",
                    ));
                }
                _ => {}
            }
        }
    }
}

fn validate_participant_capabilities(
    participant: &RpgParticipantSetup,
    path: &str,
    required: &BTreeSet<&str>,
    ruleset_values: &BTreeMap<(RulesetValueKind, &str), (i64, i64)>,
    content_values: &BTreeSet<(&str, &str)>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    let mut vitality = 0;
    let mut identities = BTreeSet::new();
    for (index, capability) in participant.capabilities.iter().enumerate() {
        let capability_path = format!("{path}.capabilities[{index}]");
        let owner = capability.owner_id();
        if !required.contains(owner) && owner != "capability.vitality" {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CAPABILITY_OWNER_INCOMPATIBLE",
                &capability_path,
                format!("artifact does not declare initial capability owner {owner}"),
            ));
        }
        let identity = match capability {
            RpgInitialCapability::Vitality { value } => {
                vitality += 1;
                validate_bounded(value, &capability_path, diagnostics);
                (owner, "vitality")
            }
            RpgInitialCapability::Stat { id, value } => {
                validate_initial_ruleset_value(
                    RulesetValueKind::Stat,
                    id,
                    *value,
                    &capability_path,
                    ruleset_values,
                    diagnostics,
                );
                (owner, id.as_str())
            }
            RpgInitialCapability::Defense { id, value } => {
                validate_initial_ruleset_value(
                    RulesetValueKind::Defense,
                    id,
                    *value,
                    &capability_path,
                    ruleset_values,
                    diagnostics,
                );
                (owner, id.as_str())
            }
            RpgInitialCapability::Resource { id, value } => {
                validate_bounded(value, &capability_path, diagnostics);
                validate_initial_content_value(
                    "resource",
                    id,
                    &capability_path,
                    content_values,
                    diagnostics,
                );
                (owner, id.as_str())
            }
            RpgInitialCapability::Modifier {
                stacking_group,
                id,
                remaining_turns,
                ..
            } => {
                validate_initial_content_value(
                    "modifier",
                    id,
                    &capability_path,
                    content_values,
                    diagnostics,
                );
                if id.trim().is_empty()
                    || !(1..=MAXIMUM_RPG_MODIFIER_TURNS).contains(remaining_turns)
                {
                    diagnostics.push(scenario_diagnostic(
                        "RPG_SCENARIO_MODIFIER_INVALID",
                        &capability_path,
                        format!(
                            "modifier identity and remaining turns within 1..={MAXIMUM_RPG_MODIFIER_TURNS} are required"
                        ),
                    ));
                }
                (owner, stacking_group.as_str())
            }
        };
        if identity.1.trim().is_empty() || !identities.insert(identity) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CAPABILITY_DUPLICATE",
                capability_path,
                format!(
                    "capability identity {} must be non-empty and unique within owner {}",
                    identity.1, identity.0
                ),
            ));
        }
    }
    if vitality != 1 {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_VITALITY_REQUIRED",
            format!("{path}.capabilities"),
            "each participant requires exactly one vitality capability",
        ));
    }
}

fn validate_initial_ruleset_value(
    kind: RulesetValueKind,
    id: &str,
    value: i32,
    path: &str,
    values: &BTreeMap<(RulesetValueKind, &str), (i64, i64)>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    let Some((minimum, maximum)) = values.get(&(kind, id)) else {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_RULESET_VALUE_UNKNOWN",
            format!("{path}.id"),
            format!(
                "initial {:?} {id} is not provided by the bound ruleset",
                kind
            ),
        ));
        return;
    };
    let value = i64::from(value);
    if value < *minimum || value > *maximum {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_RULESET_VALUE_OUT_OF_DOMAIN",
            format!("{path}.value"),
            format!("initial value must be within {minimum}..={maximum}"),
        ));
    }
}

fn validate_initial_content_value(
    catalog: &str,
    id: &str,
    path: &str,
    values: &BTreeSet<(&str, &str)>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    if values.contains(&(catalog, id)) {
        return;
    }
    diagnostics.push(scenario_diagnostic(
        "RPG_SCENARIO_CONTENT_VALUE_UNKNOWN",
        format!("{path}.id"),
        format!("initial {catalog} {id} is not defined by the bound content packs"),
    ));
}

fn validate_bounded(
    value: &BoundedValue,
    path: &str,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    if value.max < 0 || value.current < 0 || value.current > value.max {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_CAPABILITY_VALUE_OUT_OF_BOUNDS",
            path,
            "bounded capability values require 0 <= current <= max",
        ));
    }
}

fn validate_turn(
    scenario: &RpgScenario,
    participant_ids: &BTreeSet<&str>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    let mut order = BTreeSet::new();
    for (index, participant_id) in scenario.turn.initiative_order.iter().enumerate() {
        if !participant_ids.contains(participant_id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_TURN_PARTICIPANT_UNKNOWN",
                format!("$.turn.initiativeOrder[{index}]"),
                format!("unknown initiative participant {participant_id}"),
            ));
        }
        if !order.insert(participant_id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_TURN_PARTICIPANT_DUPLICATE",
                format!("$.turn.initiativeOrder[{index}]"),
                format!("duplicate initiative participant {participant_id}"),
            ));
        }
    }
    if order != *participant_ids {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_TURN_ORDER_INCOMPLETE",
            "$.turn.initiativeOrder",
            "initiative order must contain every participant exactly once",
        ));
    }
    if !order.contains(scenario.turn.current_actor_id.as_str()) {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_CURRENT_ACTOR_UNKNOWN",
            "$.turn.currentActorId",
            "current actor must appear in initiative order",
        ));
    } else if !scenario
        .participants
        .iter()
        .find(|participant| participant.id == scenario.turn.current_actor_id)
        .and_then(|participant| {
            participant
                .capabilities
                .iter()
                .find_map(|capability| match capability {
                    RpgInitialCapability::Vitality { value } => Some(value.current > 0),
                    _ => None,
                })
        })
        .unwrap_or(false)
    {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_CURRENT_ACTOR_INACTIVE",
            "$.turn.currentActorId",
            "current actor must have positive vitality",
        ));
    }
    if scenario.turn.round == 0 || scenario.turn.turn == 0 {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_TURN_COUNTER_INVALID",
            "$.turn",
            "round and turn counters must be positive",
        ));
    }
}

pub(crate) fn position_in_board(board: &RpgBoardSetup, position: GridPosition) -> bool {
    position.x < board.width && position.y < board.height
}

pub(crate) fn cell_blocks_position(board: &RpgBoardSetup, position: GridPosition) -> bool {
    board
        .cells
        .iter()
        .find(|cell| cell.position == position)
        .map(|cell| {
            cell.capabilities.iter().any(|capability| {
                matches!(
                    capability.value,
                    RpgCellCapabilityValue::Traversal {
                        passable: false,
                        ..
                    }
                )
            })
        })
        .unwrap_or(false)
}

pub(crate) fn runtime_board_rejection(
    board: &RpgBoardSetup,
    state: &RpgCapabilityState,
) -> Option<RpgResolutionRejection> {
    let mut occupied = BTreeMap::new();
    for entity in state.entities() {
        if !position_in_board(board, entity.position()) {
            return Some(resolution_rejection(
                "RPG_BOARD_POSITION_OUT_OF_BOUNDS",
                "$.resolution.state.position",
                format!("participant {} moved outside the board extent", entity.id()),
            ));
        }
        if cell_blocks_position(board, entity.position()) {
            return Some(resolution_rejection(
                "RPG_BOARD_POSITION_BLOCKED",
                "$.resolution.state.position",
                format!("participant {} moved onto an impassable cell", entity.id()),
            ));
        }
        if let Some(previous) = occupied.insert(entity.position(), entity.id()) {
            return Some(resolution_rejection(
                "RPG_BOARD_POSITION_OCCUPIED",
                "$.resolution.state.position",
                format!(
                    "participants {previous} and {} occupy the same cell",
                    entity.id()
                ),
            ));
        }
    }
    None
}

pub(crate) fn validate_restored_encounter(
    authority: &RpgEncounterAuthority,
    state: &RpgCapabilityState,
) -> Vec<RpgScenarioDiagnostic> {
    let mut diagnostics = Vec::new();
    let setup_ids = authority
        .scenario
        .participants
        .iter()
        .map(|participant| participant.id.as_str())
        .collect::<BTreeSet<_>>();
    let state_ids = state
        .entities()
        .map(RpgEntityState::id)
        .collect::<BTreeSet<_>>();
    if setup_ids != state_ids {
        diagnostics.push(scenario_diagnostic(
            "RPG_CHECKPOINT_PARTICIPANT_SET_MISMATCH",
            "$.state.entities",
            "checkpoint state must contain exactly the scenario participants",
        ));
    }
    if authority.turn.initiative_order != authority.scenario.turn.initiative_order {
        diagnostics.push(scenario_diagnostic(
            "RPG_CHECKPOINT_TURN_ORDER_MISMATCH",
            "$.turn.initiativeOrder",
            "checkpoint initiative order must match the scenario binding",
        ));
    }
    let current_actor_active = state
        .entity(&authority.turn.current_actor_id)
        .map(|entity| entity.vitality().current > 0)
        .unwrap_or(false);
    if !setup_ids.contains(authority.turn.current_actor_id.as_str())
        || authority.turn.round == 0
        || authority.turn.turn == 0
        || (matches!(
            encounter_outcome(state),
            RpgEncounterOutcomeView::InProgress
        ) && !current_actor_active)
    {
        diagnostics.push(scenario_diagnostic(
            "RPG_CHECKPOINT_TURN_STATE_INVALID",
            "$.turn",
            "checkpoint turn state must identify an active scenario participant with positive counters",
        ));
    }
    if let Some(rejection) = runtime_board_rejection(&authority.scenario.board, state) {
        diagnostics.push(scenario_diagnostic(
            "RPG_CHECKPOINT_BOARD_STATE_INVALID",
            rejection.path,
            format!("{}: {}", rejection.code, rejection.message),
        ));
    }
    for (index, entry) in authority.log.iter().enumerate() {
        let expected_sequence = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        let action_owned = authority
            .participant_definitions
            .get(&entry.actor_id)
            .map(|definitions| {
                entry.action_id == RPG_END_TURN_CONTROL_ID || definitions.contains(&entry.action_id)
            })
            .unwrap_or(false);
        if entry.sequence != expected_sequence
            || entry.state_revision == 0
            || entry.state_revision > state.revision()
            || !action_owned
        {
            diagnostics.push(scenario_diagnostic(
                "RPG_CHECKPOINT_LOG_INVALID",
                format!("$.log[{index}]"),
                "checkpoint log sequence, revision, actor, or action is invalid",
            ));
        }
    }
    diagnostics
}

pub(crate) fn living_intent_rejection(
    state: &RpgCapabilityState,
    intent: &RpgIntent,
) -> Option<RpgResolutionRejection> {
    if !participant_is_living(state, &intent.actor_id) {
        return Some(resolution_rejection(
            "RPG_TURN_ACTOR_INACTIVE",
            "$.command.intent.actorId",
            format!(
                "participant {} cannot act without positive vitality",
                intent.actor_id
            ),
        ));
    }
    intent
        .target_ids
        .iter()
        .enumerate()
        .find_map(|(index, target_id)| {
            state.entity(target_id).and_then(|_| {
                (!participant_is_living(state, target_id)).then(|| {
                    resolution_rejection(
                        "RPG_INTENT_TARGET_INACTIVE",
                        format!("$.command.intent.targetIds[{index}]"),
                        format!(
                            "participant {target_id} cannot be targeted without positive vitality"
                        ),
                    )
                })
            })
        })
}

fn participant_is_living(state: &RpgCapabilityState, participant_id: &str) -> bool {
    state
        .entity(participant_id)
        .map(|participant| participant.vitality().current > 0)
        .unwrap_or(false)
}

pub(crate) fn action_view(
    action: CompiledRpgAction,
    candidate_ids: Vec<String>,
    unavailable: Option<RpgResolutionRejection>,
) -> RpgActionView {
    RpgActionView {
        definition_id: action.id,
        label: action.name,
        available: unavailable.is_none() && !candidate_ids.is_empty(),
        unavailable,
        maximum_targets: action.targets.maximum_targets,
        options: RpgActionOptionsView {
            participant_ids: candidate_ids,
            cell_ids: Vec::new(),
            area_ids: Vec::new(),
        },
    }
}

pub(crate) fn participant_view(
    entity: &RpgEntityState,
    label: String,
    definition_ids: Vec<String>,
) -> RpgParticipantView {
    RpgParticipantView {
        id: entity.id().to_owned(),
        label,
        team_id: entity.team().clone(),
        position: entity.position(),
        definition_ids,
        vitality: entity.vitality(),
        stats: entity
            .stats()
            .map(|(id, value)| RpgNamedIntegerView {
                id: id.to_owned(),
                value,
            })
            .collect(),
        defenses: entity
            .defenses()
            .map(|(id, value)| RpgNamedIntegerView {
                id: id.to_owned(),
                value,
            })
            .collect(),
        resources: entity
            .resources()
            .map(|(id, value)| RpgNamedBoundedView {
                id: id.to_owned(),
                value,
            })
            .collect(),
        modifiers: entity
            .modifiers()
            .map(|(stacking_group, modifier)| RpgModifierView {
                stacking_group: stacking_group.to_owned(),
                id: modifier.id().to_owned(),
                value: modifier.value(),
                remaining_turns: modifier.remaining_turns(),
            })
            .collect(),
    }
}

pub(crate) fn encounter_outcome(state: &RpgCapabilityState) -> RpgEncounterOutcomeView {
    let active_teams = state
        .entities()
        .filter(|entity| entity.vitality().current > 0)
        .map(|entity| entity.team().clone())
        .collect::<BTreeSet<_>>();
    if active_teams.len() > 1 {
        RpgEncounterOutcomeView::InProgress
    } else {
        RpgEncounterOutcomeView::Completed {
            winning_team_ids: active_teams.into_iter().collect(),
        }
    }
}

pub(crate) fn random_failure(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> RpgRandomSourceFailure {
    RpgRandomSourceFailure {
        code: code.to_owned(),
        path: path.into(),
        message: message.into(),
        expected_request: None,
        actual_request: None,
    }
}

fn scenario_diagnostic(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> RpgScenarioDiagnostic {
    RpgScenarioDiagnostic {
        code: code.to_owned(),
        path: path.into(),
        message: message.into(),
    }
}

fn resolution_rejection(
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
    use serde_json::json;

    use super::*;

    #[test]
    fn scenario_decode_rejects_every_definition_script_and_tester_field() {
        let base = json!({
            "schema": {"id": RPG_SCENARIO_SCHEMA_ID, "version": 1},
            "playBundleId": "artifact.test",
            "board": {"width": 2, "height": 2, "cells": []},
            "participants": [],
            "turn": {
                "initiativeOrder": [],
                "currentActorId": "participant.test",
                "round": 1,
                "turn": 1
            },
            "randomSource": {
                "policyId": "random.recorded",
                "policyVersion": 1,
                "sourceId": "source.test",
                "sourceVersion": 1
            }
        });
        for (field, value) in [
            ("definitions", json!([])),
            ("commands", json!([])),
            ("targets", json!([])),
            ("reactions", json!([])),
            ("rolls", json!([])),
            ("expectedEvents", json!([])),
            ("expectedOutcomes", json!([])),
            ("tester", json!({})),
        ] {
            let mut source = base.clone();
            source
                .as_object_mut()
                .expect("scenario fixture is an object")
                .insert(field.to_owned(), value);
            let failure = serde_json::from_value::<RpgScenario>(source).unwrap_err();
            assert!(
                failure.to_string().contains("unknown field"),
                "field {field} must fail strict decode: {failure}"
            );
        }
    }
}
