use std::collections::{BTreeMap, BTreeSet};

use rpg_core::{
    RpgCapabilityId, RpgContributionStackingPolicy, RpgRandomRequest, RpgRandomRequestKind,
    RpgSpatialSourceBoundary, MAXIMUM_RPG_DAMAGE_PARTS, MAXIMUM_RPG_DAMAGE_TAGS,
    MAXIMUM_RPG_MODIFIER_TURNS,
};
use rpg_ir::{
    CompiledCharacterFeature, CompiledEffectDefinition, CompiledItemDefinition,
    CompiledSpatialSourceDefinition, EquippedItemBindingRequirement, NormalizedRpgIr, RpgIrAction,
    RpgIrActivation, RpgIrActivationTiming, RpgIrCheck, RpgIrFormula, RpgIrOperation,
    RpgIrPredicate, RpgIrProgram, RpgIrRequirementKind, RpgIrResourceCost, RpgIrRollScope,
    RpgIrScalarTestDifficulty, RpgIrSubject, RpgIrTargetKind, RpgIrTargetSelector,
    RpgIrTeamConstraint, Ruleset, RulesetActivationBudget, RulesetHeterogeneousPoolProfile,
    RulesetScalarTestProfile, RPG_IR_IDENTITY, RPG_IR_MAJOR,
};
use serde::Serialize;

use crate::diagnostic::{RpgCompileFailure, RpgDiagnostic, RpgDiagnosticStage};
use crate::registry::{capability_version, operation_registration, RpgOperationRegistration};

const MAX_PROGRAM_DEPTH: usize = 16;
const MAX_PROGRAM_NODES: usize = 256;
const MAX_EXPANDED_PROGRAM_NODES: u64 = 4_096;
const MAX_EXPRESSION_DEPTH: usize = 16;
const MAX_EXPRESSION_NODES: usize = 256;
const MAX_REPEAT_COUNT: u32 = 16;
const MAX_TARGET_COUNT: u32 = 32;
const MAX_AREA_CELLS: u32 = 256;
const MAX_BOARD_EXTENT: u32 = 1_024;
const MAX_AREA_ANCHOR_RANGE: u32 = (MAX_BOARD_EXTENT - 1) * 2;
const MAX_DICE_COUNT: u32 = 64;
const MAX_DICE_SIDES: u32 = 1_000;

#[derive(Debug, Clone, Copy)]
enum CatalogKind {
    Stat,
    Defense,
    Resource,
    Modifier,
}

struct ProgramValidationState {
    node_count: usize,
    expanded_node_count: u64,
    action_target_maximum: u32,
    action_target_kind: RpgIrTargetKind,
    check_kind: CheckKind,
    outcome_branch_count: usize,
}

fn is_selected_destination_movement_program(program: &RpgIrProgram) -> bool {
    let RpgIrProgram::Atomic { body } = program else {
        return false;
    };
    let RpgIrProgram::OnCheck {
        hit,
        miss,
        saved,
        failed,
        no_roll,
    } = body.as_ref()
    else {
        return false;
    };
    if hit.is_some() || miss.is_some() || saved.is_some() || failed.is_some() {
        return false;
    }
    matches!(
        no_roll.as_deref(),
        Some(RpgIrProgram::Operation {
            operation: RpgIrOperation::MoveToCell { .. }
        })
    )
}

#[derive(Debug, Clone, Copy)]
enum CheckKind {
    NoRoll,
    Attack,
    SavingThrow,
    ScalarTest,
    HeterogeneousPool,
}

#[derive(Debug, Clone)]
pub struct CompiledRpgRules {
    package_id: String,
    package_version: String,
    capability_plan: BTreeMap<String, u32>,
    actions: BTreeMap<String, CompiledAction>,
    bound_actions: BTreeMap<(String, String), CompiledAction>,
    binding_requirements: BTreeMap<String, EquippedItemBindingRequirement>,
    character_features: BTreeMap<String, CompiledCharacterFeature>,
    effects: BTreeMap<String, CompiledEffectDefinition>,
    spatial_sources: BTreeMap<String, CompiledSpatialSourceDefinition>,
    spatial_source_triggers: BTreeMap<(String, RpgSpatialSourceBoundary), CompiledAction>,
    items: BTreeMap<String, CompiledItemDefinition>,
    calculation_selectors: BTreeMap<String, CompiledCalculationSelector>,
    contribution_stacking_groups: BTreeMap<String, RpgContributionStackingPolicy>,
    scalar_test_profiles: BTreeMap<String, CompiledScalarTestProfile>,
    heterogeneous_pool_profiles: BTreeMap<String, RulesetHeterogeneousPoolProfile>,
    activation_budgets: BTreeMap<String, RulesetActivationBudget>,
    movement_allowance_budget_id: Option<String>,
    accepted_activation_ceiling: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledCalculationSelector {
    pub(crate) minimum: i64,
    pub(crate) maximum: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledScalarTestProfile {
    pub(crate) definition: RulesetScalarTestProfile,
    pub(crate) minimum: i64,
    pub(crate) maximum: i64,
}

impl CompiledRpgRules {
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    pub fn package_version(&self) -> &str {
        &self.package_version
    }

    pub fn action_ids(&self) -> impl Iterator<Item = &str> {
        self.actions
            .keys()
            .chain(self.binding_requirements.keys())
            .map(String::as_str)
    }

    pub fn actions(&self) -> impl Iterator<Item = CompiledRpgAction> + '_ {
        self.actions
            .iter()
            .map(|(id, action)| compiled_action_projection(id, action, None))
            .chain(
                self.bound_actions
                    .iter()
                    .map(|((action_id, item_definition_id), action)| {
                        compiled_action_projection(
                            action_id,
                            action,
                            Some(CompiledEquippedItemActionBinding {
                                requirement: self
                                    .binding_requirements
                                    .get(action_id)
                                    .expect("bound action requirement exists")
                                    .clone(),
                                item_definition_id: item_definition_id.clone(),
                            }),
                        )
                    }),
            )
    }

    pub fn selected_destination_maximum_distance(&self, action_id: &str) -> Option<u32> {
        self.actions
            .get(action_id)
            .and_then(|action| selected_destination_maximum_distance(&action.program))
    }

    pub fn selected_destination_maximum_distance_for_binding(
        &self,
        action_id: &str,
        item_definition_id: Option<&str>,
    ) -> Option<u32> {
        self.action_for_binding(action_id, item_definition_id)
            .and_then(|action| selected_destination_maximum_distance(&action.program))
    }

    pub fn required_capabilities(&self) -> impl Iterator<Item = (&str, u32)> {
        self.capability_plan
            .iter()
            .map(|(id, version)| (id.as_str(), *version))
    }

    pub fn binding_requirement(&self, action_id: &str) -> Option<&EquippedItemBindingRequirement> {
        self.binding_requirements.get(action_id)
    }

    pub fn bound_item_definition_ids<'a>(
        &'a self,
        action_id: &'a str,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.bound_actions
            .keys()
            .filter(move |(candidate_action_id, _)| candidate_action_id == action_id)
            .map(|(_, item_definition_id)| item_definition_id.as_str())
    }

    pub(crate) fn action_for_binding(
        &self,
        action_id: &str,
        item_definition_id: Option<&str>,
    ) -> Option<&CompiledAction> {
        match item_definition_id {
            Some(item_definition_id) => self
                .bound_actions
                .get(&(action_id.to_owned(), item_definition_id.to_owned())),
            None => self.actions.get(action_id),
        }
    }

    pub(crate) fn register_bound_actions(&mut self, registrations: Vec<BoundActionRegistration>) {
        for registration in registrations {
            let action = self
                .actions
                .remove(&registration.compiled_action_id)
                .expect("bound action compilation produced its synthetic action");
            self.binding_requirements
                .entry(registration.action_id.clone())
                .or_insert_with(|| registration.requirement.clone());
            self.bound_actions.insert(
                (registration.action_id, registration.item_definition_id),
                action,
            );
        }
    }

    pub(crate) fn register_character_features(&mut self, features: &[CompiledCharacterFeature]) {
        self.character_features = features
            .iter()
            .cloned()
            .map(|feature| (feature.definition_id.clone(), feature))
            .collect();
    }

    pub(crate) fn register_items(&mut self, items: &[CompiledItemDefinition]) {
        self.items = items
            .iter()
            .cloned()
            .map(|item| (item.definition_id.clone(), item))
            .collect();
    }

    pub(crate) fn register_effects(&mut self, effects: &[CompiledEffectDefinition]) {
        self.effects = effects
            .iter()
            .cloned()
            .map(|effect| (effect.definition_id.clone(), effect))
            .collect();
    }

    pub(crate) fn register_spatial_sources(
        &mut self,
        definitions: &[CompiledSpatialSourceDefinition],
    ) {
        self.spatial_source_triggers = definitions
            .iter()
            .flat_map(|definition| {
                definition.triggers.iter().map(|trigger| {
                    let action_id = trigger.operation_path.clone();
                    let body = trigger.body.clone();
                    (
                        (definition.definition_id.clone(), trigger.boundary),
                        compile_action(
                            RpgIrAction {
                                id: action_id.clone(),
                                name: action_id,
                                source_path: trigger.operation_path.clone(),
                                tags: Vec::new(),
                                targets: body.targets,
                                check: body.check,
                                roll_scope: body.roll_scope,
                                costs: body.costs,
                                activation: body.activation,
                                program: body.program,
                            },
                            None,
                        ),
                    )
                })
            })
            .collect();
        self.spatial_sources = definitions
            .iter()
            .cloned()
            .map(|definition| (definition.definition_id.clone(), definition))
            .collect();
    }

    pub(crate) fn register_contribution_contracts(&mut self, ruleset: &Ruleset) {
        let domains = ruleset
            .provides
            .numeric_domains
            .iter()
            .map(|domain| (domain.id.as_str(), domain))
            .collect::<BTreeMap<_, _>>();
        self.calculation_selectors = ruleset
            .provides
            .calculation_selectors
            .iter()
            .filter_map(|selector| {
                domains
                    .get(selector.numeric_domain_id.as_str())
                    .map(|domain| {
                        (
                            selector.id.clone(),
                            CompiledCalculationSelector {
                                minimum: domain.minimum,
                                maximum: domain.maximum,
                            },
                        )
                    })
            })
            .collect();
        self.activation_budgets = ruleset
            .provides
            .activation_budgets
            .iter()
            .cloned()
            .map(|budget| (budget.id.clone(), budget))
            .collect();
        self.movement_allowance_budget_id = ruleset.provides.movement_allowance_budget_id.clone();
        self.accepted_activation_ceiling =
            ruleset.models.action_economy.accepted_activation_ceiling();
        self.contribution_stacking_groups = ruleset
            .provides
            .contribution_stacking_groups
            .iter()
            .map(|group| (group.id.clone(), group.policy))
            .collect();
        self.scalar_test_profiles = ruleset
            .provides
            .scalar_test_profiles
            .iter()
            .filter_map(|profile| {
                domains
                    .get(profile.numeric_domain_id.as_str())
                    .map(|domain| {
                        (
                            profile.id.clone(),
                            CompiledScalarTestProfile {
                                definition: profile.clone(),
                                minimum: domain.minimum,
                                maximum: domain.maximum,
                            },
                        )
                    })
            })
            .collect();
        self.heterogeneous_pool_profiles = ruleset
            .provides
            .heterogeneous_pool_profiles
            .iter()
            .cloned()
            .map(|profile| (profile.id.clone(), profile))
            .collect();
    }

    pub(crate) fn character_feature(
        &self,
        definition_id: &str,
    ) -> Option<&CompiledCharacterFeature> {
        self.character_features.get(definition_id)
    }

    pub fn movement_reactions_for_feature(
        &self,
        definition_id: &str,
    ) -> &[rpg_ir::RpgMovementReactionDefinition] {
        self.character_features
            .get(definition_id)
            .map(|feature| feature.movement_reactions.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn item(&self, definition_id: &str) -> Option<&CompiledItemDefinition> {
        self.items.get(definition_id)
    }

    pub(crate) fn effect(&self, definition_id: &str) -> Option<&CompiledEffectDefinition> {
        self.effects.get(definition_id)
    }

    pub fn spatial_source(&self, definition_id: &str) -> Option<&CompiledSpatialSourceDefinition> {
        self.spatial_sources.get(definition_id)
    }

    pub(crate) fn spatial_source_trigger(
        &self,
        definition_id: &str,
        boundary: RpgSpatialSourceBoundary,
    ) -> Option<&CompiledAction> {
        self.spatial_source_triggers
            .get(&(definition_id.to_owned(), boundary))
    }

    pub(crate) fn calculation_selector(
        &self,
        selector_id: &str,
    ) -> Option<&CompiledCalculationSelector> {
        self.calculation_selectors.get(selector_id)
    }

    pub(crate) fn contribution_stacking_policy(
        &self,
        group_id: &str,
    ) -> Option<RpgContributionStackingPolicy> {
        self.contribution_stacking_groups.get(group_id).copied()
    }

    pub(crate) fn scalar_test_profile(
        &self,
        profile_id: &str,
    ) -> Option<&CompiledScalarTestProfile> {
        self.scalar_test_profiles.get(profile_id)
    }

    pub(crate) fn heterogeneous_pool_profile(
        &self,
        profile_id: &str,
    ) -> Option<&RulesetHeterogeneousPoolProfile> {
        self.heterogeneous_pool_profiles.get(profile_id)
    }

    pub fn uses_variable_activation_budgets(&self) -> bool {
        self.accepted_activation_ceiling.is_some()
    }

    pub fn accepted_activation_ceiling(&self) -> Option<u32> {
        self.accepted_activation_ceiling
    }

    pub fn activation_budgets(&self) -> impl Iterator<Item = &RulesetActivationBudget> {
        self.activation_budgets.values()
    }

    pub fn movement_allowance_budget(&self) -> Option<&RulesetActivationBudget> {
        self.movement_allowance_budget_id
            .as_ref()
            .and_then(|id| self.activation_budgets.get(id))
    }

    pub fn action_activation_timing_for_binding(
        &self,
        action_id: &str,
        item_definition_id: Option<&str>,
    ) -> Option<rpg_ir::RpgIrActivationTiming> {
        self.action_for_binding(action_id, item_definition_id)
            .and_then(|action| {
                action
                    .activation
                    .as_ref()
                    .map(|activation| activation.timing)
            })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BoundActionRegistration {
    pub(crate) compiled_action_id: String,
    pub(crate) action_id: String,
    pub(crate) item_definition_id: String,
    pub(crate) requirement: EquippedItemBindingRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledRpgAction {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub tags: Vec<String>,
    pub targets: RpgIrTargetSelector,
    pub check: RpgIrCheck,
    pub roll_scope: RpgIrRollScope,
    pub costs: Vec<RpgIrResourceCost>,
    pub activation: Option<RpgIrActivation>,
    pub random_plan: Vec<RpgRandomPlanEntry>,
    pub selected_destination_maximum_distance: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<CompiledEquippedItemActionBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledEquippedItemActionBinding {
    pub requirement: EquippedItemBindingRequirement,
    pub item_definition_id: String,
}

fn compiled_action_projection(
    id: &str,
    action: &CompiledAction,
    binding: Option<CompiledEquippedItemActionBinding>,
) -> CompiledRpgAction {
    CompiledRpgAction {
        id: id.to_owned(),
        name: action.name.clone(),
        source_path: action.source_path.clone(),
        tags: action.tags.clone(),
        targets: action.targets.clone(),
        check: action.check.clone(),
        roll_scope: action.roll_scope,
        costs: action.costs.clone(),
        activation: action.activation.clone(),
        random_plan: action.random_plan.clone(),
        selected_destination_maximum_distance: selected_destination_maximum_distance(
            &action.program,
        ),
        binding,
    }
}

fn selected_destination_maximum_distance(program: &CompiledProgram) -> Option<u32> {
    let CompiledProgram::Atomic(body) = program else {
        return None;
    };
    let CompiledProgram::OnCheck {
        hit: None,
        miss: None,
        saved: None,
        failed: None,
        no_roll: Some(no_roll),
    } = body.as_ref()
    else {
        return None;
    };
    let CompiledProgram::Operation(operation) = no_roll.as_ref() else {
        return None;
    };
    match operation.declaration {
        RpgIrOperation::MoveToCell {
            maximum_distance, ..
        } => Some(maximum_distance),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
/// A closed authority branch that must be selected before a catalog random
/// request becomes required.
pub enum RpgRandomPlanConditionKind {
    WhenThen,
    WhenOtherwise,
    CheckHit,
    CheckMiss,
    CheckSaved,
    CheckFailed,
    CheckNoRoll,
    OutcomeBranch,
    OutcomeDefault,
    AllPreviousTrue,
    AnyPreviousFalse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpgRandomPlanCondition {
    pub kind: RpgRandomPlanConditionKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One possible random request and the complete branch condition stack that
/// guards it. An empty condition list means the request is unconditional;
/// sibling entries with exclusive conditions are alternatives, not a union of
/// evidence that callers should submit together.
pub struct RpgRandomPlanEntry {
    pub request: RpgRandomRequest,
    pub conditions: Vec<RpgRandomPlanCondition>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledAction {
    pub(crate) name: String,
    pub(crate) source_path: String,
    pub(crate) tags: Vec<String>,
    pub(crate) targets: RpgIrTargetSelector,
    pub(crate) check: RpgIrCheck,
    pub(crate) roll_scope: RpgIrRollScope,
    pub(crate) costs: Vec<RpgIrResourceCost>,
    pub(crate) activation: Option<RpgIrActivation>,
    pub(crate) program: CompiledProgram,
    pub(crate) random_plan: Vec<RpgRandomPlanEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledOperation {
    pub(crate) binding: &'static RpgOperationRegistration,
    pub(crate) declaration: RpgIrOperation,
}

#[derive(Debug, Clone)]
pub(crate) enum CompiledProgram {
    Operation(CompiledOperation),
    Sequence(Vec<CompiledProgram>),
    When {
        predicate: RpgIrPredicate,
        then: Box<CompiledProgram>,
        otherwise: Option<Box<CompiledProgram>>,
    },
    Repeat {
        count: u32,
        body: Box<CompiledProgram>,
    },
    ForEachTarget {
        maximum: u32,
        body: Box<CompiledProgram>,
    },
    OnCheck {
        hit: Option<Box<CompiledProgram>>,
        miss: Option<Box<CompiledProgram>>,
        saved: Option<Box<CompiledProgram>>,
        failed: Option<Box<CompiledProgram>>,
        no_roll: Option<Box<CompiledProgram>>,
    },
    OnOutcome {
        branches: BTreeMap<String, Box<CompiledProgram>>,
        default: Box<CompiledProgram>,
    },
    Atomic(Box<CompiledProgram>),
}

pub fn compile_normalized_rpg_json(source: &[u8]) -> Result<CompiledRpgRules, RpgCompileFailure> {
    let decoded =
        serde_json::from_slice::<NormalizedRpgIr>(source).map_err(|error| RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Decode,
                "RPG_IR_DECODE_FAILED",
                "$",
                error.to_string(),
            )],
        })?;
    compile_normalized_rpg_ir(decoded)
}

pub fn compile_normalized_rpg_ir(
    source: NormalizedRpgIr,
) -> Result<CompiledRpgRules, RpgCompileFailure> {
    if let Some((index, _)) = source
        .actions
        .iter()
        .enumerate()
        .find(|(_, action)| matches!(action.check, RpgIrCheck::ScalarTest { .. }))
    {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RPG_IR_SCALAR_TEST_RULESET_REQUIRED",
                format!("$.actions[{index}].check.profile"),
                "a scalar-test action must be compiled through a PlayBundle with its exact Ruleset",
            )],
        });
    }
    if let Some((index, path)) = source
        .actions
        .iter()
        .enumerate()
        .find_map(|(index, action)| {
            action
                .activation
                .as_ref()
                .map(|_| (index, "activation"))
                .or_else(|| program_activation_path(&action.program).map(|path| (index, path)))
        })
    {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RPG_IR_ACTIVATION_RULESET_REQUIRED",
                format!("$.actions[{index}].{path}"),
                "activation budgets must be compiled through a PlayBundle with their exact Ruleset",
            )],
        });
    }
    compile_normalized_rpg_ir_with_ruleset(source, None)
}

fn program_activation_path(program: &RpgIrProgram) -> Option<&'static str> {
    match program {
        RpgIrProgram::Operation {
            operation: RpgIrOperation::OpenReaction { options, .. },
        } if options.iter().any(|option| option.activation.is_some()) => {
            Some("program.operation.options.activation")
        }
        RpgIrProgram::Operation { .. } => None,
        RpgIrProgram::Sequence { steps } => steps.iter().find_map(program_activation_path),
        RpgIrProgram::When {
            then, otherwise, ..
        } => program_activation_path(then)
            .or_else(|| otherwise.as_deref().and_then(program_activation_path)),
        RpgIrProgram::Repeat { body, .. }
        | RpgIrProgram::ForEachTarget { body, .. }
        | RpgIrProgram::Atomic { body } => program_activation_path(body),
        RpgIrProgram::OnCheck {
            hit,
            miss,
            saved,
            failed,
            no_roll,
        } => [hit, miss, saved, failed, no_roll]
            .into_iter()
            .flatten()
            .find_map(|branch| program_activation_path(branch)),
        RpgIrProgram::OnOutcome { branches, default } => branches
            .values()
            .find_map(|branch| program_activation_path(branch))
            .or_else(|| program_activation_path(default)),
    }
}

pub(crate) fn compile_normalized_rpg_ir_with_ruleset(
    source: NormalizedRpgIr,
    ruleset: Option<&Ruleset>,
) -> Result<CompiledRpgRules, RpgCompileFailure> {
    let mut validator = Validator::new(&source);
    validator.validate();
    if !validator.diagnostics.is_empty() {
        return Err(RpgCompileFailure {
            diagnostics: validator.diagnostics,
        });
    }
    drop(validator);

    let capability_plan = source
        .requirements
        .iter()
        .filter(|requirement| requirement.kind == RpgIrRequirementKind::Capability)
        .map(|requirement| (requirement.id.clone(), requirement.version))
        .collect();

    Ok(CompiledRpgRules {
        package_id: source.package.id,
        package_version: source.package.version,
        capability_plan,
        actions: source
            .actions
            .into_iter()
            .map(|action| {
                let id = action.id.clone();
                let compiled = compile_action(action, ruleset);
                (id, compiled)
            })
            .collect(),
        bound_actions: BTreeMap::new(),
        binding_requirements: BTreeMap::new(),
        character_features: BTreeMap::new(),
        effects: BTreeMap::new(),
        spatial_sources: BTreeMap::new(),
        spatial_source_triggers: BTreeMap::new(),
        items: BTreeMap::new(),
        calculation_selectors: BTreeMap::new(),
        contribution_stacking_groups: BTreeMap::new(),
        scalar_test_profiles: BTreeMap::new(),
        heterogeneous_pool_profiles: BTreeMap::new(),
        activation_budgets: BTreeMap::new(),
        movement_allowance_budget_id: None,
        accepted_activation_ceiling: None,
    })
}

fn compile_action(action: RpgIrAction, ruleset: Option<&Ruleset>) -> CompiledAction {
    let random_plan = collect_action_random_plan(&action, ruleset);
    CompiledAction {
        name: action.name,
        source_path: action.source_path,
        tags: action.tags,
        targets: action.targets,
        check: action.check,
        roll_scope: action.roll_scope,
        costs: action.costs,
        activation: action.activation,
        program: compile_program(action.program),
        random_plan,
    }
}

fn collect_action_random_plan(
    action: &RpgIrAction,
    ruleset: Option<&Ruleset>,
) -> Vec<RpgRandomPlanEntry> {
    let mut plan = Vec::new();
    if !matches!(action.check, RpgIrCheck::NoRoll) {
        if let RpgIrCheck::HeterogeneousPool {
            profile, base_dice, ..
        } = &action.check
        {
            let profile = ruleset.and_then(|ruleset| {
                ruleset
                    .provides
                    .heterogeneous_pool_profiles
                    .iter()
                    .find(|candidate| candidate.id == profile.id)
            });
            let heterogeneous_terms = base_dice
                .iter()
                .filter_map(|term| {
                    profile.and_then(|profile| {
                        profile
                            .die_types
                            .iter()
                            .find(|die| die.id == term.die_type_id)
                            .map(|die| rpg_core::RpgHeterogeneousRandomTerm {
                                die_type_id: term.die_type_id.clone(),
                                count: term.count,
                                sides: die.sides,
                            })
                    })
                })
                .collect::<Vec<_>>();
            let count = heterogeneous_terms
                .iter()
                .fold(0_u32, |total, term| total.saturating_add(term.count));
            plan.push(RpgRandomPlanEntry {
                request: RpgRandomRequest {
                    kind: RpgRandomRequestKind::HeterogeneousPool,
                    count,
                    sides: 0,
                    path: "$.action.check".to_owned(),
                    heterogeneous_terms,
                },
                conditions: Vec::new(),
            });
            collect_program_random_plan(&action.program, "$.action.program", &[], &mut plan);
            return plan;
        }
        let kind = match action.check {
            RpgIrCheck::Attack { .. } => RpgRandomRequestKind::AttackCheck,
            RpgIrCheck::SavingThrow { .. } => RpgRandomRequestKind::SavingThrowCheck,
            RpgIrCheck::ScalarTest { .. } => RpgRandomRequestKind::ScalarTest,
            RpgIrCheck::HeterogeneousPool { .. } => unreachable!(),
            RpgIrCheck::NoRoll => unreachable!(),
        };
        let sides = match &action.check {
            RpgIrCheck::ScalarTest { profile, .. } => ruleset
                .and_then(|ruleset| {
                    ruleset
                        .provides
                        .scalar_test_profiles
                        .iter()
                        .find(|candidate| candidate.id == profile.id)
                })
                .map_or(0, |profile| profile.die_sides),
            RpgIrCheck::HeterogeneousPool { .. } => unreachable!(),
            _ => 20,
        };
        let count = match action.roll_scope {
            RpgIrRollScope::Shared => 1,
            RpgIrRollScope::PerTarget => action.targets.maximum_targets,
            RpgIrRollScope::None => 0,
        };
        plan.push(RpgRandomPlanEntry {
            request: RpgRandomRequest {
                kind,
                count,
                sides,
                path: "$.action.check".to_owned(),
                heterogeneous_terms: Vec::new(),
            },
            conditions: Vec::new(),
        });
    }
    collect_program_random_plan(&action.program, "$.action.program", &[], &mut plan);
    plan
}

fn collect_program_random_plan(
    program: &RpgIrProgram,
    path: &str,
    conditions: &[RpgRandomPlanCondition],
    plan: &mut Vec<RpgRandomPlanEntry>,
) {
    match program {
        RpgIrProgram::Operation { operation } => match operation {
            RpgIrOperation::Damage { parts } => {
                for (index, part) in parts.iter().enumerate() {
                    collect_formula_random_plan(
                        &part.amount,
                        &format!("{path}.parts[{index}].amount"),
                        conditions,
                        plan,
                    );
                }
            }
            RpgIrOperation::Heal { amount } => {
                collect_formula_random_plan(amount, &format!("{path}.amount"), conditions, plan);
            }
            RpgIrOperation::ChangeResource { delta, .. } => {
                collect_formula_random_plan(delta, &format!("{path}.delta"), conditions, plan);
            }
            RpgIrOperation::ApplyModifier { value, .. } => {
                collect_formula_random_plan(value, &format!("{path}.value"), conditions, plan);
            }
            RpgIrOperation::ApplyEffect { rank, .. } => {
                collect_formula_random_plan(rank, &format!("{path}.rank"), conditions, plan);
            }
            RpgIrOperation::RemoveEffect { .. } => {}
            RpgIrOperation::Move {
                delta_x, delta_y, ..
            } => {
                collect_formula_random_plan(delta_x, &format!("{path}.deltaX"), conditions, plan);
                collect_formula_random_plan(delta_y, &format!("{path}.deltaY"), conditions, plan);
            }
            RpgIrOperation::MoveToCell { .. }
            | RpgIrOperation::Push { .. }
            | RpgIrOperation::Slide { .. } => {}
            RpgIrOperation::CreateSpatialSource { .. } => {}
            RpgIrOperation::OpenReaction { .. } => {}
        },
        RpgIrProgram::Sequence { steps } => {
            for (index, step) in steps.iter().enumerate() {
                collect_program_random_plan(
                    step,
                    &format!("{path}.steps[{index}]"),
                    conditions,
                    plan,
                );
            }
        }
        RpgIrProgram::When {
            predicate,
            then,
            otherwise,
        } => {
            collect_predicate_random_plan(
                predicate,
                &format!("{path}.predicate"),
                conditions,
                plan,
            );
            let then_conditions =
                with_condition(conditions, RpgRandomPlanConditionKind::WhenThen, path);
            collect_program_random_plan(then, &format!("{path}.then"), &then_conditions, plan);
            if let Some(otherwise) = otherwise {
                let otherwise_conditions =
                    with_condition(conditions, RpgRandomPlanConditionKind::WhenOtherwise, path);
                collect_program_random_plan(
                    otherwise,
                    &format!("{path}.otherwise"),
                    &otherwise_conditions,
                    plan,
                );
            }
        }
        RpgIrProgram::Repeat { count, body } => {
            let start = plan.len();
            collect_program_random_plan(body, &format!("{path}.body"), conditions, plan);
            for entry in &mut plan[start..] {
                entry.request.count = entry.request.count.saturating_mul(*count);
            }
        }
        RpgIrProgram::ForEachTarget { maximum, body } => {
            let start = plan.len();
            collect_program_random_plan(body, &format!("{path}.body"), conditions, plan);
            for entry in &mut plan[start..] {
                entry.request.count = entry.request.count.saturating_mul(*maximum);
            }
        }
        RpgIrProgram::OnCheck {
            hit,
            miss,
            saved,
            failed,
            no_roll,
        } => {
            for (label, condition_kind, branch) in [
                ("hit", RpgRandomPlanConditionKind::CheckHit, hit),
                ("miss", RpgRandomPlanConditionKind::CheckMiss, miss),
                ("saved", RpgRandomPlanConditionKind::CheckSaved, saved),
                ("failed", RpgRandomPlanConditionKind::CheckFailed, failed),
                ("noRoll", RpgRandomPlanConditionKind::CheckNoRoll, no_roll),
            ] {
                if let Some(branch) = branch {
                    let branch_conditions = with_condition(conditions, condition_kind, path);
                    collect_program_random_plan(
                        branch,
                        &format!("{path}.{label}"),
                        &branch_conditions,
                        plan,
                    );
                }
            }
        }
        RpgIrProgram::OnOutcome { branches, default } => {
            for (band_id, branch) in branches {
                let branch_path = format!("{path}.branches.{band_id}");
                let branch_conditions = with_condition(
                    conditions,
                    RpgRandomPlanConditionKind::OutcomeBranch,
                    &branch_path,
                );
                collect_program_random_plan(branch, &branch_path, &branch_conditions, plan);
            }
            let default_path = format!("{path}.default");
            let default_conditions = with_condition(
                conditions,
                RpgRandomPlanConditionKind::OutcomeDefault,
                &default_path,
            );
            collect_program_random_plan(default, &default_path, &default_conditions, plan);
        }
        RpgIrProgram::Atomic { body } => {
            collect_program_random_plan(body, &format!("{path}.body"), conditions, plan);
        }
    }
}

fn collect_predicate_random_plan(
    predicate: &RpgIrPredicate,
    path: &str,
    conditions: &[RpgRandomPlanCondition],
    plan: &mut Vec<RpgRandomPlanEntry>,
) {
    match predicate {
        RpgIrPredicate::Always => {}
        RpgIrPredicate::Compare { left, right, .. } => {
            collect_formula_random_plan(left, &format!("{path}.left"), conditions, plan);
            collect_formula_random_plan(right, &format!("{path}.right"), conditions, plan);
        }
        RpgIrPredicate::Not { predicate } => {
            collect_predicate_random_plan(
                predicate,
                &format!("{path}.predicate"),
                conditions,
                plan,
            );
        }
        RpgIrPredicate::All { predicates } => {
            for (index, predicate) in predicates.iter().enumerate() {
                let predicate_conditions = if index == 0 {
                    conditions.to_vec()
                } else {
                    with_condition(
                        conditions,
                        RpgRandomPlanConditionKind::AllPreviousTrue,
                        &format!("{path}[0..{index}]"),
                    )
                };
                collect_predicate_random_plan(
                    predicate,
                    &format!("{path}[{index}]"),
                    &predicate_conditions,
                    plan,
                );
            }
        }
        RpgIrPredicate::Any { predicates } => {
            for (index, predicate) in predicates.iter().enumerate() {
                let predicate_conditions = if index == 0 {
                    conditions.to_vec()
                } else {
                    with_condition(
                        conditions,
                        RpgRandomPlanConditionKind::AnyPreviousFalse,
                        &format!("{path}[0..{index}]"),
                    )
                };
                collect_predicate_random_plan(
                    predicate,
                    &format!("{path}[{index}]"),
                    &predicate_conditions,
                    plan,
                );
            }
        }
    }
}

fn collect_formula_random_plan(
    formula: &RpgIrFormula,
    path: &str,
    conditions: &[RpgRandomPlanCondition],
    plan: &mut Vec<RpgRandomPlanEntry>,
) {
    match formula {
        RpgIrFormula::Dice { count, sides, .. } => plan.push(RpgRandomPlanEntry {
            request: RpgRandomRequest {
                kind: RpgRandomRequestKind::FormulaDice,
                count: *count,
                sides: *sides,
                path: path.to_owned(),
                heterogeneous_terms: Vec::new(),
            },
            conditions: conditions.to_vec(),
        }),
        RpgIrFormula::Add { terms } => {
            for (index, term) in terms.iter().enumerate() {
                collect_formula_random_plan(
                    term,
                    &format!("{path}.terms[{index}]"),
                    conditions,
                    plan,
                );
            }
        }
        RpgIrFormula::Half { value } => {
            collect_formula_random_plan(value, &format!("{path}.value"), conditions, plan);
        }
        RpgIrFormula::Constant { .. } | RpgIrFormula::ReadStat { .. } => {}
    }
}

fn with_condition(
    conditions: &[RpgRandomPlanCondition],
    kind: RpgRandomPlanConditionKind,
    path: &str,
) -> Vec<RpgRandomPlanCondition> {
    let mut result = conditions.to_vec();
    result.push(RpgRandomPlanCondition {
        kind,
        path: path.to_owned(),
    });
    result
}

fn compile_program(program: RpgIrProgram) -> CompiledProgram {
    match program {
        RpgIrProgram::Operation { operation } => {
            let binding = operation_registration(operation.registration_id())
                .expect("validated operation must have a static binding");
            CompiledProgram::Operation(CompiledOperation {
                binding,
                declaration: operation,
            })
        }
        RpgIrProgram::Sequence { steps } => {
            CompiledProgram::Sequence(steps.into_iter().map(compile_program).collect())
        }
        RpgIrProgram::When {
            predicate,
            then,
            otherwise,
        } => CompiledProgram::When {
            predicate,
            then: Box::new(compile_program(*then)),
            otherwise: otherwise.map(|program| Box::new(compile_program(*program))),
        },
        RpgIrProgram::Repeat { count, body } => CompiledProgram::Repeat {
            count,
            body: Box::new(compile_program(*body)),
        },
        RpgIrProgram::ForEachTarget { maximum, body } => CompiledProgram::ForEachTarget {
            maximum,
            body: Box::new(compile_program(*body)),
        },
        RpgIrProgram::OnCheck {
            hit,
            miss,
            saved,
            failed,
            no_roll,
        } => CompiledProgram::OnCheck {
            hit: hit.map(|program| Box::new(compile_program(*program))),
            miss: miss.map(|program| Box::new(compile_program(*program))),
            saved: saved.map(|program| Box::new(compile_program(*program))),
            failed: failed.map(|program| Box::new(compile_program(*program))),
            no_roll: no_roll.map(|program| Box::new(compile_program(*program))),
        },
        RpgIrProgram::OnOutcome { branches, default } => CompiledProgram::OnOutcome {
            branches: branches
                .into_iter()
                .map(|(band_id, program)| (band_id, Box::new(compile_program(*program))))
                .collect(),
            default: Box::new(compile_program(*default)),
        },
        RpgIrProgram::Atomic { body } => CompiledProgram::Atomic(Box::new(compile_program(*body))),
    }
}

struct Validator<'a> {
    source: &'a NormalizedRpgIr,
    diagnostics: Vec<RpgDiagnostic>,
    requirements: BTreeMap<(u8, String), u32>,
    operation_ids: BTreeSet<String>,
    capability_ids: BTreeSet<String>,
    stats: BTreeSet<String>,
    defenses: BTreeSet<String>,
    resources: BTreeSet<String>,
    modifiers: BTreeSet<String>,
}

impl<'a> Validator<'a> {
    fn new(source: &'a NormalizedRpgIr) -> Self {
        Self {
            source,
            diagnostics: Vec::new(),
            requirements: BTreeMap::new(),
            operation_ids: BTreeSet::new(),
            capability_ids: BTreeSet::new(),
            stats: source.catalogs.stats.iter().cloned().collect(),
            defenses: source.catalogs.defenses.iter().cloned().collect(),
            resources: source.catalogs.resources.iter().cloned().collect(),
            modifiers: source.catalogs.modifiers.iter().cloned().collect(),
        }
    }

    fn validate(&mut self) {
        self.validate_compatibility();
        self.validate_catalogs();
        self.validate_requirements();
        self.validate_actions();
    }

    fn validate_compatibility(&mut self) {
        if self.source.schema.identity != RPG_IR_IDENTITY {
            self.error(
                RpgDiagnosticStage::Compatibility,
                "RPG_IR_IDENTITY_UNSUPPORTED",
                "$.schema.identity",
                format!("expected {RPG_IR_IDENTITY}"),
            );
        }
        if self.source.schema.major != RPG_IR_MAJOR {
            self.error(
                RpgDiagnosticStage::Compatibility,
                "RPG_IR_MAJOR_UNSUPPORTED",
                "$.schema.major",
                format!("supported major is {RPG_IR_MAJOR}"),
            );
        }
        self.require_identifier(&self.source.package.id, "$.package.id");
        self.require_nonempty(
            &self.source.package.version,
            "$.package.version",
            "package version",
        );
    }

    fn validate_catalogs(&mut self) {
        self.validate_catalog(&self.source.catalogs.stats, "$.catalogs.stats");
        self.validate_catalog(&self.source.catalogs.defenses, "$.catalogs.defenses");
        self.validate_catalog(&self.source.catalogs.resources, "$.catalogs.resources");
        self.validate_catalog(&self.source.catalogs.modifiers, "$.catalogs.modifiers");
        self.validate_catalog(
            &self.source.catalogs.capabilities,
            "$.catalogs.capabilities",
        );
        self.capability_ids = self.source.catalogs.capabilities.iter().cloned().collect();
    }

    fn validate_catalog(&mut self, values: &[String], path: &str) {
        let mut seen = BTreeSet::new();
        for (index, value) in values.iter().enumerate() {
            self.require_identifier(value, &format!("{path}[{index}]"));
            if !seen.insert(value) {
                self.error(
                    RpgDiagnosticStage::References,
                    "RPG_IR_DUPLICATE_CATALOG_ID",
                    format!("{path}[{index}]"),
                    format!("duplicate catalog id {value}"),
                );
            }
        }
    }

    fn validate_requirements(&mut self) {
        for (index, requirement) in self.source.requirements.iter().enumerate() {
            let path = format!("$.requirements[{index}]");
            self.require_identifier(&requirement.id, &format!("{path}.id"));
            let kind = requirement_kind_key(requirement.kind);
            if self
                .requirements
                .insert((kind, requirement.id.clone()), requirement.version)
                .is_some()
            {
                self.error(
                    RpgDiagnosticStage::Requirements,
                    "RPG_IR_DUPLICATE_REQUIREMENT",
                    path.clone(),
                    format!("duplicate requirement {}", requirement.id),
                );
                continue;
            }

            let supported = match requirement.kind {
                RpgIrRequirementKind::Operation => {
                    operation_registration(&requirement.id).map(|value| value.version)
                }
                RpgIrRequirementKind::Capability => capability_version(&requirement.id),
            };
            if supported != Some(requirement.version) {
                self.diagnostics.push(
                    RpgDiagnostic::error(
                        RpgDiagnosticStage::Requirements,
                        "RPG_IR_REQUIREMENT_UNSUPPORTED",
                        &path,
                        format!(
                            "unsupported requirement {} version {}",
                            requirement.id, requirement.version
                        ),
                    )
                    .with_requirement(format!("{}@{}", requirement.id, requirement.version)),
                );
            }

            match requirement.kind {
                RpgIrRequirementKind::Operation => {
                    self.operation_ids.insert(requirement.id.clone());
                }
                RpgIrRequirementKind::Capability => {
                    if !self.capability_ids.contains(&requirement.id) {
                        self.error(
                            RpgDiagnosticStage::References,
                            "RPG_IR_CAPABILITY_NOT_CATALOGED",
                            &path,
                            format!(
                                "capability {} is not in the capability catalog",
                                requirement.id
                            ),
                        );
                    }
                }
            }
        }
    }

    fn validate_actions(&mut self) {
        let mut action_ids = BTreeSet::new();
        for (index, action) in self.source.actions.iter().enumerate() {
            let path = format!("$.actions[{index}]");
            self.require_identifier(&action.id, &format!("{path}.id"));
            self.require_nonempty(&action.name, &format!("{path}.name"), "action name");
            self.require_nonempty(
                &action.source_path,
                &format!("{path}.sourcePath"),
                "source path",
            );
            let mut previous_tag = None::<&str>;
            for (tag_index, tag) in action.tags.iter().enumerate() {
                if !is_portable_identifier(tag)
                    || previous_tag.is_some_and(|previous| previous >= tag.as_str())
                {
                    self.error(
                        RpgDiagnosticStage::Artifact,
                        "RPG_IR_ACTION_TAGS_NOT_CANONICAL",
                        format!("{path}.tags[{tag_index}]"),
                        "action tags must be unique sorted portable identifiers",
                    );
                }
                previous_tag = Some(tag);
            }
            if !action_ids.insert(&action.id) {
                self.error(
                    RpgDiagnosticStage::References,
                    "RPG_IR_DUPLICATE_ACTION_ID",
                    format!("{path}.id"),
                    format!("duplicate action id {}", action.id),
                );
            }
            if action.targets.maximum_targets == 0
                || action.targets.maximum_targets > MAX_TARGET_COUNT
            {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_TARGET_BOUND_INVALID",
                    format!("{path}.targets.maximumTargets"),
                    format!("target maximum must be between 1 and {MAX_TARGET_COUNT}"),
                );
            }
            if action.targets.kind == RpgIrTargetKind::Cell {
                if action.targets.team != RpgIrTeamConstraint::Any
                    || action.targets.maximum_targets != 1
                    || action.targets.area.is_some()
                {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_CELL_TARGET_INVALID",
                        format!("{path}.targets"),
                        "cell targets require team any and exactly one destination",
                    );
                }
                if !matches!(action.check, RpgIrCheck::NoRoll) {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_CELL_CHECK_INVALID",
                        format!("{path}.check"),
                        "cell-target actions require a no-roll check",
                    );
                }
            }
            match (&action.targets.kind, &action.targets.area) {
                (RpgIrTargetKind::Area, Some(area)) => {
                    let schema_valid = area.schema.identity == "asha.rpg.area-selector"
                        && area.schema.version == 1;
                    let shape_cells = match (&area.origin, &area.shape) {
                        (
                            rpg_ir::RpgIrAreaOrigin::Anchor,
                            rpg_ir::RpgIrAreaShape::Diamond { radius },
                        ) if *radius <= MAX_BOARD_EXTENT => radius
                            .checked_add(1)
                            .and_then(|next| radius.checked_mul(next))
                            .and_then(|product| product.checked_mul(2))
                            .and_then(|product| product.checked_add(1)),
                        (
                            rpg_ir::RpgIrAreaOrigin::Actor,
                            rpg_ir::RpgIrAreaShape::OrthogonalLine { length },
                        ) if (1..=MAX_AREA_CELLS).contains(length) => Some(*length),
                        _ => None,
                    };
                    if !schema_valid
                        || action.targets.maximum_range > MAX_AREA_ANCHOR_RANGE
                        || area.minimum_targets > action.targets.maximum_targets
                        || shape_cells.is_none_or(|count| count > MAX_AREA_CELLS)
                    {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_AREA_TARGET_INVALID",
                            format!("{path}.targets.area"),
                            "area targets require the versioned bounded diamond/anchor or orthogonal-line/actor contract",
                        );
                    }
                }
                (RpgIrTargetKind::Area, None) => self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_AREA_TARGET_REQUIRED",
                    format!("{path}.targets.area"),
                    "area-target actions require an area selector",
                ),
                (_, Some(_)) => self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_AREA_TARGET_UNEXPECTED",
                    format!("{path}.targets.area"),
                    "only area-target actions may declare an area selector",
                ),
                (_, None) => {}
            }
            self.validate_check(action, &path);
            if let Some(activation) = &action.activation {
                self.validate_activation(
                    activation,
                    activation.timing,
                    &format!("{path}.activation"),
                );
            }
            for (cost_index, cost) in action.costs.iter().enumerate() {
                let cost_path = format!("{path}.costs[{cost_index}]");
                self.require_reference(
                    CatalogKind::Resource,
                    &cost.resource_id,
                    &format!("{cost_path}.resourceId"),
                    "resource",
                );
                if cost.amount <= 0 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_COST_INVALID",
                        format!("{cost_path}.amount"),
                        "resource cost must be positive",
                    );
                }
                self.require_capability("capability.resources", &cost_path);
            }
            let mut program_state = ProgramValidationState {
                node_count: 0,
                expanded_node_count: 0,
                action_target_maximum: action.targets.maximum_targets,
                action_target_kind: action.targets.kind,
                check_kind: match &action.check {
                    RpgIrCheck::NoRoll => CheckKind::NoRoll,
                    RpgIrCheck::Attack { .. } => CheckKind::Attack,
                    RpgIrCheck::SavingThrow { .. } => CheckKind::SavingThrow,
                    RpgIrCheck::ScalarTest { .. } => CheckKind::ScalarTest,
                    RpgIrCheck::HeterogeneousPool { .. } => CheckKind::HeterogeneousPool,
                },
                outcome_branch_count: 0,
            };
            self.validate_program(
                &action.program,
                &format!("{path}.program"),
                1,
                1,
                false,
                &mut program_state,
            );
            if action.targets.kind == RpgIrTargetKind::Area
                && !area_program_covers_target_maximum(
                    &action.program,
                    action.targets.maximum_targets,
                )
            {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_AREA_PROGRAM_BOUND_INVALID",
                    format!("{path}.program"),
                    "every area-target for-each bound must equal the selector maximum",
                );
            }
            let outcome_check_selected = matches!(
                program_state.check_kind,
                CheckKind::ScalarTest | CheckKind::HeterogeneousPool
            );
            if outcome_check_selected != (program_state.outcome_branch_count == 1) {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_OUTCOME_BRANCH_COUNT_INVALID",
                    format!("{path}.program"),
                    "a scalar or heterogeneous-pool test requires exactly one on-outcome branch and other checks forbid it",
                );
            }
            if action.targets.kind == RpgIrTargetKind::Cell
                && !is_selected_destination_movement_program(&action.program)
            {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_CELL_PROGRAM_INVALID",
                    format!("{path}.program"),
                    "a cell-target action requires an unconditional no-roll branch containing only one moveToCell operation",
                );
            }
            if !matches!(action.program, RpgIrProgram::Atomic { .. }) {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_ATOMIC_ROOT_REQUIRED",
                    format!("{path}.program"),
                    "an action program must have one atomic root",
                );
            }
        }
    }

    fn validate_check(&mut self, action: &RpgIrAction, path: &str) {
        match &action.check {
            RpgIrCheck::NoRoll => {
                if action.roll_scope != RpgIrRollScope::None {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_ROLL_SCOPE_INVALID",
                        format!("{path}.rollScope"),
                        "a no-roll check requires roll scope none",
                    );
                }
            }
            RpgIrCheck::Attack {
                modifier,
                defense_id,
                ..
            }
            | RpgIrCheck::SavingThrow {
                difficulty: modifier,
                defense_id,
            } => {
                if action.roll_scope == RpgIrRollScope::None {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_ROLL_SCOPE_INVALID",
                        format!("{path}.rollScope"),
                        "a rolled check requires shared or per-target scope",
                    );
                }
                self.require_reference(
                    CatalogKind::Defense,
                    defense_id,
                    &format!("{path}.check.defenseId"),
                    "defense",
                );
                self.require_capability("capability.defenses", &format!("{path}.check"));
                self.require_capability("capability.random", &format!("{path}.check"));
                self.validate_formula(modifier, &format!("{path}.check.formula"));
            }
            RpgIrCheck::ScalarTest {
                profile,
                base,
                difficulty,
            } => {
                if action.roll_scope == RpgIrRollScope::None {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_ROLL_SCOPE_INVALID",
                        format!("{path}.rollScope"),
                        "a scalar test requires shared or per-target roll scope",
                    );
                }
                if profile.ruleset_id.trim().is_empty() || profile.id.trim().is_empty() {
                    self.error(
                        RpgDiagnosticStage::References,
                        "RPG_IR_SCALAR_TEST_PROFILE_INVALID",
                        format!("{path}.check.profile"),
                        "a scalar test requires an owned Ruleset profile reference",
                    );
                }
                self.require_capability("capability.random", &format!("{path}.check"));
                self.validate_scalar_expression(base, &format!("{path}.check.base"));
                match difficulty {
                    RpgIrScalarTestDifficulty::Explicit { value } => {
                        self.validate_scalar_expression(
                            value,
                            &format!("{path}.check.difficulty.value"),
                        );
                    }
                    RpgIrScalarTestDifficulty::TargetDefense { defense_id } => {
                        self.require_reference(
                            CatalogKind::Defense,
                            defense_id,
                            &format!("{path}.check.difficulty.defenseId"),
                            "defense",
                        );
                        self.require_capability(
                            "capability.defenses",
                            &format!("{path}.check.difficulty"),
                        );
                    }
                }
            }
            RpgIrCheck::HeterogeneousPool {
                profile,
                base_dice,
                automatic_axes,
            } => {
                if action.roll_scope == RpgIrRollScope::None {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_ROLL_SCOPE_INVALID",
                        format!("{path}.rollScope"),
                        "a heterogeneous pool requires shared or per-target roll scope",
                    );
                }
                if profile.ruleset_id.trim().is_empty() || profile.id.trim().is_empty() {
                    self.error(
                        RpgDiagnosticStage::References,
                        "RPG_IR_HETEROGENEOUS_POOL_PROFILE_INVALID",
                        format!("{path}.check.profile"),
                        "a heterogeneous pool requires an owned Ruleset profile reference",
                    );
                }
                if base_dice.is_empty() || base_dice.len() > 64 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_HETEROGENEOUS_POOL_TERMS_INVALID",
                        format!("{path}.check.baseDice"),
                        "a heterogeneous pool requires 1..=64 base die terms",
                    );
                }
                let mut previous_die = None::<&str>;
                let mut total_dice = 0_u32;
                for (index, term) in base_dice.iter().enumerate() {
                    if term.count == 0
                        || previous_die
                            .is_some_and(|previous| previous >= term.die_type_id.as_str())
                    {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_HETEROGENEOUS_POOL_TERM_INVALID",
                            format!("{path}.check.baseDice[{index}]"),
                            "base die terms require unique sorted ids and positive counts",
                        );
                    }
                    total_dice = total_dice.saturating_add(term.count);
                    previous_die = Some(term.die_type_id.as_str());
                }
                if total_dice > 256 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_HETEROGENEOUS_POOL_DICE_LIMIT_EXCEEDED",
                        format!("{path}.check.baseDice"),
                        "a heterogeneous pool may contain at most 256 base dice",
                    );
                }
                let mut previous_axis = None::<&str>;
                for (index, axis) in automatic_axes.iter().enumerate() {
                    if previous_axis.is_some_and(|previous| previous >= axis.axis_id.as_str()) {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_HETEROGENEOUS_POOL_AXIS_INVALID",
                            format!("{path}.check.automaticAxes[{index}]"),
                            "automatic axis entries require unique sorted ids",
                        );
                    }
                    previous_axis = Some(axis.axis_id.as_str());
                }
                self.require_capability("capability.random", &format!("{path}.check"));
            }
        }
    }

    fn validate_program(
        &mut self,
        program: &RpgIrProgram,
        path: &str,
        depth: usize,
        execution_multiplier: u64,
        target_bound: bool,
        state: &mut ProgramValidationState,
    ) {
        state.node_count = state.node_count.saturating_add(1);
        state.expanded_node_count = state
            .expanded_node_count
            .saturating_add(execution_multiplier);
        if depth > MAX_PROGRAM_DEPTH {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_PROGRAM_DEPTH_EXCEEDED",
                path,
                format!("program depth exceeds {MAX_PROGRAM_DEPTH}"),
            );
            return;
        }
        if state.node_count > MAX_PROGRAM_NODES {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_PROGRAM_SIZE_EXCEEDED",
                path,
                format!("program node count exceeds {MAX_PROGRAM_NODES}"),
            );
            return;
        }
        if state.expanded_node_count > MAX_EXPANDED_PROGRAM_NODES {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_PROGRAM_EXPANSION_EXCEEDED",
                path,
                format!("bounded program expansion exceeds {MAX_EXPANDED_PROGRAM_NODES} nodes"),
            );
            return;
        }

        match program {
            RpgIrProgram::Operation { operation } => {
                self.validate_operation(operation, path, target_bound, state);
            }
            RpgIrProgram::Sequence { steps } => {
                if steps.is_empty() {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_EMPTY_SEQUENCE",
                        path,
                        "a sequence must contain at least one step",
                    );
                }
                for (index, step) in steps.iter().enumerate() {
                    self.validate_program(
                        step,
                        &format!("{path}.steps[{index}]"),
                        depth + 1,
                        execution_multiplier,
                        target_bound,
                        state,
                    );
                }
            }
            RpgIrProgram::When {
                predicate,
                then,
                otherwise,
            } => {
                self.validate_predicate(
                    predicate,
                    &format!("{path}.predicate"),
                    target_bound || state.action_target_maximum == 1,
                );
                self.validate_program(
                    then,
                    &format!("{path}.then"),
                    depth + 1,
                    execution_multiplier,
                    target_bound,
                    state,
                );
                if let Some(otherwise) = otherwise {
                    self.validate_program(
                        otherwise,
                        &format!("{path}.otherwise"),
                        depth + 1,
                        execution_multiplier,
                        target_bound,
                        state,
                    );
                }
            }
            RpgIrProgram::Repeat { count, body } => {
                if *count == 0 || *count > MAX_REPEAT_COUNT {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_REPEAT_BOUND_INVALID",
                        format!("{path}.count"),
                        format!("repeat count must be between 1 and {MAX_REPEAT_COUNT}"),
                    );
                }
                self.validate_program(
                    body,
                    &format!("{path}.body"),
                    depth + 1,
                    execution_multiplier.saturating_mul(u64::from(*count)),
                    target_bound,
                    state,
                );
            }
            RpgIrProgram::ForEachTarget { maximum, body } => {
                if *maximum == 0
                    || *maximum > MAX_TARGET_COUNT
                    || *maximum > state.action_target_maximum
                {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_FOR_EACH_BOUND_INVALID",
                        format!("{path}.maximum"),
                        "for-each bound must be positive and no larger than the selector bound",
                    );
                }
                self.validate_program(
                    body,
                    &format!("{path}.body"),
                    depth + 1,
                    execution_multiplier.saturating_mul(u64::from(*maximum)),
                    true,
                    state,
                );
            }
            RpgIrProgram::OnCheck {
                hit,
                miss,
                saved,
                failed,
                no_roll,
            } => {
                if state.action_target_maximum > 1 && !target_bound {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_CHECK_TARGET_BINDING_REQUIRED",
                        path,
                        "a multi-target check branch must be inside for-each-target",
                    );
                }
                let has_incompatible_branch = match state.check_kind {
                    CheckKind::NoRoll => {
                        hit.is_some() || miss.is_some() || saved.is_some() || failed.is_some()
                    }
                    CheckKind::Attack => saved.is_some() || failed.is_some() || no_roll.is_some(),
                    CheckKind::SavingThrow => hit.is_some() || miss.is_some() || no_roll.is_some(),
                    CheckKind::ScalarTest | CheckKind::HeterogeneousPool => true,
                };
                if has_incompatible_branch {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_CHECK_BRANCH_INCOMPATIBLE",
                        path,
                        "on-check declares an outcome unavailable to the selected check",
                    );
                }
                let branches = [hit, miss, saved, failed, no_roll];
                if branches.iter().all(|branch| branch.is_none()) {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_EMPTY_CHECK_BRANCH",
                        path,
                        "on-check must declare at least one branch",
                    );
                }
                for (index, branch) in branches.into_iter().enumerate() {
                    if let Some(branch) = branch {
                        self.validate_program(
                            branch,
                            &format!("{path}.branches[{index}]"),
                            depth + 1,
                            execution_multiplier,
                            target_bound,
                            state,
                        );
                    }
                }
            }
            RpgIrProgram::OnOutcome { branches, default } => {
                state.outcome_branch_count = state.outcome_branch_count.saturating_add(1);
                if !matches!(
                    state.check_kind,
                    CheckKind::ScalarTest | CheckKind::HeterogeneousPool
                ) {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_OUTCOME_BRANCH_INCOMPATIBLE",
                        path,
                        "on-outcome is available only to scalar or heterogeneous-pool actions",
                    );
                }
                if state.action_target_maximum > 1 && !target_bound {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_CHECK_TARGET_BINDING_REQUIRED",
                        path,
                        "a multi-target outcome branch must be inside for-each-target",
                    );
                }
                if branches.is_empty() {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_OUTCOME_BRANCH_EMPTY",
                        format!("{path}.branches"),
                        "on-outcome requires at least one explicit outcome-band branch",
                    );
                }
                for (band_id, branch) in branches {
                    if band_id.trim().is_empty() {
                        self.error(
                            RpgDiagnosticStage::References,
                            "RPG_IR_OUTCOME_BAND_ID_INVALID",
                            format!("{path}.branches"),
                            "outcome-band branch identities cannot be empty",
                        );
                    }
                    self.validate_program(
                        branch,
                        &format!("{path}.branches.{band_id}"),
                        depth + 1,
                        execution_multiplier,
                        target_bound,
                        state,
                    );
                }
                self.validate_program(
                    default,
                    &format!("{path}.default"),
                    depth + 1,
                    execution_multiplier,
                    target_bound,
                    state,
                );
            }
            RpgIrProgram::Atomic { body } => {
                if depth != 1 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_NESTED_ATOMIC_FORBIDDEN",
                        path,
                        "atomic is allowed only at the action root",
                    );
                }
                self.validate_program(
                    body,
                    &format!("{path}.body"),
                    depth + 1,
                    execution_multiplier,
                    target_bound,
                    state,
                );
            }
        }
    }

    fn validate_operation(
        &mut self,
        operation: &RpgIrOperation,
        path: &str,
        target_bound: bool,
        state: &mut ProgramValidationState,
    ) {
        let action_target_maximum = state.action_target_maximum;
        let has_target_binding = target_bound || action_target_maximum == 1;
        let id = operation.registration_id();
        if !self.operation_ids.contains(id) {
            self.error(
                RpgDiagnosticStage::Requirements,
                "RPG_IR_OPERATION_REQUIREMENT_MISSING",
                path,
                format!("operation {id} is used without an exact requirement"),
            );
        }
        if let Some(registration) = operation_registration(id) {
            self.require_capability(registration.mutation_owner.as_str(), path);
            for capability in registration.reads {
                self.require_capability(capability.as_str(), path);
            }
        }
        match operation {
            RpgIrOperation::Damage { parts } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                if parts.is_empty() || parts.len() > MAXIMUM_RPG_DAMAGE_PARTS {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_DAMAGE_PARTS_INVALID",
                        format!("{path}.parts"),
                        format!("damage packets require 1..={MAXIMUM_RPG_DAMAGE_PARTS} parts"),
                    );
                }
                let mut previous_id = None::<&str>;
                for (index, part) in parts.iter().enumerate() {
                    let part_path = format!("{path}.parts[{index}]");
                    if !is_portable_identifier(&part.id)
                        || previous_id.is_some_and(|previous| previous >= part.id.as_str())
                    {
                        self.error(
                            RpgDiagnosticStage::Artifact,
                            "RPG_IR_DAMAGE_PARTS_NOT_CANONICAL",
                            format!("{part_path}.id"),
                            "damage part identities must be unique sorted portable identifiers",
                        );
                    }
                    previous_id = Some(&part.id);
                    self.require_identifier(&part.damage_type, &format!("{part_path}.damageType"));
                    if part.tags.len() > MAXIMUM_RPG_DAMAGE_TAGS {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_DAMAGE_TAG_LIMIT_EXCEEDED",
                            format!("{part_path}.tags"),
                            format!(
                                "one damage part may declare at most {MAXIMUM_RPG_DAMAGE_TAGS} tags"
                            ),
                        );
                    }
                    let mut previous_tag = None::<&str>;
                    for (tag_index, tag) in part.tags.iter().enumerate() {
                        if !is_portable_identifier(tag)
                            || previous_tag.is_some_and(|previous| previous >= tag.as_str())
                        {
                            self.error(
                                RpgDiagnosticStage::Artifact,
                                "RPG_IR_DAMAGE_TAGS_NOT_CANONICAL",
                                format!("{part_path}.tags[{tag_index}]"),
                                "damage tags must be unique sorted portable identifiers",
                            );
                        }
                        previous_tag = Some(tag);
                    }
                    self.validate_formula_at(
                        &part.amount,
                        &format!("{part_path}.amount"),
                        has_target_binding,
                    );
                }
            }
            RpgIrOperation::Heal { amount } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.validate_formula_at(amount, &format!("{path}.amount"), has_target_binding);
            }
            RpgIrOperation::ChangeResource {
                subject,
                resource_id,
                delta,
            } => {
                if *subject == RpgIrSubject::Target {
                    self.require_target_binding(path, target_bound, action_target_maximum);
                }
                self.require_reference(
                    CatalogKind::Resource,
                    resource_id,
                    &format!("{path}.resourceId"),
                    "resource",
                );
                self.validate_formula_at(delta, &format!("{path}.delta"), has_target_binding);
            }
            RpgIrOperation::ApplyModifier {
                modifier_id,
                stacking_group,
                stacking: _,
                value,
                duration_turns,
            } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.require_reference(
                    CatalogKind::Modifier,
                    modifier_id,
                    &format!("{path}.modifierId"),
                    "modifier",
                );
                self.require_identifier(stacking_group, &format!("{path}.stackingGroup"));
                if !(1..=MAXIMUM_RPG_MODIFIER_TURNS).contains(duration_turns) {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_DURATION_INVALID",
                        format!("{path}.durationTurns"),
                        format!(
                            "modifier duration must be between 1 and {MAXIMUM_RPG_MODIFIER_TURNS} turns"
                        ),
                    );
                }
                self.validate_formula_at(value, &format!("{path}.value"), has_target_binding);
            }
            RpgIrOperation::ApplyEffect {
                effect_definition_id,
                rank,
            } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.require_identifier(
                    effect_definition_id,
                    &format!("{path}.effectDefinitionId"),
                );
                self.validate_formula_at(rank, &format!("{path}.rank"), has_target_binding);
            }
            RpgIrOperation::RemoveEffect {
                effect_definition_id,
            } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.require_identifier(
                    effect_definition_id,
                    &format!("{path}.effectDefinitionId"),
                );
            }
            RpgIrOperation::Move {
                subject,
                delta_x,
                delta_y,
                maximum_distance,
                provokes: _,
            } => {
                if *subject == RpgIrSubject::Target {
                    self.require_target_binding(path, target_bound, action_target_maximum);
                }
                if *maximum_distance == 0 || *maximum_distance > 64 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_MOVEMENT_BOUND_INVALID",
                        format!("{path}.maximumDistance"),
                        "movement maximum distance must be between 1 and 64",
                    );
                }
                self.validate_formula_at(delta_x, &format!("{path}.deltaX"), has_target_binding);
                self.validate_formula_at(delta_y, &format!("{path}.deltaY"), has_target_binding);
            }
            RpgIrOperation::MoveToCell {
                maximum_distance,
                provokes: _,
            } => {
                if state.action_target_kind != RpgIrTargetKind::Cell {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_MOVE_TO_CELL_TARGET_INVALID",
                        path,
                        "moveToCell requires a cell-target action",
                    );
                }
                if *maximum_distance == 0 || *maximum_distance > 64 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_MOVEMENT_BOUND_INVALID",
                        format!("{path}.maximumDistance"),
                        "movement maximum distance must be between 1 and 64",
                    );
                }
            }
            RpgIrOperation::Push { subject, distance } => {
                if *subject != RpgIrSubject::Target {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_FORCED_MOVEMENT_SUBJECT_INVALID",
                        format!("{path}.subject"),
                        "push currently requires the selected participant target",
                    );
                }
                self.require_target_binding(path, target_bound, action_target_maximum);
                if *distance == 0 || *distance > 64 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_FORCED_MOVEMENT_BOUND_INVALID",
                        format!("{path}.distance"),
                        "push distance must be between 1 and 64",
                    );
                }
            }
            RpgIrOperation::Slide {
                subject,
                maximum_distance,
            } => {
                if *subject != RpgIrSubject::Target {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_FORCED_MOVEMENT_SUBJECT_INVALID",
                        format!("{path}.subject"),
                        "slide currently requires the selected participant target",
                    );
                }
                self.require_target_binding(path, target_bound, action_target_maximum);
                if *maximum_distance == 0 || *maximum_distance > 64 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_FORCED_MOVEMENT_BOUND_INVALID",
                        format!("{path}.maximumDistance"),
                        "slide maximum distance must be between 1 and 64",
                    );
                }
            }
            RpgIrOperation::CreateSpatialSource {
                spatial_source_definition_id,
                instance_id,
                owner,
                source,
            } => {
                self.require_capability(RpgCapabilityId::SpatialSources.as_str(), path);
                self.require_identifier(
                    spatial_source_definition_id,
                    &format!("{path}.spatialSourceDefinitionId"),
                );
                self.require_identifier(instance_id, &format!("{path}.instanceId"));
                if *owner == RpgIrSubject::Target || *source == RpgIrSubject::Target {
                    self.require_target_binding(path, target_bound, action_target_maximum);
                }
            }
            RpgIrOperation::OpenReaction {
                reaction_id,
                options,
            } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.require_identifier(reaction_id, &format!("{path}.reactionId"));
                if options.is_empty() || options.len() > 16 {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_REACTION_OPTIONS_INVALID",
                        format!("{path}.options"),
                        "a reaction must declare between 1 and 16 options",
                    );
                }
                let mut option_ids = BTreeSet::new();
                for (index, option) in options.iter().enumerate() {
                    let option_path = format!("{path}.options[{index}]");
                    self.require_identifier(&option.id, &format!("{option_path}.id"));
                    if !option_ids.insert(&option.id) {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_REACTION_OPTION_DUPLICATE",
                            format!("{option_path}.id"),
                            "reaction option ids must be unique",
                        );
                    }
                    if option.label.trim().is_empty() {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_REACTION_OPTION_LABEL_EMPTY",
                            format!("{option_path}.label"),
                            "reaction option label must not be empty",
                        );
                    }
                    if option.damage_reduction > 10_000 {
                        self.error(
                            RpgDiagnosticStage::Semantics,
                            "RPG_IR_REACTION_REDUCTION_INVALID",
                            format!("{option_path}.damageReduction"),
                            "reaction damage reduction exceeds the supported bound",
                        );
                    }
                    if let Some(activation) = &option.activation {
                        self.validate_activation(
                            activation,
                            RpgIrActivationTiming::Reaction,
                            &format!("{option_path}.activation"),
                        );
                    }
                }
            }
        }
    }

    fn validate_activation(
        &mut self,
        activation: &RpgIrActivation,
        expected_timing: RpgIrActivationTiming,
        path: &str,
    ) {
        self.require_capability(RpgCapabilityId::ActivationBudgets.as_str(), path);
        if activation.timing != expected_timing {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_ACTIVATION_TIMING_INVALID",
                format!("{path}.timing"),
                "activation timing does not match its action or reaction position",
            );
        }

        let mut previous = None::<(&str, &str)>;
        for (index, cost) in activation.costs.iter().enumerate() {
            let cost_path = format!("{path}.costs[{index}]");
            self.require_identifier(
                &cost.budget.ruleset_id,
                &format!("{cost_path}.budget.rulesetId"),
            );
            self.require_identifier(&cost.budget.id, &format!("{cost_path}.budget.id"));
            let current = (cost.budget.ruleset_id.as_str(), cost.budget.id.as_str());
            if previous.is_some_and(|prior| prior >= current) {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_ACTIVATION_COSTS_NOT_CANONICAL",
                    cost_path.clone(),
                    "activation costs must be unique and sorted by ruleset and budget id",
                );
            }
            if cost.amount < 0 {
                self.error(
                    RpgDiagnosticStage::Semantics,
                    "RPG_IR_ACTIVATION_COST_INVALID",
                    format!("{cost_path}.amount"),
                    "activation budget costs must be nonnegative",
                );
            }
            previous = Some(current);
        }
    }

    fn require_target_binding(&mut self, path: &str, target_bound: bool, maximum: u32) {
        if maximum > 1 && !target_bound {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_TARGET_BINDING_REQUIRED",
                path,
                "target-mutating operations for a multi-target action must be inside for-each-target",
            );
        }
    }

    fn validate_predicate(&mut self, predicate: &RpgIrPredicate, path: &str, target_bound: bool) {
        let mut node_count = 0;
        self.validate_predicate_node(predicate, path, target_bound, 1, &mut node_count);
    }

    fn validate_predicate_node(
        &mut self,
        predicate: &RpgIrPredicate,
        path: &str,
        target_bound: bool,
        depth: usize,
        node_count: &mut usize,
    ) {
        *node_count = node_count.saturating_add(1);
        if depth > MAX_EXPRESSION_DEPTH || *node_count > MAX_EXPRESSION_NODES {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_PREDICATE_BOUND_EXCEEDED",
                path,
                "predicate depth or node count exceeds the supported bound",
            );
            return;
        }
        match predicate {
            RpgIrPredicate::Always => {}
            RpgIrPredicate::Compare { left, right, .. } => {
                self.validate_formula_at(left, &format!("{path}.left"), target_bound);
                self.validate_formula_at(right, &format!("{path}.right"), target_bound);
            }
            RpgIrPredicate::Not { predicate } => {
                self.validate_predicate_node(
                    predicate,
                    &format!("{path}.predicate"),
                    target_bound,
                    depth + 1,
                    node_count,
                );
            }
            RpgIrPredicate::All { predicates } | RpgIrPredicate::Any { predicates } => {
                if predicates.is_empty() {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_EMPTY_PREDICATE_SET",
                        path,
                        "predicate sets must not be empty",
                    );
                }
                for (index, predicate) in predicates.iter().enumerate() {
                    self.validate_predicate_node(
                        predicate,
                        &format!("{path}.predicates[{index}]"),
                        target_bound,
                        depth + 1,
                        node_count,
                    );
                }
            }
        }
    }

    fn validate_formula(&mut self, formula: &RpgIrFormula, path: &str) {
        self.validate_formula_with_policy(formula, path, true, true);
    }

    fn validate_formula_at(&mut self, formula: &RpgIrFormula, path: &str, target_bound: bool) {
        self.validate_formula_with_policy(formula, path, target_bound, true);
    }

    fn validate_scalar_expression(&mut self, formula: &RpgIrFormula, path: &str) {
        self.validate_formula_with_policy(formula, path, true, false);
    }

    fn validate_formula_with_policy(
        &mut self,
        formula: &RpgIrFormula,
        path: &str,
        target_bound: bool,
        allow_random: bool,
    ) {
        let mut node_count = 0;
        self.validate_formula_node(
            formula,
            path,
            target_bound,
            allow_random,
            1,
            &mut node_count,
        );
    }

    fn validate_formula_node(
        &mut self,
        formula: &RpgIrFormula,
        path: &str,
        target_bound: bool,
        allow_random: bool,
        depth: usize,
        node_count: &mut usize,
    ) {
        *node_count = node_count.saturating_add(1);
        if depth > MAX_EXPRESSION_DEPTH || *node_count > MAX_EXPRESSION_NODES {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_FORMULA_BOUND_EXCEEDED",
                path,
                "formula depth or node count exceeds the supported bound",
            );
            return;
        }
        match formula {
            RpgIrFormula::Constant { .. } => {}
            RpgIrFormula::ReadStat { subject, stat_id } => {
                if *subject == RpgIrSubject::Target && !target_bound {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_TARGET_BINDING_REQUIRED",
                        path,
                        "target stat read requires target binding",
                    );
                }
                self.require_reference(CatalogKind::Stat, stat_id, path, "stat");
                self.require_capability("capability.stats", path);
            }
            RpgIrFormula::Add { terms } => {
                if terms.is_empty() {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_EMPTY_FORMULA",
                        path,
                        "add requires at least one term",
                    );
                }
                for (index, term) in terms.iter().enumerate() {
                    self.validate_formula_node(
                        term,
                        &format!("{path}.terms[{index}]"),
                        target_bound,
                        allow_random,
                        depth + 1,
                        node_count,
                    );
                }
            }
            RpgIrFormula::Dice { count, sides, .. } => {
                if !allow_random {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_SCALAR_TEST_RANDOM_FORMULA_INVALID",
                        path,
                        "scalar-test base and explicit difficulty expressions cannot contain dice",
                    );
                }
                if *count == 0 || *count > MAX_DICE_COUNT || *sides < 2 || *sides > MAX_DICE_SIDES {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_DICE_BOUND_INVALID",
                        path,
                        "dice count or side count is outside the supported bounds",
                    );
                }
                if allow_random {
                    self.require_capability("capability.random", path);
                }
            }
            RpgIrFormula::Half { value } => {
                self.validate_formula_node(
                    value,
                    &format!("{path}.value"),
                    target_bound,
                    allow_random,
                    depth + 1,
                    node_count,
                );
            }
        }
    }

    fn require_capability(&mut self, id: &str, path: &str) {
        if !self.capability_ids.contains(id)
            || !self.requirements.contains_key(&(
                requirement_kind_key(RpgIrRequirementKind::Capability),
                id.to_owned(),
            ))
        {
            self.error(
                RpgDiagnosticStage::Requirements,
                "RPG_IR_CAPABILITY_REQUIREMENT_MISSING",
                path,
                format!("semantic use requires cataloged exact capability {id}"),
            );
        }
    }

    fn require_reference(&mut self, catalog: CatalogKind, id: &str, path: &str, kind: &str) {
        let exists = match catalog {
            CatalogKind::Stat => self.stats.contains(id),
            CatalogKind::Defense => self.defenses.contains(id),
            CatalogKind::Resource => self.resources.contains(id),
            CatalogKind::Modifier => self.modifiers.contains(id),
        };
        if !exists {
            self.error(
                RpgDiagnosticStage::References,
                "RPG_IR_REFERENCE_UNKNOWN",
                path,
                format!("unknown {kind} reference {id}"),
            );
        }
    }

    fn require_identifier(&mut self, value: &str, path: &str) {
        if value.is_empty()
            || !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "._-/".contains(character))
        {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_IDENTIFIER_INVALID",
                path,
                "identifier must use ASCII letters, digits, dot, underscore, dash, or slash",
            );
        }
    }

    fn require_nonempty(&mut self, value: &str, path: &str, field: &str) {
        if value.trim().is_empty() {
            self.error(
                RpgDiagnosticStage::Semantics,
                "RPG_IR_VALUE_EMPTY",
                path,
                format!("{field} must not be empty"),
            );
        }
    }

    fn error(
        &mut self,
        stage: RpgDiagnosticStage,
        code: &str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics
            .push(RpgDiagnostic::error(stage, code, path, message));
    }
}

fn requirement_kind_key(kind: RpgIrRequirementKind) -> u8 {
    match kind {
        RpgIrRequirementKind::Operation => 0,
        RpgIrRequirementKind::Capability => 1,
    }
}

fn area_program_covers_target_maximum(program: &RpgIrProgram, maximum: u32) -> bool {
    fn collect(program: &RpgIrProgram, maxima: &mut Vec<u32>) {
        match program {
            RpgIrProgram::Operation { .. } => {}
            RpgIrProgram::Sequence { steps } => {
                for step in steps {
                    collect(step, maxima);
                }
            }
            RpgIrProgram::When {
                then, otherwise, ..
            } => {
                collect(then, maxima);
                if let Some(otherwise) = otherwise {
                    collect(otherwise, maxima);
                }
            }
            RpgIrProgram::Repeat { body, .. } | RpgIrProgram::Atomic { body } => {
                collect(body, maxima);
            }
            RpgIrProgram::ForEachTarget {
                maximum: bound,
                body,
            } => {
                maxima.push(*bound);
                collect(body, maxima);
            }
            RpgIrProgram::OnCheck {
                hit,
                miss,
                saved,
                failed,
                no_roll,
            } => {
                for branch in [hit, miss, saved, failed, no_roll].into_iter().flatten() {
                    collect(branch, maxima);
                }
            }
            RpgIrProgram::OnOutcome {
                branches, default, ..
            } => {
                for branch in branches.values() {
                    collect(branch, maxima);
                }
                collect(default, maxima);
            }
        }
    }

    let mut maxima = Vec::new();
    collect(program, &mut maxima);
    !maxima.is_empty() && maxima.into_iter().all(|bound| bound == maximum)
}

fn is_portable_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-/".contains(character))
}
