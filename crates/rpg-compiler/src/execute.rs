use std::collections::BTreeMap;

use rpg_core::{
    DeterministicRandomStream, RpgCapabilityMutationError, RpgCapabilityState, RpgDomainEvent,
    RpgIntent, RpgModifierStackingPolicy, RpgResolutionReceipt, RpgResolutionRejection,
    RpgTraceStep,
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
            staged_state: state.clone(),
            staged_random: random.clone(),
            random_start: random.consumed(),
            outcomes: BTreeMap::new(),
            events: Vec::new(),
            trace: Vec::new(),
            current_target: None,
        };

        execution.spend_costs()?;
        execution.resolve_checks()?;
        execution.execute_program(&action.program, "$.action.program")?;
        let revision = execution.staged_state.advance_revision();
        execution.trace.push(RpgTraceStep {
            path: "$.resolution.commit".to_owned(),
            code: "RPG_RESOLUTION_COMMITTED".to_owned(),
            detail: format!("state revision {revision}"),
        });

        let random_consumed = execution
            .staged_random
            .consumed()
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
        *state = execution.staged_state;
        *random = execution.staged_random;
        Ok(receipt)
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
    staged_state: RpgCapabilityState,
    staged_random: DeterministicRandomStream,
    random_start: usize,
    outcomes: BTreeMap<String, CheckOutcome>,
    events: Vec<RpgDomainEvent>,
    trace: Vec<RpgTraceStep>,
    current_target: Option<String>,
}

impl Execution<'_> {
    fn spend_costs(&mut self) -> Result<(), RpgResolutionRejection> {
        for (index, cost) in self.action.costs.iter().enumerate() {
            let path = format!("$.action.costs[{index}]");
            let remaining = self
                .staged_state
                .spend_resource(&self.intent.actor_id, &cost.resource_id, cost.amount)
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
            Some(self.take_random(20, "$.action.check.sharedRoll")?)
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
                        None => self.take_random(20, &format!("{path}.roll"))?,
                    };
                    let modifier = self.eval_formula(modifier, &format!("{path}.modifier"))?;
                    let total = i32::try_from(roll)
                        .unwrap_or(i32::MAX)
                        .saturating_add(modifier);
                    let defense = self
                        .staged_state
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
                        None => self.take_random(20, &format!("{path}.roll"))?,
                    };
                    let difficulty =
                        self.eval_formula(difficulty, &format!("{path}.difficulty"))?;
                    let defense = self
                        .staged_state
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
                    detail: format!("base revision {}", self.staged_state.revision()),
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
        match &operation.declaration {
            RpgIrOperation::Damage {
                amount,
                damage_type,
            } => {
                let target_id = self.target_id(path)?;
                let amount = self.eval_nonnegative_formula(amount, &format!("{path}.amount"))?;
                let remaining_vitality = self
                    .staged_state
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
                    .staged_state
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
                    .staged_state
                    .change_resource(&entity_id, resource_id, delta)
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
                self.staged_state
                    .apply_modifier(
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
                    .staged_state
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
                self.staged_state
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
                let mut total = *bonus;
                for index in 0..*count {
                    let roll = self.take_random(*sides, &format!("{path}.dice[{index}]"))?;
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

    fn take_random(&mut self, sides: u32, path: &str) -> Result<u32, RpgResolutionRejection> {
        let value = self.staged_random.take().ok_or_else(|| {
            self.fail(
                "RPG_RANDOM_EXHAUSTED",
                path,
                "deterministic random stream is exhausted",
            )
        })?;
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
                .staged_random
                .consumed()
                .saturating_sub(self.random_start),
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
    }
}
