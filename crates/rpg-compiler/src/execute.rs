use std::collections::BTreeMap;

use rpg_core::{
    DeterministicRandomStream, RpgCapabilityId, RpgCapabilityMutationError, RpgCapabilityState,
    RpgCapabilityWorkspace, RpgDomainEvent, RpgIntent, RpgModifierStackingPolicy, RpgRandomRequest,
    RpgRandomRequestKind, RpgReactionDecision, RpgReactionOption, RpgReactionRequest,
    RpgResolutionReceipt, RpgResolutionRejection, RpgTraceStep,
};
use rpg_ir::{
    RpgIrCheck, RpgIrComparison, RpgIrFormula, RpgIrOperation, RpgIrPredicate, RpgIrRollScope,
    RpgIrSubject, RpgIrTeamConstraint,
};

use crate::compile::{CompiledAction, CompiledOperation, CompiledProgram};
use crate::CompiledRpgRuleset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckOutcome {
    Hit,
    Miss,
    Saved,
    Failed,
    NoRoll,
}

impl CompiledRpgRuleset {
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

    fn resolve_internal(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
        reaction: Option<&RpgReactionDecision>,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        let action = self.action(&intent.action_id).ok_or_else(|| {
            rejection(
                "RPG_INTENT_ACTION_UNKNOWN",
                "$.intent.actionId",
                format!("unknown action {}", intent.action_id),
            )
        })?;
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
            current_target: None,
            reaction,
            reaction_consumed: false,
            pending_damage_reduction: 0,
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

        let random_consumed = execution
            .workspace
            .random_consumed()
            .saturating_sub(execution.random_start);
        let receipt = RpgResolutionReceipt {
            action_id: intent.action_id.clone(),
            actor_id: intent.actor_id.clone(),
            target_ids: execution.target_ids.clone(),
            events: execution.events,
            trace: execution.trace,
            random_consumed,
            state_revision: revision,
        };
        execution.workspace.commit(state, random);
        Ok(receipt)
    }

    pub fn candidate_ids(
        &self,
        state: &RpgCapabilityState,
        actor_id: &str,
        action_id: &str,
    ) -> Result<Vec<String>, RpgResolutionRejection> {
        let action = self.action(action_id).ok_or_else(|| {
            rejection(
                "RPG_INTENT_ACTION_UNKNOWN",
                "$.actionId",
                format!("unknown action {action_id}"),
            )
        })?;
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

    pub fn preflight(
        &self,
        state: &RpgCapabilityState,
        intent: &RpgIntent,
    ) -> Result<(), RpgResolutionRejection> {
        let action = self.action(&intent.action_id).ok_or_else(|| {
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
    current_target: Option<String>,
    reaction: Option<&'a RpgReactionDecision>,
    reaction_consumed: bool,
    pending_damage_reduction: u32,
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
                    let total = i32::try_from(roll)
                        .unwrap_or(i32::MAX)
                        .saturating_add(modifier);
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
            RpgIrOperation::Move { .. } => RpgCapabilityId::Position,
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
            trace: self.trace.clone(),
            random_attempted: self
                .workspace
                .random_consumed()
                .saturating_sub(self.random_start),
            random_request: None,
            reaction_request: None,
        }
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
        trace: Vec::new(),
        random_attempted: 0,
        random_request: None,
        reaction_request: None,
    }
}
