use std::collections::BTreeMap;

use rpg_core::{
    DeterministicRandomStream, GridPosition, RpgCapabilityId, RpgCapabilityMutationError,
    RpgCapabilityState, RpgCapabilityWorkspace, RpgDomainEvent, RpgIntent,
    RpgModifierStackingPolicy, RpgRandomEvidence, RpgRandomRequest, RpgRandomRequestKind,
    RpgReactionDecision, RpgReactionOption, RpgReactionRequest, RpgResolutionReceipt,
    RpgResolutionRejection, RpgRollContribution, RpgRollContributionCondition,
    RpgRollContributionReason, RpgRollContributionSelector, RpgTraceStep,
};
use rpg_ir::{
    CompiledCharacterFeature, RpgIrCheck, RpgIrComparison, RpgIrFormula, RpgIrOperation,
    RpgIrPredicate, RpgIrRollScope, RpgIrSubject, RpgIrTargetKind, RpgIrTeamConstraint,
};

use crate::compile::{CompiledAction, CompiledOperation, CompiledProgram};
use crate::CompiledRpgRules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckOutcome {
    Hit,
    Miss,
    Saved,
    Failed,
    NoRoll,
}

impl CompiledRpgRules {
    pub fn resolve(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_internal(state, random, intent, None)
    }

    pub fn resolve_with_reaction_decision(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
        reaction: &RpgReactionDecision,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_internal(state, random, intent, Some(reaction))
    }

    fn resolve_internal<'a>(
        &'a self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &'a RpgIntent,
        reaction: Option<&'a RpgReactionDecision>,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        let action = self
            .action_for_binding(
                &intent.action_id,
                intent
                    .item_binding
                    .as_ref()
                    .map(|binding| binding.item_definition_id.as_str()),
            )
            .ok_or_else(|| {
                rejection(
                    "RPG_INTENT_ACTION_UNKNOWN",
                    "$.intent.actionId",
                    format!("unknown action {}", intent.action_id),
                )
            })?;
        let character_feature_ids = state
            .entity(&intent.actor_id)
            .map(|actor| actor.character_feature_ids())
            .unwrap_or_default();
        let character_features = self.resolve_character_features(character_feature_ids)?;
        let target_ids = validate_intent(action, state, intent)?;
        let mut execution = Execution {
            action,
            intent,
            target_ids,
            workspace: RpgCapabilityWorkspace::stage(state, random),
            random_start: random.consumed(),
            outcomes: BTreeMap::new(),
            events: Vec::new(),
            trace: Vec::new(),
            random_evidence: Vec::new(),
            current_target: None,
            reaction,
            reaction_consumed: false,
            pending_damage_reduction: 0,
            character_features,
        };

        execution.spend_costs()?;
        execution.resolve_checks()?;
        execution.execute_program(&action.program, "$.action.program")?;
        if reaction.is_some() && !execution.reaction_consumed {
            return Err(execution.fail(
                "RPG_REACTION_DECISION_UNUSED",
                "$.reaction",
                "the staged transaction did not reach its reaction window",
            ));
        }
        let revision = execution.workspace.advance_revision();
        execution.trace.push(RpgTraceStep {
            path: "$.resolution.commit".to_owned(),
            code: "RPG_RESOLUTION_COMMITTED".to_owned(),
            detail: format!("state revision {revision}"),
        });

        let random_consumed = u64::try_from(
            execution
                .workspace
                .random_consumed()
                .saturating_sub(execution.random_start),
        )
        .unwrap_or(u64::MAX);
        let receipt = RpgResolutionReceipt {
            action_id: intent.action_id.clone(),
            actor_id: intent.actor_id.clone(),
            target_ids: execution.target_ids.clone(),
            item_binding: intent.item_binding.clone(),
            events: execution.events,
            trace: execution.trace,
            random_evidence: execution.random_evidence,
            random_consumed,
            state_revision: revision,
        };
        execution.workspace.commit(state, random);
        Ok(receipt)
    }

    fn resolve_character_features(
        &self,
        character_feature_ids: &[String],
    ) -> Result<Vec<&CompiledCharacterFeature>, RpgResolutionRejection> {
        let mut previous = None::<&str>;
        let mut features = Vec::with_capacity(character_feature_ids.len());
        for (index, feature_id) in character_feature_ids.iter().enumerate() {
            if previous.is_some_and(|previous| previous >= feature_id.as_str()) {
                return Err(rejection(
                    "RPG_RESOLUTION_FEATURE_SELECTION_NOT_CANONICAL",
                    format!("$.characterFeatureIds[{index}]"),
                    "selected character feature identities must be unique and sorted",
                ));
            }
            previous = Some(feature_id);
            let feature = self.character_feature(feature_id).ok_or_else(|| {
                rejection(
                    "RPG_RESOLUTION_FEATURE_UNKNOWN",
                    format!("$.characterFeatureIds[{index}]"),
                    format!("character feature {feature_id} is not in the compiled PlayBundle"),
                )
            })?;
            features.push(feature);
        }
        Ok(features)
    }

    pub fn candidate_ids(
        &self,
        state: &RpgCapabilityState,
        actor_id: &str,
        action_id: &str,
    ) -> Result<Vec<String>, RpgResolutionRejection> {
        self.candidate_ids_for_binding(state, actor_id, action_id, None)
    }

    pub fn candidate_ids_for_binding(
        &self,
        state: &RpgCapabilityState,
        actor_id: &str,
        action_id: &str,
        item_definition_id: Option<&str>,
    ) -> Result<Vec<String>, RpgResolutionRejection> {
        let action = self
            .action_for_binding(action_id, item_definition_id)
            .ok_or_else(|| {
                rejection(
                    "RPG_INTENT_ACTION_UNKNOWN",
                    "$.actionId",
                    format!("unknown action {action_id}"),
                )
            })?;
        if action.targets.kind == RpgIrTargetKind::Cell {
            return Err(rejection(
                "RPG_ACTION_BOARD_REQUIRED",
                "$.actionId",
                "cell-target candidates require the encounter board authority",
            ));
        }
        let actor = state.entity(actor_id).ok_or_else(|| {
            rejection(
                "RPG_INTENT_ACTOR_UNKNOWN",
                "$.actorId",
                format!("unknown actor {actor_id}"),
            )
        })?;
        Ok(state
            .entities()
            .filter(|target| {
                let team_allowed = match action.targets.team {
                    RpgIrTeamConstraint::Hostile => target.team() != actor.team(),
                    RpgIrTeamConstraint::Ally => target.team() == actor.team(),
                    RpgIrTeamConstraint::Any => true,
                };
                let distance = actor
                    .position()
                    .x
                    .abs_diff(target.position().x)
                    .saturating_add(actor.position().y.abs_diff(target.position().y));
                team_allowed && distance <= action.targets.maximum_range
            })
            .map(|target| target.id().to_owned())
            .collect())
    }

    pub fn target_kind(&self, action_id: &str) -> Result<RpgIrTargetKind, RpgResolutionRejection> {
        self.target_kind_for_binding(action_id, None)
    }

    pub fn target_kind_for_binding(
        &self,
        action_id: &str,
        item_definition_id: Option<&str>,
    ) -> Result<RpgIrTargetKind, RpgResolutionRejection> {
        self.action_for_binding(action_id, item_definition_id)
            .map(|action| action.targets.kind)
            .ok_or_else(|| {
                rejection(
                    "RPG_INTENT_ACTION_UNKNOWN",
                    "$.actionId",
                    format!("unknown action {action_id}"),
                )
            })
    }

    pub fn preflight(
        &self,
        state: &RpgCapabilityState,
        intent: &RpgIntent,
    ) -> Result<(), RpgResolutionRejection> {
        let action = self
            .action_for_binding(
                &intent.action_id,
                intent
                    .item_binding
                    .as_ref()
                    .map(|binding| binding.item_definition_id.as_str()),
            )
            .ok_or_else(|| {
                rejection(
                    "RPG_INTENT_ACTION_UNKNOWN",
                    "$.intent.actionId",
                    format!("unknown action {}", intent.action_id),
                )
            })?;
        validate_intent(action, state, intent).map(|_| ())
    }
}

fn validate_intent(
    action: &CompiledAction,
    state: &RpgCapabilityState,
    intent: &RpgIntent,
) -> Result<Vec<String>, RpgResolutionRejection> {
    let actor = state.entity(&intent.actor_id).ok_or_else(|| {
        rejection(
            "RPG_INTENT_ACTOR_UNKNOWN",
            "$.intent.actorId",
            format!("unknown actor {}", intent.actor_id),
        )
    })?;
    if intent.target_ids.is_empty() {
        return Err(rejection(
            "RPG_INTENT_TARGETS_EMPTY",
            "$.intent.targetIds",
            "at least one target is required",
        ));
    }
    if intent.target_ids.len() > action.targets.maximum_targets as usize {
        return Err(rejection(
            "RPG_INTENT_TARGET_LIMIT_EXCEEDED",
            "$.intent.targetIds",
            format!(
                "action allows at most {} target(s)",
                action.targets.maximum_targets
            ),
        ));
    }

    let mut target_ids = intent.target_ids.clone();
    target_ids.sort();
    let original_length = target_ids.len();
    target_ids.dedup();
    if target_ids.len() != original_length {
        return Err(rejection(
            "RPG_INTENT_TARGET_DUPLICATE",
            "$.intent.targetIds",
            "target ids must be unique",
        ));
    }

    match action.targets.kind {
        RpgIrTargetKind::Participant => {
            if !intent.cell_targets.is_empty() {
                return Err(rejection(
                    "RPG_INTENT_CELL_BINDING_UNEXPECTED",
                    "$.intent.cellTargets",
                    "participant-target actions cannot include cell bindings",
                ));
            }
            for (index, target_id) in target_ids.iter().enumerate() {
                let target = state.entity(target_id).ok_or_else(|| {
                    rejection(
                        "RPG_INTENT_TARGET_UNKNOWN",
                        format!("$.intent.targetIds[{index}]"),
                        format!("unknown target {target_id}"),
                    )
                })?;
                let team_allowed = match action.targets.team {
                    RpgIrTeamConstraint::Hostile => target.team() != actor.team(),
                    RpgIrTeamConstraint::Ally => target.team() == actor.team(),
                    RpgIrTeamConstraint::Any => true,
                };
                if !team_allowed {
                    return Err(rejection(
                        "RPG_INTENT_TARGET_TEAM_INVALID",
                        format!("$.intent.targetIds[{index}]"),
                        format!("target {target_id} does not satisfy the team selector"),
                    ));
                }
                let distance = actor
                    .position()
                    .x
                    .abs_diff(target.position().x)
                    .saturating_add(actor.position().y.abs_diff(target.position().y));
                if distance > action.targets.maximum_range {
                    return Err(rejection(
                        "RPG_INTENT_TARGET_OUT_OF_RANGE",
                        format!("$.intent.targetIds[{index}]"),
                        format!("target {target_id} is at range {distance}"),
                    ));
                }
            }
        }
        RpgIrTargetKind::Cell => {
            if intent.cell_targets.len() != target_ids.len() {
                return Err(rejection(
                    "RPG_INTENT_CELL_BINDING_MISSING",
                    "$.intent.cellTargets",
                    "every selected cell id requires one authoritative position binding",
                ));
            }
            for (index, target_id) in target_ids.iter().enumerate() {
                let binding = intent
                    .cell_targets
                    .iter()
                    .find(|binding| binding.id == *target_id)
                    .ok_or_else(|| {
                        rejection(
                            "RPG_INTENT_CELL_BINDING_MISSING",
                            format!("$.intent.targetIds[{index}]"),
                            format!("selected cell {target_id} has no position binding"),
                        )
                    })?;
                let distance = actor
                    .position()
                    .x
                    .abs_diff(binding.position.x)
                    .saturating_add(actor.position().y.abs_diff(binding.position.y));
                if distance > action.targets.maximum_range {
                    return Err(rejection(
                        "RPG_INTENT_TARGET_OUT_OF_RANGE",
                        format!("$.intent.targetIds[{index}]"),
                        format!("cell {target_id} is at range {distance}"),
                    ));
                }
            }
        }
    }

    for (index, cost) in action.costs.iter().enumerate() {
        let resource = actor.resource(&cost.resource_id).ok_or_else(|| {
            rejection(
                "RPG_INTENT_RESOURCE_UNKNOWN",
                format!("$.action.costs[{index}].resourceId"),
                format!("actor has no resource {}", cost.resource_id),
            )
        })?;
        if resource.current < cost.amount {
            return Err(rejection(
                "RPG_INTENT_RESOURCE_INSUFFICIENT",
                format!("$.action.costs[{index}]"),
                format!("actor cannot pay {} {}", cost.amount, cost.resource_id),
            ));
        }
    }

    Ok(target_ids)
}

struct Execution<'a> {
    action: &'a CompiledAction,
    intent: &'a RpgIntent,
    target_ids: Vec<String>,
    workspace: RpgCapabilityWorkspace,
    random_start: usize,
    outcomes: BTreeMap<String, CheckOutcome>,
    events: Vec<RpgDomainEvent>,
    trace: Vec<RpgTraceStep>,
    random_evidence: Vec<RpgRandomEvidence>,
    current_target: Option<String>,
    reaction: Option<&'a RpgReactionDecision>,
    reaction_consumed: bool,
    pending_damage_reduction: u32,
    character_features: Vec<&'a CompiledCharacterFeature>,
}

impl Execution<'_> {
    fn spend_costs(&mut self) -> Result<(), RpgResolutionRejection> {
        for (index, cost) in self.action.costs.iter().enumerate() {
            let path = format!("$.action.costs[{index}]");
            let remaining = self
                .workspace
                .resources_owner()
                .spend(&self.intent.actor_id, &cost.resource_id, cost.amount)
                .map_err(|error| self.mutation_rejection(error, &path))?;
            self.events.push(RpgDomainEvent::ResourceSpent {
                entity_id: self.intent.actor_id.clone(),
                resource_id: cost.resource_id.clone(),
                amount: cost.amount,
                remaining,
            });
            self.trace.push(RpgTraceStep {
                path,
                code: "RPG_COST_STAGED".to_owned(),
                detail: format!("{} {} remaining {remaining}", cost.amount, cost.resource_id),
            });
        }
        Ok(())
    }

    fn resolve_checks(&mut self) -> Result<(), RpgResolutionRejection> {
        let shared_roll = if self.action.roll_scope == RpgIrRollScope::Shared
            && !matches!(self.action.check, RpgIrCheck::NoRoll)
        {
            let kind = match self.action.check {
                RpgIrCheck::Attack { .. } => RpgRandomRequestKind::AttackCheck,
                RpgIrCheck::SavingThrow { .. } => RpgRandomRequestKind::SavingThrowCheck,
                RpgIrCheck::NoRoll => unreachable!("no-roll check excluded above"),
            };
            Some(self.take_random(kind, 20, "$.action.check.sharedRoll")?)
        } else {
            None
        };
        let target_ids = self.target_ids.clone();
        for (index, target_id) in target_ids.into_iter().enumerate() {
            self.current_target = Some(target_id.clone());
            let path = format!("$.action.check.targets[{index}]");
            let outcome = match &self.action.check {
                RpgIrCheck::NoRoll => CheckOutcome::NoRoll,
                RpgIrCheck::Attack {
                    modifier,
                    defense_id,
                } => {
                    let roll = match shared_roll {
                        Some(value) => value,
                        None => self.take_random(
                            RpgRandomRequestKind::AttackCheck,
                            20,
                            &format!("{path}.roll"),
                        )?,
                    };
                    let modifier = self.eval_formula(modifier, &format!("{path}.modifier"))?;
                    let mut contributions = vec![RpgRollContribution {
                        source_definition_id: self.intent.action_id.clone(),
                        source_label: self.action.name.clone(),
                        amount: modifier,
                        reason: RpgRollContributionReason::ActionCheckModifier,
                    }];
                    contributions
                        .extend(self.applicable_character_feature_contributions(&target_id));
                    let total = contributions.iter().try_fold(
                        i32::try_from(roll).unwrap_or(i32::MAX),
                        |running, contribution| {
                            running.checked_add(contribution.amount).ok_or_else(|| {
                                self.fail(
                                    "RPG_RUNTIME_ROLL_TOTAL_OVERFLOW",
                                    &format!("{path}.contributions"),
                                    "roll contribution total exceeded the runtime integer domain",
                                )
                            })
                        },
                    )?;
                    let defense = self
                        .workspace
                        .state()
                        .entity(&target_id)
                        .and_then(|target| target.defense(defense_id))
                        .ok_or_else(|| {
                            self.fail(
                                "RPG_RUNTIME_DEFENSE_MISSING",
                                &format!("{path}.defense"),
                                format!("target {target_id} has no defense {defense_id}"),
                            )
                        })?;
                    let hit = total >= defense;
                    self.events.push(RpgDomainEvent::AttackResolved {
                        actor_id: self.intent.actor_id.clone(),
                        target_id: target_id.clone(),
                        roll,
                        total,
                        defense_id: defense_id.clone(),
                        defense,
                        hit,
                        contributions,
                    });
                    if hit {
                        CheckOutcome::Hit
                    } else {
                        CheckOutcome::Miss
                    }
                }
                RpgIrCheck::SavingThrow {
                    difficulty,
                    defense_id,
                } => {
                    let roll = match shared_roll {
                        Some(value) => value,
                        None => self.take_random(
                            RpgRandomRequestKind::SavingThrowCheck,
                            20,
                            &format!("{path}.roll"),
                        )?,
                    };
                    let difficulty =
                        self.eval_formula(difficulty, &format!("{path}.difficulty"))?;
                    let defense = self
                        .workspace
                        .state()
                        .entity(&target_id)
                        .and_then(|target| target.defense(defense_id))
                        .ok_or_else(|| {
                            self.fail(
                                "RPG_RUNTIME_DEFENSE_MISSING",
                                &format!("{path}.defense"),
                                format!("target {target_id} has no defense {defense_id}"),
                            )
                        })?;
                    let total = i32::try_from(roll)
                        .unwrap_or(i32::MAX)
                        .saturating_add(defense);
                    let saved = total >= difficulty;
                    self.events.push(RpgDomainEvent::SavingThrowResolved {
                        target_id: target_id.clone(),
                        roll,
                        total,
                        difficulty,
                        saved,
                    });
                    if saved {
                        CheckOutcome::Saved
                    } else {
                        CheckOutcome::Failed
                    }
                }
            };
            self.outcomes.insert(target_id.clone(), outcome);
            self.trace.push(RpgTraceStep {
                path,
                code: "RPG_CHECK_RESOLVED".to_owned(),
                detail: format!("target {target_id} outcome {outcome:?}"),
            });
        }
        self.current_target = None;
        Ok(())
    }

    fn applicable_character_feature_contributions(
        &self,
        target_id: &str,
    ) -> Vec<RpgRollContribution> {
        let mut contributions = Vec::new();
        for feature in &self.character_features {
            for contribution in &feature.roll_contributions {
                if contribution.selector != RpgRollContributionSelector::Attack
                    || !self.roll_condition_applies(&contribution.condition, target_id)
                {
                    continue;
                }
                contributions.push(RpgRollContribution {
                    source_definition_id: feature.definition_id.clone(),
                    source_label: feature.label.clone(),
                    amount: contribution.amount,
                    reason: RpgRollContributionReason::CharacterFeature {
                        contribution_id: contribution.id.clone(),
                        selector: contribution.selector,
                        condition: contribution.condition.clone(),
                    },
                });
            }
        }
        contributions
    }

    fn roll_condition_applies(
        &self,
        condition: &RpgRollContributionCondition,
        target_id: &str,
    ) -> bool {
        match condition {
            RpgRollContributionCondition::Always => true,
            RpgRollContributionCondition::ActorFlanksTarget => self.actor_flanks_target(target_id),
            RpgRollContributionCondition::ActorSurrounded { minimum_hostiles } => {
                self.actor_adjacent_living_hostile_count() >= *minimum_hostiles
            }
            RpgRollContributionCondition::All { conditions } => conditions
                .iter()
                .all(|condition| self.roll_condition_applies(condition, target_id)),
        }
    }

    fn actor_flanks_target(&self, target_id: &str) -> bool {
        let state = self.workspace.state();
        let Some(actor) = state.entity(&self.intent.actor_id) else {
            return false;
        };
        let Some(target) = state.entity(target_id) else {
            return false;
        };
        if actor.vitality().current <= 0
            || target.vitality().current <= 0
            || target.team() == actor.team()
            || cardinal_distance(actor.position(), target.position()) != 1
        {
            return false;
        }
        state.entities().any(|ally| {
            ally.id() != actor.id()
                && ally.id() != target.id()
                && ally.team() == actor.team()
                && ally.vitality().current > 0
                && cardinal_distance(ally.position(), target.position()) == 1
                && positions_are_opposite(actor.position(), target.position(), ally.position())
        })
    }

    fn actor_adjacent_living_hostile_count(&self) -> u32 {
        let state = self.workspace.state();
        let Some(actor) = state.entity(&self.intent.actor_id) else {
            return 0;
        };
        if actor.vitality().current <= 0 {
            return 0;
        }
        u32::try_from(
            state
                .entities()
                .filter(|candidate| {
                    candidate.id() != actor.id()
                        && candidate.team() != actor.team()
                        && candidate.vitality().current > 0
                        && cardinal_distance(candidate.position(), actor.position()) == 1
                })
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    fn execute_program(
        &mut self,
        program: &CompiledProgram,
        path: &str,
    ) -> Result<(), RpgResolutionRejection> {
        match program {
            CompiledProgram::Operation(operation) => self.execute_operation(operation, path),
            CompiledProgram::Sequence(steps) => {
                for (index, step) in steps.iter().enumerate() {
                    self.execute_program(step, &format!("{path}.steps[{index}]"))?;
                }
                Ok(())
            }
            CompiledProgram::When {
                predicate,
                then,
                otherwise,
            } => {
                let predicate_result =
                    self.eval_predicate(predicate, &format!("{path}.predicate"))?;
                self.trace.push(RpgTraceStep {
                    path: path.to_owned(),
                    code: "RPG_BRANCH_SELECTED".to_owned(),
                    detail: format!("predicate {predicate_result}"),
                });
                if predicate_result {
                    self.execute_program(then, &format!("{path}.then"))
                } else if let Some(otherwise) = otherwise {
                    self.execute_program(otherwise, &format!("{path}.otherwise"))
                } else {
                    Ok(())
                }
            }
            CompiledProgram::Repeat { count, body } => {
                for index in 0..*count {
                    self.execute_program(body, &format!("{path}.repeat[{index}]"))?;
                }
                Ok(())
            }
            CompiledProgram::ForEachTarget { maximum, body } => {
                if self.target_ids.len() > *maximum as usize {
                    return Err(self.fail(
                        "RPG_RUNTIME_TARGET_BOUND_EXCEEDED",
                        path,
                        format!("target count exceeds program bound {maximum}"),
                    ));
                }
                let target_ids = self.target_ids.clone();
                for (index, target_id) in target_ids.into_iter().enumerate() {
                    self.current_target = Some(target_id);
                    self.execute_program(body, &format!("{path}.targets[{index}]"))?;
                }
                self.current_target = None;
                Ok(())
            }
            CompiledProgram::OnCheck {
                hit,
                miss,
                saved,
                failed,
                no_roll,
            } => {
                let outcome = self.current_outcome(path)?;
                let selected = match outcome {
                    CheckOutcome::Hit => hit,
                    CheckOutcome::Miss => miss,
                    CheckOutcome::Saved => saved,
                    CheckOutcome::Failed => failed,
                    CheckOutcome::NoRoll => no_roll,
                };
                self.trace.push(RpgTraceStep {
                    path: path.to_owned(),
                    code: "RPG_CHECK_BRANCH_SELECTED".to_owned(),
                    detail: format!("outcome {outcome:?}"),
                });
                if let Some(selected) = selected {
                    self.execute_program(selected, &format!("{path}.selected"))?;
                }
                Ok(())
            }
            CompiledProgram::Atomic(body) => {
                self.trace.push(RpgTraceStep {
                    path: path.to_owned(),
                    code: "RPG_ATOMIC_WORKSPACE_OPENED".to_owned(),
                    detail: format!("base revision {}", self.workspace.state().revision()),
                });
                self.execute_program(body, &format!("{path}.body"))
            }
        }
    }

    fn execute_operation(
        &mut self,
        operation: &CompiledOperation,
        path: &str,
    ) -> Result<(), RpgResolutionRejection> {
        let expected_owner = match operation.declaration {
            RpgIrOperation::Damage { .. } | RpgIrOperation::Heal { .. } => {
                RpgCapabilityId::Vitality
            }
            RpgIrOperation::ChangeResource { .. } => RpgCapabilityId::Resources,
            RpgIrOperation::ApplyModifier { .. } => RpgCapabilityId::Modifiers,
            RpgIrOperation::Move { .. } | RpgIrOperation::MoveToCell { .. } => {
                RpgCapabilityId::Position
            }
            RpgIrOperation::OpenReaction { .. } => RpgCapabilityId::Reactions,
        };
        if operation
            .binding
            .bind_mutation_owner(expected_owner)
            .is_err()
        {
            return Err(self.fail(
                "RPG_OPERATION_OWNER_MISMATCH",
                path,
                format!(
                    "{} binds {}, but the operation requires {}",
                    operation.binding.id,
                    operation.binding.mutation_owner.as_str(),
                    expected_owner.as_str()
                ),
            ));
        }
        match &operation.declaration {
            RpgIrOperation::Damage {
                amount,
                damage_type,
            } => {
                let target_id = self.target_id(path)?;
                let requested_amount =
                    self.eval_nonnegative_formula(amount, &format!("{path}.amount"))?;
                let reduction = i32::try_from(self.pending_damage_reduction).unwrap_or(i32::MAX);
                let amount = requested_amount.saturating_sub(reduction).max(0);
                self.pending_damage_reduction = 0;
                let remaining_vitality = self
                    .workspace
                    .vitality_owner()
                    .apply_damage(&target_id, amount)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::DamageApplied {
                    source_id: self.intent.actor_id.clone(),
                    target_id,
                    amount,
                    damage_type: damage_type.clone(),
                    remaining_vitality,
                });
            }
            RpgIrOperation::Heal { amount } => {
                let target_id = self.target_id(path)?;
                let amount = self.eval_nonnegative_formula(amount, &format!("{path}.amount"))?;
                let current_vitality = self
                    .workspace
                    .vitality_owner()
                    .apply_healing(&target_id, amount)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::HealingApplied {
                    source_id: self.intent.actor_id.clone(),
                    target_id,
                    amount,
                    current_vitality,
                });
            }
            RpgIrOperation::ChangeResource {
                subject,
                resource_id,
                delta,
            } => {
                let entity_id = self.subject_id(*subject, path)?;
                let delta = self.eval_formula(delta, &format!("{path}.delta"))?;
                let current = self
                    .workspace
                    .resources_owner()
                    .change(&entity_id, resource_id, delta)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::ResourceChanged {
                    entity_id,
                    resource_id: resource_id.clone(),
                    delta,
                    current,
                });
            }
            RpgIrOperation::ApplyModifier {
                modifier_id,
                stacking_group,
                stacking,
                value,
                duration_turns,
            } => {
                let target_id = self.target_id(path)?;
                let value = self.eval_formula(value, &format!("{path}.value"))?;
                let stacking = match stacking {
                    rpg_ir::RpgIrStackingPolicy::Replace => RpgModifierStackingPolicy::Replace,
                    rpg_ir::RpgIrStackingPolicy::Refresh => RpgModifierStackingPolicy::Refresh,
                };
                self.workspace
                    .modifiers_owner()
                    .apply(
                        &target_id,
                        modifier_id,
                        stacking_group,
                        stacking,
                        value,
                        *duration_turns,
                    )
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::ModifierApplied {
                    source_id: self.intent.actor_id.clone(),
                    target_id,
                    modifier_id: modifier_id.clone(),
                    stacking_group: stacking_group.clone(),
                    stacking,
                    value,
                    remaining_turns: *duration_turns,
                });
            }
            RpgIrOperation::Move {
                subject,
                delta_x,
                delta_y,
                maximum_distance,
                provokes,
            } => {
                let entity_id = self.subject_id(*subject, path)?;
                let delta_x = self.eval_formula(delta_x, &format!("{path}.deltaX"))?;
                let delta_y = self.eval_formula(delta_y, &format!("{path}.deltaY"))?;
                let (previous, current) = self
                    .workspace
                    .position_owner()
                    .move_entity(&entity_id, delta_x, delta_y, *maximum_distance)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::PositionChanged {
                    source_id: self.intent.actor_id.clone(),
                    entity_id,
                    previous,
                    current,
                    provokes: *provokes,
                });
            }
            RpgIrOperation::MoveToCell {
                maximum_distance,
                provokes,
            } => {
                let target_id = self.target_id(path)?;
                let destination =
                    self.intent
                        .cell_targets
                        .iter()
                        .find(|target| target.id == target_id)
                        .map(|target| target.position)
                        .ok_or_else(|| {
                            self.fail(
                        "RPG_RUNTIME_CELL_BINDING_MISSING",
                        path,
                        format!("selected cell {target_id} has no authoritative position binding"),
                    )
                        })?;
                let previous = self
                    .workspace
                    .state()
                    .entity(&self.intent.actor_id)
                    .map(|entity| entity.position())
                    .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_ACTOR_MISSING",
                            path,
                            format!("actor {} is missing", self.intent.actor_id),
                        )
                    })?;
                let delta_x = i64::from(destination.x) - i64::from(previous.x);
                let delta_y = i64::from(destination.y) - i64::from(previous.y);
                let delta_x = i32::try_from(delta_x).map_err(|_| {
                    self.fail(
                        "RPG_RUNTIME_MOVEMENT_DELTA_INVALID",
                        path,
                        "selected cell x delta exceeds the supported position space",
                    )
                })?;
                let delta_y = i32::try_from(delta_y).map_err(|_| {
                    self.fail(
                        "RPG_RUNTIME_MOVEMENT_DELTA_INVALID",
                        path,
                        "selected cell y delta exceeds the supported position space",
                    )
                })?;
                let (previous, current) = self
                    .workspace
                    .position_owner()
                    .move_entity(&self.intent.actor_id, delta_x, delta_y, *maximum_distance)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                self.events.push(RpgDomainEvent::PositionChanged {
                    source_id: self.intent.actor_id.clone(),
                    entity_id: self.intent.actor_id.clone(),
                    previous,
                    current,
                    provokes: *provokes,
                });
            }
            RpgIrOperation::OpenReaction {
                reaction_id,
                options,
            } => {
                if self.reaction_consumed {
                    return Err(self.fail(
                        "RPG_REACTION_MULTIPLE_WINDOWS_UNSUPPORTED",
                        path,
                        "one command may open only one reaction window",
                    ));
                }
                let target_id = self.target_id(path)?;
                let request = RpgReactionRequest {
                    reaction_id: reaction_id.clone(),
                    actor_id: self.intent.actor_id.clone(),
                    target_id: target_id.clone(),
                    action_id: self.intent.action_id.clone(),
                    options: options
                        .iter()
                        .map(|option| RpgReactionOption {
                            id: option.id.clone(),
                            label: option.label.clone(),
                            damage_reduction: option.damage_reduction,
                        })
                        .collect(),
                    path: path.to_owned(),
                };
                let decision = match self.reaction {
                    Some(decision) => decision,
                    None => {
                        let mut rejection = self.fail(
                            "RPG_REACTION_REQUIRED",
                            path,
                            "the staged command is awaiting a reaction decision",
                        );
                        rejection.reaction_request = Some(Box::new(request));
                        return Err(rejection);
                    }
                };
                if decision.reaction_id != *reaction_id {
                    return Err(self.fail(
                        "RPG_REACTION_ID_MISMATCH",
                        "$.reaction.reactionId",
                        format!("expected reaction {reaction_id}"),
                    ));
                }
                let damage_reduction = match &decision.option_id {
                    Some(option_id) => options
                        .iter()
                        .find(|option| option.id == *option_id)
                        .map(|option| option.damage_reduction)
                        .ok_or_else(|| {
                            self.fail(
                                "RPG_REACTION_OPTION_UNKNOWN",
                                "$.reaction.optionId",
                                format!("unknown reaction option {option_id}"),
                            )
                        })?,
                    None => 0,
                };
                self.reaction_consumed = true;
                self.pending_damage_reduction = damage_reduction;
                self.events.push(RpgDomainEvent::ReactionOpened {
                    reaction_id: reaction_id.clone(),
                    actor_id: self.intent.actor_id.clone(),
                    target_id,
                    action_id: self.intent.action_id.clone(),
                });
                self.events.push(RpgDomainEvent::ReactionResolved {
                    reaction_id: reaction_id.clone(),
                    option_id: decision.option_id.clone(),
                    damage_reduction,
                });
                self.trace.push(RpgTraceStep {
                    path: path.to_owned(),
                    code: "RPG_REACTION_RESOLVED".to_owned(),
                    detail: format!(
                        "{} selected with damage reduction {damage_reduction}",
                        decision.option_id.as_deref().unwrap_or("decline")
                    ),
                });
            }
        }
        self.trace.push(RpgTraceStep {
            path: path.to_owned(),
            code: "RPG_OPERATION_STAGED".to_owned(),
            detail: format!("{}@{}", operation.binding.id, operation.binding.version),
        });
        Ok(())
    }

    fn eval_nonnegative_formula(
        &mut self,
        formula: &RpgIrFormula,
        path: &str,
    ) -> Result<i32, RpgResolutionRejection> {
        let value = self.eval_formula(formula, path)?;
        if value < 0 {
            return Err(self.fail(
                "RPG_RUNTIME_AMOUNT_NEGATIVE",
                path,
                format!("operation amount resolved to {value}"),
            ));
        }
        Ok(value)
    }

    fn eval_formula(
        &mut self,
        formula: &RpgIrFormula,
        path: &str,
    ) -> Result<i32, RpgResolutionRejection> {
        match formula {
            RpgIrFormula::Constant { value } => Ok(*value),
            RpgIrFormula::ReadStat { subject, stat_id } => {
                let entity_id = self.subject_id(*subject, path)?;
                self.workspace
                    .state()
                    .entity(&entity_id)
                    .and_then(|entity| entity.stat(stat_id))
                    .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_STAT_MISSING",
                            path,
                            format!("entity {entity_id} has no stat {stat_id}"),
                        )
                    })
            }
            RpgIrFormula::Add { terms } => {
                let mut total = 0_i32;
                for (index, term) in terms.iter().enumerate() {
                    let value = self.eval_formula(term, &format!("{path}.terms[{index}]"))?;
                    total = total.checked_add(value).ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_INTEGER_OVERFLOW",
                            path,
                            "formula addition overflowed",
                        )
                    })?;
                }
                Ok(total)
            }
            RpgIrFormula::Dice {
                count,
                sides,
                bonus,
            } => {
                self.require_random(RpgRandomRequestKind::FormulaDice, *count, *sides, path)?;
                let mut total = *bonus;
                for index in 0..*count {
                    let roll = self.take_random(
                        RpgRandomRequestKind::FormulaDice,
                        *sides,
                        &format!("{path}.dice[{index}]"),
                    )?;
                    let roll = i32::try_from(roll).map_err(|_| {
                        self.fail(
                            "RPG_RUNTIME_INTEGER_OVERFLOW",
                            path,
                            "random value does not fit formula integer range",
                        )
                    })?;
                    total = total.checked_add(roll).ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_INTEGER_OVERFLOW",
                            path,
                            "dice formula overflowed",
                        )
                    })?;
                }
                Ok(total)
            }
            RpgIrFormula::Half { value } => {
                Ok(self.eval_formula(value, &format!("{path}.value"))? / 2)
            }
        }
    }

    fn eval_predicate(
        &mut self,
        predicate: &RpgIrPredicate,
        path: &str,
    ) -> Result<bool, RpgResolutionRejection> {
        match predicate {
            RpgIrPredicate::Always => Ok(true),
            RpgIrPredicate::Compare {
                left,
                comparison,
                right,
            } => {
                let left = self.eval_formula(left, &format!("{path}.left"))?;
                let right = self.eval_formula(right, &format!("{path}.right"))?;
                Ok(match comparison {
                    RpgIrComparison::Equal => left == right,
                    RpgIrComparison::NotEqual => left != right,
                    RpgIrComparison::LessThan => left < right,
                    RpgIrComparison::LessThanOrEqual => left <= right,
                    RpgIrComparison::GreaterThan => left > right,
                    RpgIrComparison::GreaterThanOrEqual => left >= right,
                })
            }
            RpgIrPredicate::Not { predicate } => {
                Ok(!self.eval_predicate(predicate, &format!("{path}.predicate"))?)
            }
            RpgIrPredicate::All { predicates } => {
                for (index, predicate) in predicates.iter().enumerate() {
                    if !self.eval_predicate(predicate, &format!("{path}.predicates[{index}]"))? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            RpgIrPredicate::Any { predicates } => {
                for (index, predicate) in predicates.iter().enumerate() {
                    if self.eval_predicate(predicate, &format!("{path}.predicates[{index}]"))? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    fn require_random(
        &self,
        kind: RpgRandomRequestKind,
        count: u32,
        sides: u32,
        path: &str,
    ) -> Result<(), RpgResolutionRejection> {
        let available = u32::try_from(self.workspace.random_remaining()).unwrap_or(u32::MAX);
        if available >= count {
            return Ok(());
        }
        Err(self.random_rejection(kind, count - available, sides, path))
    }

    fn take_random(
        &mut self,
        kind: RpgRandomRequestKind,
        sides: u32,
        path: &str,
    ) -> Result<u32, RpgResolutionRejection> {
        let value = self
            .workspace
            .random_owner()
            .take()
            .ok_or_else(|| self.random_rejection(kind, 1, sides, path))?;
        if value == 0 || value > sides {
            return Err(self.fail(
                "RPG_RANDOM_VALUE_OUT_OF_RANGE",
                path,
                format!("random value {value} is outside 1..={sides}"),
            ));
        }
        self.trace.push(RpgTraceStep {
            path: path.to_owned(),
            code: "RPG_RANDOM_CONSUMED".to_owned(),
            detail: format!("d{sides}={value}"),
        });
        self.random_evidence.push(RpgRandomEvidence {
            request: RpgRandomRequest {
                kind,
                count: 1,
                sides,
                path: path.to_owned(),
            },
            values: vec![value],
        });
        Ok(value)
    }

    fn random_rejection(
        &self,
        kind: RpgRandomRequestKind,
        count: u32,
        sides: u32,
        path: &str,
    ) -> RpgResolutionRejection {
        let mut rejection = self.fail(
            "RPG_RANDOM_EXHAUSTED",
            path,
            "deterministic random stream is exhausted",
        );
        rejection.random_request = Some(Box::new(RpgRandomRequest {
            kind,
            count,
            sides,
            path: path.to_owned(),
        }));
        rejection
    }

    fn current_outcome(&self, path: &str) -> Result<CheckOutcome, RpgResolutionRejection> {
        let target_id = self.target_id(path)?;
        self.outcomes.get(&target_id).copied().ok_or_else(|| {
            self.fail(
                "RPG_RUNTIME_CHECK_OUTCOME_MISSING",
                path,
                format!("target {target_id} has no check outcome"),
            )
        })
    }

    fn subject_id(
        &self,
        subject: RpgIrSubject,
        path: &str,
    ) -> Result<String, RpgResolutionRejection> {
        match subject {
            RpgIrSubject::Actor => Ok(self.intent.actor_id.clone()),
            RpgIrSubject::Target => self.target_id(path),
        }
    }

    fn target_id(&self, path: &str) -> Result<String, RpgResolutionRejection> {
        if let Some(target_id) = &self.current_target {
            return Ok(target_id.clone());
        }
        if self.target_ids.len() == 1 {
            return Ok(self.target_ids[0].clone());
        }
        Err(self.fail(
            "RPG_RUNTIME_TARGET_BINDING_MISSING",
            path,
            "operation requires a current target",
        ))
    }

    fn mutation_rejection(
        &self,
        error: RpgCapabilityMutationError,
        path: &str,
    ) -> RpgResolutionRejection {
        let (code, message) = match error {
            RpgCapabilityMutationError::UnknownEntity => {
                ("RPG_MUTATION_ENTITY_UNKNOWN", "mutation entity is unknown")
            }
            RpgCapabilityMutationError::UnknownResource => (
                "RPG_MUTATION_RESOURCE_UNKNOWN",
                "mutation resource is unknown",
            ),
            RpgCapabilityMutationError::InvalidAmount => {
                ("RPG_MUTATION_AMOUNT_INVALID", "mutation amount is invalid")
            }
            RpgCapabilityMutationError::InsufficientResource => (
                "RPG_MUTATION_RESOURCE_INSUFFICIENT",
                "mutation resource is insufficient",
            ),
            RpgCapabilityMutationError::ResourceOutOfBounds => (
                "RPG_MUTATION_RESOURCE_OUT_OF_BOUNDS",
                "resource transition exceeds its declared bounds",
            ),
            RpgCapabilityMutationError::ModifierTenureInvalid => (
                "RPG_MUTATION_MODIFIER_TENURE_INVALID",
                "modifier tenure is outside the supported turn bounds",
            ),
            RpgCapabilityMutationError::MovementDistanceInvalid => (
                "RPG_MUTATION_MOVEMENT_DISTANCE_INVALID",
                "movement distance is zero or exceeds its bound",
            ),
            RpgCapabilityMutationError::PositionOutOfBounds => (
                "RPG_MUTATION_POSITION_OUT_OF_BOUNDS",
                "movement leaves the supported position space",
            ),
        };
        self.fail(code, path, message)
    }

    fn fail(&self, code: &str, path: &str, message: impl Into<String>) -> RpgResolutionRejection {
        RpgResolutionRejection {
            code: code.to_owned(),
            path: path.to_owned(),
            message: message.into(),
            trace: Box::new(self.trace.clone()),
            random_evidence: Box::new(self.random_evidence.clone()),
            random_attempted: u64::try_from(
                self.workspace
                    .random_consumed()
                    .saturating_sub(self.random_start),
            )
            .unwrap_or(u64::MAX),
            random_request: None,
            reaction_request: None,
        }
    }
}

fn cardinal_distance(left: GridPosition, right: GridPosition) -> u32 {
    left.x
        .abs_diff(right.x)
        .saturating_add(left.y.abs_diff(right.y))
}

fn positions_are_opposite(first: GridPosition, center: GridPosition, second: GridPosition) -> bool {
    (first.y == center.y
        && second.y == center.y
        && u64::from(first.x).saturating_add(u64::from(second.x))
            == u64::from(center.x).saturating_mul(2))
        || (first.x == center.x
            && second.x == center.x
            && u64::from(first.y).saturating_add(u64::from(second.y))
                == u64::from(center.y).saturating_mul(2))
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
