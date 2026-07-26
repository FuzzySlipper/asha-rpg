use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use rpg_compiler::{CompiledPlayBundle, CompiledRpgAction, RulesetValueKey};
use rpg_core::{
    ActiveRpgModifier, BoundedValue, GridPosition, RpgCapabilityState, RpgEntityState, RpgIntent,
    RpgIntentItemBinding, RpgRandomRequest, RpgReactionRequest, RpgResolutionRejection, RpgTeamId,
    StateFingerprint, MAXIMUM_RPG_MODIFIER_TURNS,
};
use rpg_ir::{
    MaterializedContentDefinitionKind, MaterializedContentVisibility, RpgIrActivation,
    RpgIrAreaOrigin, RpgIrAreaShape, RpgIrLineOfEffectRequirement, RpgIrTargetSelector,
    RulesetActivationBudgetResetBoundary, RulesetActivationTiming, RulesetValueKind,
};
use serde::{Deserialize, Serialize};

pub const RPG_SCENARIO_SCHEMA_ID: &str = "asha.rpg.scenario";
pub const RPG_SCENARIO_SCHEMA_VERSION: u32 = 3;
pub const RPG_ENCOUNTER_VIEW_SCHEMA_ID: &str = "asha.rpg.encounter.view";
pub const RPG_ENCOUNTER_VIEW_SCHEMA_VERSION: u32 = 12;
pub const RPG_END_TURN_CONTROL_ID: &str = "control.end-turn";
pub const RPG_LINE_OF_EFFECT_OBSTRUCTION_ID: &str = "line-of-effect.obstruction";
pub const RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION: u32 = 1;

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
    LineOfEffectObstruction { blocks: bool },
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
    #[serde(default)]
    pub class_definition_id: Option<String>,
    #[serde(default)]
    pub feature_definition_ids: Vec<String>,
    #[serde(default)]
    pub items: Vec<RpgItemInstanceSetup>,
    #[serde(default)]
    pub equipment: Vec<RpgEquipmentSlotSetup>,
    pub capabilities: Vec<RpgInitialCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgItemInstanceSetup {
    pub id: String,
    pub definition_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgEquipmentSlotSetup {
    pub slot_id: String,
    pub item_instance_id: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
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
pub struct RpgEffectView {
    pub instance_id: String,
    pub definition_id: String,
    pub definition_version: u32,
    pub label: String,
    pub source_entity_id: String,
    pub stacking_id: String,
    pub stacking: rpg_core::RpgEffectStackingPolicy,
    pub rank: i32,
    pub duration_anchor: rpg_core::RpgEffectDurationAnchor,
    pub remaining_count: u32,
    pub application_revision: u64,
    pub contributions: Vec<rpg_core::RpgScalarContributionDefinition>,
    pub outcome_band_shifts: Vec<rpg_core::RpgOutcomeBandShiftDefinition>,
    pub pool_contributions: Vec<rpg_core::RpgPoolContributionDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgParticipantView {
    pub id: String,
    pub label: String,
    pub team_id: RpgTeamId,
    pub position: GridPosition,
    pub definition_ids: Vec<String>,
    pub class_definition_id: Option<String>,
    pub feature_definition_ids: Vec<String>,
    pub items: Vec<RpgItemInstanceView>,
    pub equipment: Vec<RpgEquipmentSlotSetup>,
    pub vitality: BoundedValue,
    pub stats: Vec<RpgNamedIntegerView>,
    pub defenses: Vec<RpgNamedIntegerView>,
    pub resources: Vec<RpgNamedBoundedView>,
    pub modifiers: Vec<RpgModifierView>,
    pub effects: Vec<RpgEffectView>,
    pub activation_budgets: Vec<RpgActivationBudgetView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActivationBudgetView {
    pub id: String,
    pub label: String,
    pub timing: RulesetActivationTiming,
    pub reset_boundary: RulesetActivationBudgetResetBoundary,
    pub initial_amount: i32,
    pub remaining: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgItemInstanceView {
    pub id: String,
    pub definition_id: String,
    pub label: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub traits: Vec<String>,
    pub allowed_slots: Vec<String>,
    pub attributes: Vec<rpg_ir::ItemAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionOptionsView {
    pub binding: RpgActionOptionBindingView,
    pub participant_ids: Vec<String>,
    pub cell_paths: Vec<RpgCellPathView>,
    pub filtered_participants: Vec<RpgFilteredParticipantOptionView>,
    pub filtered_cells: Vec<RpgFilteredCellOptionView>,
    pub area_options: Vec<RpgAreaOptionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionOptionBindingView {
    pub session_binding_id: String,
    pub artifact_id: String,
    pub scenario_fingerprint: StateFingerprint,
    pub authority_revision: u64,
    pub round: u64,
    pub turn: u64,
    pub current_actor_id: String,
    pub action_id: String,
    pub item_binding: Option<RpgIntentItemBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgFilteredParticipantOptionView {
    pub participant_id: String,
    pub reason: String,
    pub blocking_cell_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgFilteredCellOptionView {
    pub cell_id: String,
    pub reason: String,
    pub blocking_cell_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgCellPathView {
    pub destination_cell_id: String,
    pub cell_ids: Vec<String>,
    pub movement_cost: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaFilteredParticipantView {
    pub participant_id: String,
    pub reason: String,
    #[serde(default)]
    pub blocking_cell_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaFilteredCellView {
    pub x: i64,
    pub y: i64,
    pub reason: String,
    #[serde(default)]
    pub blocking_cell_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaOptionView {
    pub session_binding_id: String,
    pub artifact_id: String,
    pub scenario_fingerprint: StateFingerprint,
    pub authority_revision: u64,
    pub round: u64,
    pub turn: u64,
    pub current_actor_id: String,
    pub action_id: String,
    pub item_binding: Option<RpgIntentItemBinding>,
    pub origin: RpgIrAreaOrigin,
    pub shape: RpgIrAreaShape,
    pub origin_cell_id: String,
    pub anchor_cell_id: String,
    pub included_cell_ids: Vec<String>,
    pub filtered_cells: Vec<RpgAreaFilteredCellView>,
    pub included_participant_ids: Vec<String>,
    pub filtered_participants: Vec<RpgAreaFilteredParticipantView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RpgAreaProjection {
    pub origin: RpgIrAreaOrigin,
    pub shape: RpgIrAreaShape,
    pub origin_cell_id: String,
    pub anchor_cell_id: String,
    pub included_cells: Vec<RpgCellSetup>,
    pub filtered_cells: Vec<RpgAreaFilteredCellView>,
    pub included_participant_ids: Vec<String>,
    pub filtered_participants: Vec<RpgAreaFilteredParticipantView>,
}

pub(crate) struct RpgAreaBoardIndex<'a> {
    board: &'a RpgBoardSetup,
    cells_by_id: BTreeMap<&'a str, &'a RpgCellSetup>,
    cells_by_position: BTreeMap<GridPosition, &'a RpgCellSetup>,
    participants_by_position: BTreeMap<GridPosition, Vec<&'a RpgEntityState>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RpgLineOfEffectProjection {
    pub clear: bool,
    pub reason: Option<&'static str>,
    pub blocking_cell_ids: Vec<String>,
}

impl<'a> RpgAreaBoardIndex<'a> {
    pub(crate) fn new(board: &'a RpgBoardSetup, state: &'a RpgCapabilityState) -> Self {
        let mut participants_by_position = BTreeMap::<_, Vec<_>>::new();
        for participant in state.entities() {
            participants_by_position
                .entry(participant.position())
                .or_default()
                .push(participant);
        }
        for participants in participants_by_position.values_mut() {
            participants.sort_by_key(|participant| participant.id());
        }
        Self {
            board,
            cells_by_id: board
                .cells
                .iter()
                .map(|cell| (cell.id.as_str(), cell))
                .collect(),
            cells_by_position: board
                .cells
                .iter()
                .map(|cell| (cell.position, cell))
                .collect(),
            participants_by_position,
        }
    }
}

fn line_of_effect_supercover(start: GridPosition, end: GridPosition) -> Vec<GridPosition> {
    let mut current_x = i64::from(start.x);
    let mut current_y = i64::from(start.y);
    let end_x = i64::from(end.x);
    let end_y = i64::from(end.y);
    let delta_x = end_x.abs_diff(current_x);
    let delta_y = end_y.abs_diff(current_y);
    let step_x = (end_x - current_x).signum();
    let step_y = (end_y - current_y).signum();
    let mut traversed_x = 0_u64;
    let mut traversed_y = 0_u64;
    let mut positions = Vec::new();
    let mut seen = BTreeSet::new();
    while traversed_x < delta_x || traversed_y < delta_y {
        let x_crossing = traversed_x
            .saturating_mul(2)
            .saturating_add(1)
            .saturating_mul(delta_y);
        let y_crossing = traversed_y
            .saturating_mul(2)
            .saturating_add(1)
            .saturating_mul(delta_x);
        if x_crossing == y_crossing {
            let mut corner_cells = [
                (current_x.saturating_add(step_x), current_y),
                (current_x, current_y.saturating_add(step_y)),
            ];
            corner_cells.sort_by_key(|(x, y)| (*y, *x));
            for (x, y) in corner_cells {
                if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
                    let position = GridPosition { x, y };
                    if seen.insert(position) {
                        positions.push(position);
                    }
                }
            }
            current_x = current_x.saturating_add(step_x);
            current_y = current_y.saturating_add(step_y);
            traversed_x = traversed_x.saturating_add(1);
            traversed_y = traversed_y.saturating_add(1);
        } else if x_crossing < y_crossing {
            current_x = current_x.saturating_add(step_x);
            traversed_x = traversed_x.saturating_add(1);
        } else {
            current_y = current_y.saturating_add(step_y);
            traversed_y = traversed_y.saturating_add(1);
        }
        if let (Ok(x), Ok(y)) = (u32::try_from(current_x), u32::try_from(current_y)) {
            let position = GridPosition { x, y };
            if seen.insert(position) {
                positions.push(position);
            }
        }
    }
    positions
}

fn cell_blocks_line_of_effect(cell: &RpgCellSetup) -> bool {
    cell.capabilities.iter().any(|capability| {
        capability.id == RPG_LINE_OF_EFFECT_OBSTRUCTION_ID
            && capability.version == RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION
            && matches!(
                capability.value,
                RpgCellCapabilityValue::LineOfEffectObstruction { blocks: true }
            )
    })
}

pub(crate) fn line_of_effect_projection(
    board_index: &RpgAreaBoardIndex<'_>,
    start: GridPosition,
    end: GridPosition,
) -> RpgLineOfEffectProjection {
    if !board_index.cells_by_position.contains_key(&start)
        || !board_index.cells_by_position.contains_key(&end)
    {
        return RpgLineOfEffectProjection {
            clear: false,
            reason: Some("lineOfEffectCellMissing"),
            blocking_cell_ids: Vec::new(),
        };
    }
    let mut missing_cell = false;
    let mut blocking_cell_ids = Vec::new();
    for position in line_of_effect_supercover(start, end) {
        if position == start || position == end {
            continue;
        }
        let Some(cell) = board_index.cells_by_position.get(&position) else {
            missing_cell = true;
            continue;
        };
        if cell_blocks_line_of_effect(cell) {
            blocking_cell_ids.push(cell.id.clone());
        }
    }
    let reason = if missing_cell {
        Some("lineOfEffectCellMissing")
    } else if blocking_cell_ids.is_empty() {
        None
    } else {
        Some("lineOfEffectBlocked")
    };
    RpgLineOfEffectProjection {
        clear: reason.is_none(),
        reason,
        blocking_cell_ids,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgActionView {
    pub definition_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
    pub available: bool,
    pub unavailable: Option<RpgResolutionRejection>,
    pub maximum_targets: u32,
    pub activation: Option<RpgIrActivation>,
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
    pub session_binding_id: String,
    pub artifact_id: String,
    pub scenario_fingerprint: StateFingerprint,
    pub state_revision: u64,
    pub accepted_random_position: u64,
    pub random_source: RpgRandomSourceBinding,
    pub board: RpgBoardSetup,
    pub participants: Vec<RpgParticipantView>,
    pub turn: RpgTurnState,
    pub accepted_activations_this_turn: u32,
    pub accepted_activation_ceiling: Option<u32>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgBoundActionProposal {
    pub binding: RpgActionOptionBindingView,
    pub target_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaActionProposal {
    pub session_binding_id: String,
    pub authority_revision: u64,
    pub action_id: String,
    pub actor_id: String,
    pub anchor_cell_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaSubmissionResult {
    pub outcome: crate::RpgCommandOutcome,
    pub replay_entry: Option<crate::RpgReplayEntry>,
    pub encounter: RpgEncounterView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgBoundActionSubmissionResult {
    pub outcome: crate::RpgCommandOutcome,
    pub replay_entry: Option<crate::RpgReplayEntry>,
    pub encounter: RpgEncounterView,
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
                    "authority requested {} bounded value(s), but the roll tape is exhausted",
                    request.count
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
        if let Some((index, value, sides)) = first_random_value_out_of_range(request, &entry.values)
        {
            return Err(random_failure(
                "RPG_RANDOM_TAPE_VALUE_OUT_OF_RANGE",
                &request.path,
                format!("roll-tape value {value} at offset {index} is outside 1..={sides}"),
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

fn first_random_value_out_of_range(
    request: &RpgRandomRequest,
    values: &[u32],
) -> Option<(usize, u32, u32)> {
    if request.heterogeneous_terms.is_empty() {
        return values
            .iter()
            .copied()
            .enumerate()
            .find(|(_, value)| *value == 0 || *value > request.sides)
            .map(|(index, value)| (index, value, request.sides));
    }
    let mut offset = 0_usize;
    for term in &request.heterogeneous_terms {
        for _ in 0..term.count {
            let value = values.get(offset).copied().unwrap_or(0);
            if value == 0 || value > term.sides {
                return Some((offset, value, term.sides));
            }
            offset = offset.saturating_add(1);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub(crate) struct RpgEncounterAuthority {
    pub(crate) scenario: RpgScenario,
    pub(crate) turn: RpgTurnState,
    pub(crate) participant_definitions: BTreeMap<String, Vec<String>>,
    pub(crate) participant_labels: BTreeMap<String, String>,
    pub(crate) item_definitions: BTreeMap<String, rpg_ir::CompiledItemDefinition>,
    pub(crate) effect_definitions: BTreeMap<String, rpg_ir::CompiledEffectDefinition>,
    pub(crate) log: Vec<RpgEncounterLogEntry>,
}

impl RpgEncounterAuthority {
    pub(crate) fn current_actor_id(&self) -> &str {
        &self.turn.current_actor_id
    }

    pub(crate) fn next_turn(&self, state: &RpgCapabilityState) -> RpgTurnState {
        let mut next_turn = self.turn.clone();
        if next_turn.initiative_order.is_empty() {
            return next_turn;
        }
        let current = self
            .turn
            .initiative_order
            .iter()
            .position(|id| id == &next_turn.current_actor_id)
            .unwrap_or(0);
        for offset in 1..=next_turn.initiative_order.len() {
            let next = (current + offset) % next_turn.initiative_order.len();
            let participant_id = &next_turn.initiative_order[next];
            let active = state
                .entity(participant_id)
                .map(|entity| entity.vitality().current > 0)
                .unwrap_or(false);
            if active {
                if next <= current {
                    next_turn.round = next_turn.round.saturating_add(1);
                }
                next_turn.turn = next_turn.turn.saturating_add(1);
                next_turn.current_actor_id = participant_id.clone();
                return next_turn;
            }
        }
        next_turn
    }

    pub(crate) fn set_turn(&mut self, turn: RpgTurnState) {
        self.turn = turn;
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
            item_binding: receipt.item_binding.clone(),
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
            item_binding: None,
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
        entity
            .restore_character_selection(
                participant.class_definition_id.clone(),
                participant.feature_definition_ids.clone(),
            )
            .expect("validated character selection restores");
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
        for budget in bundle.rules().activation_budgets() {
            entity
                .restore_activation_budget(budget.id.clone(), budget.initial_amount)
                .expect("validated activation budget restores");
        }
        let supplied_values = participant_ruleset_values(participant, bundle, false);
        let derived_values = bundle
            .value_plan()
            .evaluate(&supplied_values)
            .expect("validated ruleset value derivations evaluate");
        for (key, value) in derived_values {
            match key.kind {
                RulesetValueKind::Stat => entity
                    .restore_stat(key.id, value)
                    .expect("validated derived stat restores"),
                RulesetValueKind::Defense => entity
                    .restore_defense(key.id, value)
                    .expect("validated derived defense restores"),
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
        item_definitions: bundle
            .items()
            .iter()
            .cloned()
            .map(|item| (item.definition_id.clone(), item))
            .collect(),
        effect_definitions: bundle
            .effects()
            .iter()
            .cloned()
            .map(|effect| (effect.definition_id.clone(), effect))
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
    let item_definitions = bundle
        .items()
        .iter()
        .map(|item| (item.definition_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let character_classes = bundle
        .character_classes()
        .iter()
        .map(|class| (class.definition_id.as_str(), class))
        .collect::<BTreeMap<_, _>>();
    let character_features = bundle
        .character_features()
        .iter()
        .map(|feature| feature.definition_id.as_str())
        .collect::<BTreeSet<_>>();
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
        validate_participant_character_selection(
            participant,
            &path,
            &definition_kinds,
            &character_classes,
            &character_features,
            &mut diagnostics,
        );
        validate_participant_items(
            participant,
            &path,
            &definition_kinds,
            &item_definitions,
            &mut diagnostics,
        );
        validate_participant_capabilities(
            bundle,
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

fn validate_participant_character_selection(
    participant: &RpgParticipantSetup,
    path: &str,
    definition_kinds: &BTreeMap<
        &str,
        (
            MaterializedContentDefinitionKind,
            MaterializedContentVisibility,
        ),
    >,
    character_classes: &BTreeMap<&str, &rpg_ir::CompiledCharacterClass>,
    character_features: &BTreeSet<&str>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    let selected_class = participant.class_definition_id.as_deref().and_then(|class_id| {
        let valid = definition_kinds.get(class_id).is_some_and(|(kind, visibility)| {
            *kind == MaterializedContentDefinitionKind::CharacterClass
                && *visibility == MaterializedContentVisibility::Exported
        });
        if !valid {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_CLASS_DEFINITION_INVALID",
                format!("{path}.classDefinitionId"),
                format!(
                    "class definition {class_id} must be an exported characterClass in the bound artifact"
                ),
            ));
            return None;
        }
        character_classes.get(class_id).copied()
    });
    if participant.class_definition_id.is_none() && !participant.feature_definition_ids.is_empty() {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_FEATURE_CLASS_REQUIRED",
            format!("{path}.featureDefinitionIds"),
            "participant feature selection requires an explicit character class",
        ));
    }
    let mut previous = None::<&str>;
    for (index, feature_id) in participant.feature_definition_ids.iter().enumerate() {
        let feature_path = format!("{path}.featureDefinitionIds[{index}]");
        if !portable_identifier(feature_id)
            || previous.is_some_and(|previous| previous >= feature_id.as_str())
        {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_FEATURE_DEFINITIONS_NOT_CANONICAL",
                &feature_path,
                "participant feature identities must be unique, sorted portable identifiers",
            ));
        }
        previous = Some(feature_id);
        let valid = definition_kinds
            .get(feature_id.as_str())
            .is_some_and(|(kind, visibility)| {
                *kind == MaterializedContentDefinitionKind::CharacterFeature
                    && *visibility == MaterializedContentVisibility::Exported
                    && character_features.contains(feature_id.as_str())
            });
        if !valid {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_FEATURE_DEFINITION_INVALID",
                &feature_path,
                format!(
                    "feature definition {feature_id} must be an exported characterFeature in the bound artifact"
                ),
            ));
            continue;
        }
        if selected_class.is_some_and(|class| {
            class
                .feature_definition_ids
                .binary_search(feature_id)
                .is_err()
        }) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_FEATURE_NOT_IN_CLASS",
                feature_path,
                format!("selected class does not make character feature {feature_id} available"),
            ));
        }
    }
}

fn validate_participant_items(
    participant: &RpgParticipantSetup,
    path: &str,
    definition_kinds: &BTreeMap<
        &str,
        (
            MaterializedContentDefinitionKind,
            MaterializedContentVisibility,
        ),
    >,
    item_definitions: &BTreeMap<&str, &rpg_ir::CompiledItemDefinition>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    let mut item_instances = BTreeMap::new();
    for (index, item) in participant.items.iter().enumerate() {
        let item_path = format!("{path}.items[{index}]");
        if !portable_identifier(&item.id)
            || item_instances
                .insert(item.id.as_str(), item.definition_id.as_str())
                .is_some()
        {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_ITEM_INSTANCE_ID_INVALID",
                format!("{item_path}.id"),
                "item instance identities must be non-empty and unique per participant",
            ));
        }
        match definition_kinds.get(item.definition_id.as_str()) {
            Some((
                MaterializedContentDefinitionKind::Item,
                MaterializedContentVisibility::Exported,
            )) if item_definitions.contains_key(item.definition_id.as_str()) => {}
            Some((MaterializedContentDefinitionKind::Item, _)) => {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_ITEM_DEFINITION_NOT_EXPORTED",
                    format!("{item_path}.definitionId"),
                    format!(
                        "item definition {} is not exported by the bound artifact",
                        item.definition_id
                    ),
                ));
            }
            Some(_) => diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_ITEM_DEFINITION_INCOMPATIBLE",
                format!("{item_path}.definitionId"),
                format!("definition {} is not an item", item.definition_id),
            )),
            None => diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_ITEM_DEFINITION_UNKNOWN",
                format!("{item_path}.definitionId"),
                format!(
                    "item definition {} is not in the bound artifact",
                    item.definition_id
                ),
            )),
        }
    }

    let mut slots = BTreeSet::new();
    let mut equipped_instances = BTreeSet::new();
    for (index, equipment) in participant.equipment.iter().enumerate() {
        let equipment_path = format!("{path}.equipment[{index}]");
        if !portable_identifier(&equipment.slot_id) || !slots.insert(equipment.slot_id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_EQUIPMENT_SLOT_INVALID",
                format!("{equipment_path}.slotId"),
                "equipment slot identities must be non-empty and unique per participant",
            ));
        }
        if !equipped_instances.insert(equipment.item_instance_id.as_str()) {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_ITEM_EQUIPPED_MULTIPLE_TIMES",
                format!("{equipment_path}.itemInstanceId"),
                "one item instance may occupy only one equipment slot",
            ));
        }
        let Some(definition_id) = item_instances.get(equipment.item_instance_id.as_str()) else {
            diagnostics.push(scenario_diagnostic(
                "RPG_SCENARIO_EQUIPMENT_ITEM_UNKNOWN",
                format!("{equipment_path}.itemInstanceId"),
                format!(
                    "equipment references unknown item instance {}",
                    equipment.item_instance_id
                ),
            ));
            continue;
        };
        if let Some(item) = item_definitions.get(definition_id) {
            if item
                .allowed_slots
                .binary_search(&equipment.slot_id)
                .is_err()
            {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_EQUIPMENT_SLOT_NOT_ALLOWED",
                    format!("{equipment_path}.slotId"),
                    format!(
                        "item definition {} cannot occupy slot {}",
                        item.definition_id, equipment.slot_id
                    ),
                ));
            }
        }
    }
}

fn portable_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|value| value.is_ascii_lowercase())
        && characters.all(|value| {
            value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, '.' | '_' | '-')
        })
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
        let mut line_of_effect_obstruction_seen = false;
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
            if capability.id == RPG_LINE_OF_EFFECT_OBSTRUCTION_ID
                && !matches!(
                    capability.value,
                    RpgCellCapabilityValue::LineOfEffectObstruction { .. }
                )
            {
                diagnostics.push(scenario_diagnostic(
                    "RPG_SCENARIO_LINE_OF_EFFECT_OBSTRUCTION_INVALID",
                    &capability_path,
                    "the reserved line-of-effect obstruction identity requires its typed value",
                ));
            }
            if let Some(definition_id) = &capability.definition_id {
                match definitions.get(definition_id.as_str()) {
                    None => diagnostics.push(scenario_diagnostic(
                        "RPG_SCENARIO_DEFINITION_UNKNOWN",
                        format!("{capability_path}.definitionId"),
                        format!("definition {definition_id} is not in the bound artifact"),
                    )),
                    Some(
                        MaterializedContentDefinitionKind::Action
                        | MaterializedContentDefinitionKind::ActionProcedure
                        | MaterializedContentDefinitionKind::CharacterClass
                        | MaterializedContentDefinitionKind::CharacterFeature
                        | MaterializedContentDefinitionKind::Effect
                        | MaterializedContentDefinitionKind::Item,
                    ) => diagnostics.push(scenario_diagnostic(
                        "RPG_SCENARIO_CELL_DEFINITION_INCOMPATIBLE",
                        format!("{capability_path}.definitionId"),
                        "cell capabilities must reference an artifact support definition",
                    )),
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
                RpgCellCapabilityValue::LineOfEffectObstruction { .. } => {
                    let model_available = bundle
                        .artifact()
                        .ruleset
                        .models
                        .line_of_effect
                        .as_ref()
                        .is_some_and(|model| {
                            model.id == "line-of-effect.square-grid-supercover"
                                && model.version == 1
                        });
                    if line_of_effect_obstruction_seen
                        || capability.id != RPG_LINE_OF_EFFECT_OBSTRUCTION_ID
                        || capability.version != RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION
                        || capability.definition_id.is_some()
                        || !model_available
                    {
                        diagnostics.push(scenario_diagnostic(
                            "RPG_SCENARIO_LINE_OF_EFFECT_OBSTRUCTION_INVALID",
                            &capability_path,
                            "line-of-effect obstruction requires one line-of-effect.obstruction@1 fact per cell, no definition, and the square-grid-supercover Ruleset model",
                        ));
                    }
                    line_of_effect_obstruction_seen = true;
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
    bundle: &CompiledPlayBundle,
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
                validate_initial_ruleset_value_source(
                    bundle,
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
                validate_initial_ruleset_value_source(
                    bundle,
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
    let supplied_values = participant_ruleset_values(participant, bundle, false);
    if let Err(failure) = bundle.value_plan().evaluate(&supplied_values) {
        diagnostics.push(scenario_diagnostic(
            failure.code,
            format!("{path}.capabilities"),
            format!(
                "cannot derive ruleset value {}: {}",
                failure.target.id, failure.message
            ),
        ));
    }
}

fn validate_initial_ruleset_value_source(
    bundle: &CompiledPlayBundle,
    kind: RulesetValueKind,
    id: &str,
    value: i32,
    path: &str,
    values: &BTreeMap<(RulesetValueKind, &str), (i64, i64)>,
    diagnostics: &mut Vec<RpgScenarioDiagnostic>,
) {
    if bundle.value_plan().is_derived(kind, id) {
        diagnostics.push(scenario_diagnostic(
            "RPG_SCENARIO_DERIVED_RULESET_VALUE_SUPPLIED",
            path,
            format!("derived ruleset value {id} must not be supplied by a scenario"),
        ));
        return;
    }
    validate_initial_ruleset_value(kind, id, value, path, values, diagnostics);
}

fn participant_ruleset_values(
    participant: &RpgParticipantSetup,
    bundle: &CompiledPlayBundle,
    include_derived: bool,
) -> BTreeMap<RulesetValueKey, i32> {
    participant
        .capabilities
        .iter()
        .filter_map(|capability| match capability {
            RpgInitialCapability::Stat { id, value } => {
                Some((RulesetValueKind::Stat, id.as_str(), *value))
            }
            RpgInitialCapability::Defense { id, value } => {
                Some((RulesetValueKind::Defense, id.as_str(), *value))
            }
            _ => None,
        })
        .filter(|(kind, id, _)| include_derived || !bundle.value_plan().is_derived(*kind, id))
        .map(|(kind, id, value)| {
            (
                RulesetValueKey {
                    kind,
                    id: id.to_owned(),
                },
                value,
            )
        })
        .collect()
}

pub(crate) fn validate_derived_state(
    bundle: &CompiledPlayBundle,
    state: &RpgCapabilityState,
) -> Vec<RpgScenarioDiagnostic> {
    let mut diagnostics = Vec::new();
    for (entity_index, entity) in state.entities().enumerate() {
        let mut non_independent_stacks = BTreeSet::new();
        let mut independent_stacks = BTreeSet::new();
        let mut supplied = BTreeMap::new();
        for (id, value) in entity.stats() {
            if !bundle.value_plan().is_derived(RulesetValueKind::Stat, id) {
                supplied.insert(
                    RulesetValueKey {
                        kind: RulesetValueKind::Stat,
                        id: id.to_owned(),
                    },
                    value,
                );
            }
        }
        for (id, value) in entity.defenses() {
            if !bundle
                .value_plan()
                .is_derived(RulesetValueKind::Defense, id)
            {
                supplied.insert(
                    RulesetValueKey {
                        kind: RulesetValueKind::Defense,
                        id: id.to_owned(),
                    },
                    value,
                );
            }
        }
        let expected = match bundle.value_plan().evaluate(&supplied) {
            Ok(expected) => expected,
            Err(failure) => {
                diagnostics.push(scenario_diagnostic(
                    failure.code,
                    format!("$.state.entities[{entity_index}]"),
                    format!(
                        "cannot validate derived ruleset value {}: {}",
                        failure.target.id, failure.message
                    ),
                ));
                continue;
            }
        };
        for (key, expected_value) in expected {
            let actual_value = match key.kind {
                RulesetValueKind::Stat => entity.stat(&key.id),
                RulesetValueKind::Defense => entity.defense(&key.id),
            };
            if actual_value != Some(expected_value) {
                diagnostics.push(scenario_diagnostic(
                    "RPG_CHECKPOINT_DERIVED_RULESET_VALUE_MISMATCH",
                    format!("$.state.entities[{entity_index}]"),
                    format!(
                        "derived ruleset value {} must be {expected_value}, but checkpoint contains {:?}",
                        key.id, actual_value
                    ),
                ));
            }
        }
        for (effect_index, effect) in entity.effects().enumerate() {
            let effect_path = format!("$.state.entities[{entity_index}].effects[{effect_index}]");
            let definition = bundle
                .effects()
                .iter()
                .find(|definition| definition.definition_id == effect.definition_id());
            let valid = definition.is_some_and(|definition| {
                state.entity(effect.source_entity_id()).is_some()
                    && effect.definition_version() == definition.definition_version
                    && effect.rank() >= definition.rank_minimum
                    && effect.rank() <= definition.rank_maximum
                    && effect.stacking_id() == definition.stacking_id
                    && effect.stacking() == definition.stacking
                    && effect.duration_anchor() == definition.duration_anchor
                    && effect.remaining_count() <= definition.duration_count
                    && effect.application_revision() <= state.revision()
            });
            if !valid {
                diagnostics.push(scenario_diagnostic(
                    "RPG_CHECKPOINT_EFFECT_STATE_MISMATCH",
                    effect_path,
                    format!(
                        "active effect {} does not match its compiled definition, source, duration, or revision",
                        effect.instance_id()
                    ),
                ));
            }
            let stacking_valid = match effect.stacking() {
                rpg_core::RpgEffectStackingPolicy::IndependentBySource => independent_stacks
                    .insert((
                        effect.stacking_id().to_owned(),
                        effect.source_entity_id().to_owned(),
                    )),
                rpg_core::RpgEffectStackingPolicy::Replace
                | rpg_core::RpgEffectStackingPolicy::Refresh => {
                    non_independent_stacks.insert(effect.stacking_id().to_owned())
                }
            };
            if !stacking_valid {
                diagnostics.push(scenario_diagnostic(
                    "RPG_CHECKPOINT_EFFECT_STACKING_MISMATCH",
                    format!("$.state.entities[{entity_index}].effects[{effect_index}]"),
                    "active effects violate the compiled source-aware stacking invariant",
                ));
            }
        }
    }
    diagnostics
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RouteCandidate {
    movement_cost: u32,
    position_order: Vec<(u32, u32, String)>,
    position: (u32, u32),
    cell_ids: Vec<String>,
}

pub(crate) fn movement_paths(
    board: &RpgBoardSetup,
    state: &RpgCapabilityState,
    actor_id: &str,
    maximum_distance: u32,
) -> Vec<RpgCellPathView> {
    let Some(actor) = state.entity(actor_id) else {
        return Vec::new();
    };
    let origin = (actor.position().x, actor.position().y);
    let cells_by_position = board
        .cells
        .iter()
        .map(|cell| ((cell.position.x, cell.position.y), cell))
        .collect::<BTreeMap<_, _>>();
    let occupied = state
        .entities()
        .filter(|entity| entity.id() != actor_id)
        .map(|entity| (entity.position().x, entity.position().y))
        .collect::<BTreeSet<_>>();

    let origin_candidate = RouteCandidate {
        movement_cost: 0,
        position_order: Vec::new(),
        position: origin,
        cell_ids: Vec::new(),
    };
    let mut frontier = BTreeSet::from([origin_candidate.clone()]);
    let mut best = BTreeMap::from([(
        origin,
        (
            0_u32,
            Vec::<(u32, u32, String)>::new(),
            Vec::<String>::new(),
        ),
    )]);

    while let Some(candidate) = frontier.pop_first() {
        let Some((known_cost, known_order, _)) = best.get(&candidate.position) else {
            continue;
        };
        if (*known_cost, known_order) < (candidate.movement_cost, &candidate.position_order) {
            continue;
        }

        let (x, y) = candidate.position;
        let mut neighbours = Vec::with_capacity(4);
        if y > 0 {
            neighbours.push((x, y - 1));
        }
        if x > 0 {
            neighbours.push((x - 1, y));
        }
        if x + 1 < board.width {
            neighbours.push((x + 1, y));
        }
        if y + 1 < board.height {
            neighbours.push((x, y + 1));
        }
        neighbours.sort_by_key(|(neighbour_x, neighbour_y)| (*neighbour_y, *neighbour_x));

        for neighbour in neighbours {
            if occupied.contains(&neighbour) {
                continue;
            }
            let Some(cell) = cells_by_position.get(&neighbour) else {
                continue;
            };
            let (passable, movement_cost) = cell
                .capabilities
                .iter()
                .find_map(|capability| match capability.value {
                    RpgCellCapabilityValue::Traversal {
                        passable,
                        movement_cost,
                    } => Some((passable, movement_cost)),
                    _ => None,
                })
                .unwrap_or((true, 1));
            if !passable || movement_cost == 0 {
                continue;
            }
            let next_cost = candidate.movement_cost.saturating_add(movement_cost);
            if next_cost > maximum_distance {
                continue;
            }
            let mut next_order = candidate.position_order.clone();
            next_order.push((cell.position.y, cell.position.x, cell.id.clone()));
            let mut next_cell_ids = candidate.cell_ids.clone();
            next_cell_ids.push(cell.id.clone());
            let next_rank = (next_cost, next_order.clone());
            if best
                .get(&neighbour)
                .map(|(known_cost, known_order, _)| (*known_cost, known_order.clone()) <= next_rank)
                .unwrap_or(false)
            {
                continue;
            }
            best.insert(
                neighbour,
                (next_cost, next_order.clone(), next_cell_ids.clone()),
            );
            frontier.insert(RouteCandidate {
                movement_cost: next_cost,
                position_order: next_order,
                position: neighbour,
                cell_ids: next_cell_ids,
            });
        }
    }

    board
        .cells
        .iter()
        .filter_map(|cell| {
            let position = (cell.position.x, cell.position.y);
            if position == origin {
                return None;
            }
            best.get(&position)
                .map(|(movement_cost, _, cell_ids)| RpgCellPathView {
                    destination_cell_id: cell.id.clone(),
                    cell_ids: cell_ids.clone(),
                    movement_cost: *movement_cost,
                })
        })
        .collect()
}

pub(crate) fn area_projection(
    board_index: &RpgAreaBoardIndex<'_>,
    state: &RpgCapabilityState,
    actor_id: &str,
    targets: &RpgIrTargetSelector,
    anchor_cell_id: &str,
) -> Option<RpgAreaProjection> {
    let board = board_index.board;
    let area = targets.area.as_ref()?;
    let actor = state.entity(actor_id)?;
    let anchor = board_index.cells_by_id.get(anchor_cell_id)?;
    let anchor_range = actor
        .position()
        .x
        .abs_diff(anchor.position.x)
        .saturating_add(actor.position().y.abs_diff(anchor.position.y));
    if anchor_range > targets.maximum_range {
        return None;
    }
    if targets.line_of_effect == RpgIrLineOfEffectRequirement::Required {
        let actor_cell = board_index.cells_by_position.get(&actor.position())?;
        let anchor_projection =
            line_of_effect_projection(board_index, actor_cell.position, anchor.position);
        if !anchor_projection.clear {
            return None;
        }
    }

    let (origin_cell_id, candidates) = match (&area.origin, &area.shape) {
        (RpgIrAreaOrigin::Anchor, RpgIrAreaShape::Diamond { radius }) => {
            let radius = i64::from(*radius);
            let mut coordinates = Vec::new();
            for delta_y in -radius..=radius {
                let absolute_y = i64::try_from(delta_y.unsigned_abs()).ok()?;
                let remaining_x = radius.saturating_sub(absolute_y);
                for delta_x in -remaining_x..=remaining_x {
                    let distance = u32::try_from(
                        delta_x
                            .unsigned_abs()
                            .saturating_add(delta_y.unsigned_abs()),
                    )
                    .ok()?;
                    coordinates.push((
                        distance,
                        i64::from(anchor.position.x).saturating_add(delta_x),
                        i64::from(anchor.position.y).saturating_add(delta_y),
                    ));
                }
            }
            (anchor.id.clone(), coordinates)
        }
        (RpgIrAreaOrigin::Actor, RpgIrAreaShape::OrthogonalLine { length }) => {
            let origin_cell = board_index.cells_by_position.get(&actor.position())?;
            let delta_x = i64::from(anchor.position.x) - i64::from(actor.position().x);
            let delta_y = i64::from(anchor.position.y) - i64::from(actor.position().y);
            if (delta_x == 0) == (delta_y == 0) {
                return None;
            }
            let step_x = delta_x.signum();
            let step_y = delta_y.signum();
            let anchor_steps = delta_x
                .unsigned_abs()
                .saturating_add(delta_y.unsigned_abs());
            if anchor_steps == 0 || anchor_steps > u64::from(*length) {
                return None;
            }
            let coordinates = (1..=*length)
                .map(|distance| {
                    let distance_i64 = i64::from(distance);
                    (
                        distance,
                        i64::from(actor.position().x)
                            .saturating_add(step_x.saturating_mul(distance_i64)),
                        i64::from(actor.position().y)
                            .saturating_add(step_y.saturating_mul(distance_i64)),
                    )
                })
                .collect::<Vec<_>>();
            (origin_cell.id.clone(), coordinates)
        }
        _ => return None,
    };
    if candidates.is_empty() || candidates.len() > 256 {
        return None;
    }
    let origin_position = board_index
        .cells_by_id
        .get(origin_cell_id.as_str())?
        .position;
    let mut ranked_cells = Vec::new();
    let mut filtered_cells = Vec::new();
    for (distance, x, y) in candidates {
        let outside_board =
            x < 0 || y < 0 || x >= i64::from(board.width) || y >= i64::from(board.height);
        if outside_board {
            filtered_cells.push((
                (distance, y, x),
                RpgAreaFilteredCellView {
                    x,
                    y,
                    reason: "outsideBoard".to_owned(),
                    blocking_cell_ids: Vec::new(),
                },
            ));
            continue;
        }
        let position = GridPosition {
            x: u32::try_from(x).ok()?,
            y: u32::try_from(y).ok()?,
        };
        match board_index.cells_by_position.get(&position) {
            Some(cell) => {
                if targets.line_of_effect == RpgIrLineOfEffectRequirement::Required {
                    let projection =
                        line_of_effect_projection(board_index, origin_position, cell.position);
                    if !projection.clear {
                        filtered_cells.push((
                            (distance, y, x),
                            RpgAreaFilteredCellView {
                                x,
                                y,
                                reason: projection
                                    .reason
                                    .unwrap_or("lineOfEffectBlocked")
                                    .to_owned(),
                                blocking_cell_ids: projection.blocking_cell_ids,
                            },
                        ));
                        continue;
                    }
                }
                ranked_cells.push(((distance, y, x, cell.id.as_str()), (*cell).clone()));
            }
            None => filtered_cells.push((
                (distance, y, x),
                RpgAreaFilteredCellView {
                    x,
                    y,
                    reason: "cellMissing".to_owned(),
                    blocking_cell_ids: Vec::new(),
                },
            )),
        }
    }
    ranked_cells.sort_by(|left, right| left.0.cmp(&right.0));
    filtered_cells.sort_by_key(|candidate| candidate.0);
    let included_cells = ranked_cells
        .into_iter()
        .map(|(_, cell)| cell)
        .collect::<Vec<_>>();
    let filtered_cells = filtered_cells
        .into_iter()
        .map(|(_, cell)| cell)
        .collect::<Vec<_>>();
    if included_cells.iter().all(|cell| cell.id != anchor_cell_id) {
        return None;
    }
    let mut included_participant_ids = Vec::new();
    let mut filtered = Vec::new();
    for cell in &filtered_cells {
        if !matches!(
            cell.reason.as_str(),
            "lineOfEffectBlocked" | "lineOfEffectCellMissing"
        ) {
            continue;
        }
        let (Ok(x), Ok(y)) = (u32::try_from(cell.x), u32::try_from(cell.y)) else {
            continue;
        };
        let Some(participants) = board_index
            .participants_by_position
            .get(&GridPosition { x, y })
        else {
            continue;
        };
        for participant in participants {
            filtered.push(RpgAreaFilteredParticipantView {
                participant_id: participant.id().to_owned(),
                reason: cell.reason.clone(),
                blocking_cell_ids: cell.blocking_cell_ids.clone(),
            });
        }
    }
    for cell in &included_cells {
        let Some(participants) = board_index.participants_by_position.get(&cell.position) else {
            continue;
        };
        for participant in participants {
            let team_allowed = match targets.team {
                rpg_ir::RpgIrTeamConstraint::Hostile => participant.team() != actor.team(),
                rpg_ir::RpgIrTeamConstraint::Ally => participant.team() == actor.team(),
                rpg_ir::RpgIrTeamConstraint::Any => true,
            };
            let reason = if !team_allowed {
                Some("teamMismatch")
            } else if area.living_required && participant.vitality().current <= 0 {
                Some("notLiving")
            } else {
                None
            };
            if let Some(reason) = reason {
                filtered.push(RpgAreaFilteredParticipantView {
                    participant_id: participant.id().to_owned(),
                    reason: reason.to_owned(),
                    blocking_cell_ids: Vec::new(),
                });
            } else {
                included_participant_ids.push(participant.id().to_owned());
            }
        }
    }
    if included_participant_ids.len() < area.minimum_targets as usize
        || included_participant_ids.len() > targets.maximum_targets as usize
    {
        return None;
    }
    Some(RpgAreaProjection {
        origin: area.origin,
        shape: area.shape.clone(),
        origin_cell_id,
        anchor_cell_id: anchor_cell_id.to_owned(),
        included_cells,
        filtered_cells,
        included_participant_ids,
        filtered_participants: filtered,
    })
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
    rules: &rpg_compiler::CompiledRpgRules,
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
    for (index, participant) in authority.scenario.participants.iter().enumerate() {
        let Some(entity) = state.entity(&participant.id) else {
            continue;
        };
        if entity.class_definition_id() != participant.class_definition_id.as_deref()
            || entity.character_feature_ids() != participant.feature_definition_ids.as_slice()
        {
            diagnostics.push(scenario_diagnostic(
                "RPG_CHECKPOINT_CHARACTER_SELECTION_MISMATCH",
                format!("$.state.entities[{index}]"),
                "checkpoint class and character-feature selection must match the scenario binding",
            ));
        }
        let expected_budgets = rules
            .activation_budgets()
            .map(|budget| budget.id.as_str())
            .collect::<BTreeSet<_>>();
        let actual_budgets = entity
            .activation_budgets()
            .map(|(id, _)| id)
            .collect::<BTreeSet<_>>();
        let budget_values_valid = rules.activation_budgets().all(|budget| {
            entity
                .activation_budget(&budget.id)
                .is_some_and(|value| value >= 0 && value <= budget.initial_amount)
        });
        if expected_budgets != actual_budgets || !budget_values_valid {
            diagnostics.push(scenario_diagnostic(
                "RPG_CHECKPOINT_ACTIVATION_BUDGET_INVALID",
                format!("$.state.entities[{index}].activationBudgets"),
                "checkpoint activation budgets must exactly match the Ruleset and remain within their reachable range",
            ));
        }
    }
    let activation_count_valid = rules
        .accepted_activation_ceiling()
        .map_or(state.accepted_activations_this_turn() == 0, |ceiling| {
            state.accepted_activations_this_turn() <= ceiling
        });
    if !activation_count_valid {
        diagnostics.push(scenario_diagnostic(
            "RPG_CHECKPOINT_ACTIVATION_COUNT_INVALID",
            "$.state.acceptedActivationsThisTurn",
            "checkpoint accepted activation count is incompatible with the Ruleset ceiling",
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
        let item_binding_valid = restored_log_item_binding_is_valid(authority, rules, entry);
        if entry.sequence != expected_sequence
            || entry.state_revision == 0
            || entry.state_revision > state.revision()
            || !action_owned
            || !item_binding_valid
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

fn restored_log_item_binding_is_valid(
    authority: &RpgEncounterAuthority,
    rules: &rpg_compiler::CompiledRpgRules,
    entry: &RpgEncounterLogEntry,
) -> bool {
    let requirement = rules.binding_requirement(&entry.action_id);
    match (requirement, &entry.item_binding) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(requirement), Some(binding)) => {
            if binding.binding_id != requirement.id
                || requirement
                    .slot_ids
                    .binary_search(&binding.slot_id)
                    .is_err()
                || !rules
                    .bound_item_definition_ids(&entry.action_id)
                    .any(|definition_id| definition_id == binding.item_definition_id)
            {
                return false;
            }
            authority
                .scenario
                .participants
                .iter()
                .find(|participant| participant.id == entry.actor_id)
                .is_some_and(|participant| {
                    participant.items.iter().any(|item| {
                        item.id == binding.item_instance_id
                            && item.definition_id == binding.item_definition_id
                    }) && participant.equipment.iter().any(|equipment| {
                        equipment.slot_id == binding.slot_id
                            && equipment.item_instance_id == binding.item_instance_id
                    })
                })
        }
    }
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
    item_binding: Option<RpgIntentItemBinding>,
    item_label: Option<&str>,
    options: RpgActionOptionsView,
    unavailable: Option<RpgResolutionRejection>,
) -> RpgActionView {
    let has_options = !options.participant_ids.is_empty()
        || !options.cell_paths.is_empty()
        || !options.area_options.is_empty();
    RpgActionView {
        definition_id: action.id,
        label: item_label
            .map(|item_label| format!("{} — {item_label}", action.name))
            .unwrap_or(action.name),
        item_binding,
        available: unavailable.is_none() && has_options,
        unavailable,
        maximum_targets: action.targets.maximum_targets,
        activation: action.activation,
        options,
    }
}

pub(crate) struct RpgParticipantProjectionCatalogs<'a> {
    pub item_definitions: &'a BTreeMap<String, rpg_ir::CompiledItemDefinition>,
    pub effect_definitions: &'a BTreeMap<String, rpg_ir::CompiledEffectDefinition>,
    pub activation_budget_definitions: &'a [rpg_ir::RulesetActivationBudget],
}

pub(crate) fn participant_view(
    entity: &RpgEntityState,
    label: String,
    definition_ids: Vec<String>,
    items: &[RpgItemInstanceSetup],
    equipment: &[RpgEquipmentSlotSetup],
    catalogs: RpgParticipantProjectionCatalogs<'_>,
) -> RpgParticipantView {
    RpgParticipantView {
        id: entity.id().to_owned(),
        label,
        team_id: entity.team().clone(),
        position: entity.position(),
        definition_ids,
        class_definition_id: entity.class_definition_id().map(str::to_owned),
        feature_definition_ids: entity.character_feature_ids().to_vec(),
        items: items
            .iter()
            .map(|item| {
                let definition = catalogs.item_definitions.get(&item.definition_id);
                RpgItemInstanceView {
                    id: item.id.clone(),
                    definition_id: item.definition_id.clone(),
                    label: definition
                        .map(|definition| definition.label.clone())
                        .unwrap_or_else(|| item.definition_id.clone()),
                    description: definition.and_then(|definition| definition.description.clone()),
                    tags: definition
                        .map(|definition| definition.tags.clone())
                        .unwrap_or_default(),
                    traits: definition
                        .map(|definition| definition.traits.clone())
                        .unwrap_or_default(),
                    allowed_slots: definition
                        .map(|definition| definition.allowed_slots.clone())
                        .unwrap_or_default(),
                    attributes: definition
                        .map(|definition| definition.attributes.clone())
                        .unwrap_or_default(),
                }
            })
            .collect(),
        equipment: equipment.to_vec(),
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
        effects: entity
            .effects()
            .map(|effect| {
                let definition = catalogs.effect_definitions.get(effect.definition_id());
                RpgEffectView {
                    instance_id: effect.instance_id().to_owned(),
                    definition_id: effect.definition_id().to_owned(),
                    definition_version: effect.definition_version(),
                    label: definition
                        .map(|definition| definition.label.clone())
                        .unwrap_or_else(|| effect.definition_id().to_owned()),
                    source_entity_id: effect.source_entity_id().to_owned(),
                    stacking_id: effect.stacking_id().to_owned(),
                    stacking: effect.stacking(),
                    rank: effect.rank(),
                    duration_anchor: effect.duration_anchor(),
                    remaining_count: effect.remaining_count(),
                    application_revision: effect.application_revision(),
                    contributions: definition
                        .map(|definition| definition.contributions.clone())
                        .unwrap_or_default(),
                    outcome_band_shifts: definition
                        .map(|definition| definition.outcome_band_shifts.clone())
                        .unwrap_or_default(),
                    pool_contributions: definition
                        .map(|definition| definition.pool_contributions.clone())
                        .unwrap_or_default(),
                }
            })
            .collect(),
        activation_budgets: catalogs
            .activation_budget_definitions
            .iter()
            .map(|budget| RpgActivationBudgetView {
                id: budget.id.clone(),
                label: budget.label.clone(),
                timing: budget.timing,
                reset_boundary: budget.reset_boundary,
                initial_amount: budget.initial_amount,
                remaining: entity.activation_budget(&budget.id).unwrap_or(0),
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
            ("activationBudgets", json!([])),
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

    fn line_board(width: u32, height: u32, blocked: &[(u32, u32)]) -> RpgBoardSetup {
        RpgBoardSetup {
            width,
            height,
            cells: (0..height)
                .flat_map(|y| {
                    (0..width).map(move |x| RpgCellSetup {
                        id: format!("cell-{x}-{y}"),
                        position: GridPosition { x, y },
                        capabilities: vec![RpgCellCapabilitySetup {
                            id: RPG_LINE_OF_EFFECT_OBSTRUCTION_ID.to_owned(),
                            version: RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION,
                            definition_id: None,
                            value: RpgCellCapabilityValue::LineOfEffectObstruction {
                                blocks: blocked.contains(&(x, y)),
                            },
                        }],
                    })
                })
                .collect(),
        }
    }

    #[test]
    fn square_supercover_is_clear_and_reports_canonical_blockers() {
        let clear_board = line_board(4, 1, &[]);
        let state = RpgCapabilityState::restore(0, Vec::new()).unwrap();
        let clear_index = RpgAreaBoardIndex::new(&clear_board, &state);
        assert_eq!(
            line_of_effect_projection(
                &clear_index,
                GridPosition { x: 0, y: 0 },
                GridPosition { x: 3, y: 0 },
            ),
            RpgLineOfEffectProjection {
                clear: true,
                reason: None,
                blocking_cell_ids: Vec::new(),
            }
        );

        let blocked_board = line_board(5, 1, &[(1, 0), (3, 0)]);
        let blocked_index = RpgAreaBoardIndex::new(&blocked_board, &state);
        assert_eq!(
            line_of_effect_projection(
                &blocked_index,
                GridPosition { x: 0, y: 0 },
                GridPosition { x: 4, y: 0 },
            ),
            RpgLineOfEffectProjection {
                clear: false,
                reason: Some("lineOfEffectBlocked"),
                blocking_cell_ids: vec!["cell-1-0".to_owned(), "cell-3-0".to_owned()],
            }
        );
    }

    #[test]
    fn square_supercover_includes_both_cells_at_a_diagonal_corner_tie() {
        let board = line_board(3, 3, &[(1, 0), (0, 1)]);
        let state = RpgCapabilityState::restore(0, Vec::new()).unwrap();
        let index = RpgAreaBoardIndex::new(&board, &state);
        assert_eq!(
            line_of_effect_projection(
                &index,
                GridPosition { x: 0, y: 0 },
                GridPosition { x: 2, y: 2 },
            )
            .blocking_cell_ids,
            vec!["cell-1-0".to_owned(), "cell-0-1".to_owned()]
        );
    }

    #[test]
    fn square_supercover_fails_closed_when_an_endpoint_or_traversed_cell_is_missing() {
        let mut board = line_board(4, 1, &[]);
        board.cells.retain(|cell| cell.position.x != 2);
        let state = RpgCapabilityState::restore(0, Vec::new()).unwrap();
        let index = RpgAreaBoardIndex::new(&board, &state);
        assert_eq!(
            line_of_effect_projection(
                &index,
                GridPosition { x: 0, y: 0 },
                GridPosition { x: 3, y: 0 },
            )
            .reason,
            Some("lineOfEffectCellMissing")
        );
        assert_eq!(
            line_of_effect_projection(
                &index,
                GridPosition { x: 0, y: 0 },
                GridPosition { x: 4, y: 0 },
            )
            .reason,
            Some("lineOfEffectCellMissing")
        );
    }
}
