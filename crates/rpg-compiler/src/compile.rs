use std::collections::{BTreeMap, BTreeSet};

use rpg_core::{RpgRandomRequest, RpgRandomRequestKind, MAXIMUM_RPG_MODIFIER_TURNS};
use rpg_ir::{
    EquippedItemBindingRequirement, NormalizedRpgIr, RpgIrAction, RpgIrCheck, RpgIrFormula,
    RpgIrOperation, RpgIrPredicate, RpgIrProgram, RpgIrRequirementKind, RpgIrResourceCost,
    RpgIrRollScope, RpgIrSubject, RpgIrTargetKind, RpgIrTargetSelector, RpgIrTeamConstraint,
    RPG_IR_IDENTITY, RPG_IR_MAJOR,
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
}

#[derive(Debug, Clone)]
pub struct CompiledRpgRules {
    package_id: String,
    package_version: String,
    capability_plan: BTreeMap<String, u32>,
    actions: BTreeMap<String, CompiledAction>,
    bound_actions: BTreeMap<(String, String), CompiledAction>,
    binding_requirements: BTreeMap<String, EquippedItemBindingRequirement>,
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
    pub targets: RpgIrTargetSelector,
    pub check: RpgIrCheck,
    pub roll_scope: RpgIrRollScope,
    pub costs: Vec<RpgIrResourceCost>,
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
        targets: action.targets.clone(),
        check: action.check.clone(),
        roll_scope: action.roll_scope,
        costs: action.costs.clone(),
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
    pub(crate) targets: RpgIrTargetSelector,
    pub(crate) check: RpgIrCheck,
    pub(crate) roll_scope: RpgIrRollScope,
    pub(crate) costs: Vec<RpgIrResourceCost>,
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
            .map(|action| (action.id.clone(), compile_action(action)))
            .collect(),
        bound_actions: BTreeMap::new(),
        binding_requirements: BTreeMap::new(),
    })
}

fn compile_action(action: RpgIrAction) -> CompiledAction {
    let random_plan = collect_action_random_plan(&action);
    CompiledAction {
        name: action.name,
        source_path: action.source_path,
        targets: action.targets,
        check: action.check,
        roll_scope: action.roll_scope,
        costs: action.costs,
        program: compile_program(action.program),
        random_plan,
    }
}

fn collect_action_random_plan(action: &RpgIrAction) -> Vec<RpgRandomPlanEntry> {
    let mut plan = Vec::new();
    if !matches!(action.check, RpgIrCheck::NoRoll) {
        let kind = match action.check {
            RpgIrCheck::Attack { .. } => RpgRandomRequestKind::AttackCheck,
            RpgIrCheck::SavingThrow { .. } => RpgRandomRequestKind::SavingThrowCheck,
            RpgIrCheck::NoRoll => unreachable!(),
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
                sides: 20,
                path: "$.action.check".to_owned(),
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
            RpgIrOperation::Damage { amount, .. } | RpgIrOperation::Heal { amount } => {
                collect_formula_random_plan(amount, &format!("{path}.amount"), conditions, plan);
            }
            RpgIrOperation::ChangeResource { delta, .. } => {
                collect_formula_random_plan(delta, &format!("{path}.delta"), conditions, plan);
            }
            RpgIrOperation::ApplyModifier { value, .. } => {
                collect_formula_random_plan(value, &format!("{path}.value"), conditions, plan);
            }
            RpgIrOperation::Move {
                delta_x, delta_y, ..
            } => {
                collect_formula_random_plan(delta_x, &format!("{path}.deltaX"), conditions, plan);
                collect_formula_random_plan(delta_y, &format!("{path}.deltaY"), conditions, plan);
            }
            RpgIrOperation::MoveToCell { .. } => {}
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
            self.validate_check(action, &path);
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
                },
            };
            self.validate_program(
                &action.program,
                &format!("{path}.program"),
                1,
                1,
                false,
                &mut program_state,
            );
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
            RpgIrOperation::Damage {
                amount,
                damage_type,
            } => {
                self.require_target_binding(path, target_bound, action_target_maximum);
                self.require_identifier(damage_type, &format!("{path}.damageType"));
                self.validate_formula_at(amount, &format!("{path}.amount"), has_target_binding);
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
                }
            }
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
        self.validate_formula_at(formula, path, true);
    }

    fn validate_formula_at(&mut self, formula: &RpgIrFormula, path: &str, target_bound: bool) {
        let mut node_count = 0;
        self.validate_formula_node(formula, path, target_bound, 1, &mut node_count);
    }

    fn validate_formula_node(
        &mut self,
        formula: &RpgIrFormula,
        path: &str,
        target_bound: bool,
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
                        depth + 1,
                        node_count,
                    );
                }
            }
            RpgIrFormula::Dice { count, sides, .. } => {
                if *count == 0 || *count > MAX_DICE_COUNT || *sides < 2 || *sides > MAX_DICE_SIDES {
                    self.error(
                        RpgDiagnosticStage::Semantics,
                        "RPG_IR_DICE_BOUND_INVALID",
                        path,
                        "dice count or side count is outside the supported bounds",
                    );
                }
                self.require_capability("capability.random", path);
            }
            RpgIrFormula::Half { value } => {
                self.validate_formula_node(
                    value,
                    &format!("{path}.value"),
                    target_bound,
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
