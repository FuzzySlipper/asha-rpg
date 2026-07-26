use std::collections::{BTreeMap, BTreeSet};

use rpg_core::{
    ActiveRpgEffect, DeterministicRandomStream, GridPosition, RpgCapabilityId,
    RpgCapabilityMutationError, RpgCapabilityState, RpgCapabilityWorkspace,
    RpgContributionComparison, RpgContributionDisposition, RpgContributionPredicate,
    RpgContributionStackingPolicy, RpgContributionSubject, RpgContributionTeamRelation,
    RpgContributionValueExpression, RpgDomainEvent, RpgEffectMutation, RpgHeterogeneousRandomTerm,
    RpgHeterogeneousRandomValue, RpgIntent, RpgModifierStackingPolicy, RpgNaturalDieEffect,
    RpgNaturalDieResolution, RpgOutcomeBandShiftDecision, RpgOutcomeBandShiftDefinition,
    RpgOutcomeBandShiftDisposition, RpgOutcomeBandShiftLedger, RpgPoolCancellationResult,
    RpgPoolContributionDecision, RpgPoolContributionDefinition, RpgPoolContributionEffect,
    RpgPoolContributionLedger, RpgPoolReplacementUnit, RpgRandomEvidence, RpgRandomRequest,
    RpgRandomRequestKind, RpgReactionActivationBudgetCost, RpgReactionDecision, RpgReactionOption,
    RpgReactionRequest, RpgReactionUnavailable, RpgResolutionContext, RpgResolutionReceipt,
    RpgResolutionRejection, RpgRulesetValueKind, RpgScalarContributionDecision,
    RpgScalarContributionDefinition, RpgScalarContributionLedger, RpgTraceStep,
};
use rpg_ir::{
    CompiledCharacterFeature, CompiledEffectDefinition, CompiledItemDefinition, RpgIrActivation,
    RpgIrCheck, RpgIrComparison, RpgIrFormula, RpgIrOperation, RpgIrPredicate, RpgIrRollScope,
    RpgIrScalarTestDifficulty, RpgIrSubject, RpgIrTargetKind, RpgIrTeamConstraint,
    RulesetHeterogeneousPoolProfile, RulesetMarginBandRule, RulesetNaturalDieRule,
    RulesetOutcomeBand,
};

use crate::compile::{CompiledAction, CompiledOperation, CompiledProgram};
use crate::CompiledRpgRules;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckOutcome {
    Hit,
    Miss,
    Saved,
    Failed,
    NoRoll,
    Scalar { profile_id: String, band_id: String },
    Vector { profile_id: String, band_id: String },
}

impl CompiledRpgRules {
    pub fn resolve(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_with_context(state, random, intent, &RpgResolutionContext::default())
    }

    pub fn resolve_with_context(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
        context: &RpgResolutionContext,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_internal(state, random, intent, None, context)
    }

    pub fn resolve_with_reaction_decision(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
        reaction: &RpgReactionDecision,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_with_reaction_decision_and_context(
            state,
            random,
            intent,
            reaction,
            &RpgResolutionContext::default(),
        )
    }

    pub fn resolve_with_reaction_decision_and_context(
        &self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &RpgIntent,
        reaction: &RpgReactionDecision,
        context: &RpgResolutionContext,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.resolve_internal(state, random, intent, Some(reaction), context)
    }

    fn resolve_internal<'a>(
        &'a self,
        state: &mut RpgCapabilityState,
        random: &mut DeterministicRandomStream,
        intent: &'a RpgIntent,
        reaction: Option<&'a RpgReactionDecision>,
        context: &'a RpgResolutionContext,
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
        let bound_item = intent
            .item_binding
            .as_ref()
            .and_then(|binding| self.item(&binding.item_definition_id));
        let target_ids = validate_intent(action, state, intent)?;
        let mut execution = Execution {
            rules: self,
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
            next_effect_ordinal: 0,
            character_features,
            bound_item,
            context,
        };

        if let Some(activation) = &action.activation {
            execution.spend_activation(&intent.actor_id, activation, "$.action.activation")?;
        }
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
        validate_intent(action, state, intent)?;
        if let Some(activation) = &action.activation {
            activation_rejection(
                self,
                state,
                &intent.actor_id,
                activation,
                "$.action.activation",
            )?;
        }
        Ok(())
    }
}

fn activation_rejection(
    rules: &CompiledRpgRules,
    state: &RpgCapabilityState,
    entity_id: &str,
    activation: &RpgIrActivation,
    path: &str,
) -> Result<(), RpgResolutionRejection> {
    let Some(ceiling) = rules.accepted_activation_ceiling() else {
        return Err(rejection(
            "RPG_ACTIVATION_MODEL_UNAVAILABLE",
            path,
            "activation was submitted without the variable activation-budget model",
        ));
    };
    if state.accepted_activations_this_turn() >= ceiling {
        return Err(rejection(
            "RPG_ACTIVATION_CEILING_REACHED",
            path,
            format!("the turn has reached its {ceiling}-activation ceiling"),
        ));
    }
    let entity = state.entity(entity_id).ok_or_else(|| {
        rejection(
            "RPG_ACTIVATION_ENTITY_UNKNOWN",
            path,
            format!("unknown activation owner {entity_id}"),
        )
    })?;
    for (index, cost) in activation.costs.iter().enumerate() {
        let remaining = entity.activation_budget(&cost.budget.id).ok_or_else(|| {
            rejection(
                "RPG_ACTIVATION_BUDGET_UNKNOWN",
                format!("{path}.costs[{index}].budget"),
                format!(
                    "entity {entity_id} has no activation budget {}",
                    cost.budget.id
                ),
            )
        })?;
        if remaining < cost.amount {
            return Err(rejection(
                "RPG_ACTIVATION_BUDGET_INSUFFICIENT",
                format!("{path}.costs[{index}]"),
                format!(
                    "entity {entity_id} cannot pay {} {} with {remaining} remaining",
                    cost.amount, cost.budget.id
                ),
            ));
        }
    }
    Ok(())
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
    rules: &'a CompiledRpgRules,
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
    next_effect_ordinal: u32,
    character_features: Vec<&'a CompiledCharacterFeature>,
    bound_item: Option<&'a CompiledItemDefinition>,
    context: &'a RpgResolutionContext,
}

impl Execution<'_> {
    fn active_effect_sources(
        &self,
        target_id: &str,
        path: &str,
    ) -> Result<Vec<(ActiveRpgEffect, CompiledEffectDefinition)>, RpgResolutionRejection> {
        let mut entity_ids = vec![self.intent.actor_id.as_str(), target_id];
        entity_ids.sort_unstable();
        entity_ids.dedup();
        let mut sources = Vec::new();
        for entity_id in entity_ids {
            let entity = self.workspace.state().entity(entity_id).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_EFFECT_OWNER_UNKNOWN",
                    path,
                    format!("effect owner {entity_id} is unavailable"),
                )
            })?;
            for effect in entity.effects() {
                let definition = self.rules.effect(effect.definition_id()).ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_EFFECT_DEFINITION_UNKNOWN",
                        path,
                        format!(
                            "active effect {} references unavailable definition {}",
                            effect.instance_id(),
                            effect.definition_id()
                        ),
                    )
                })?;
                if effect.definition_version() != definition.definition_version {
                    return Err(self.fail(
                        "RPG_RUNTIME_EFFECT_DEFINITION_VERSION_MISMATCH",
                        path,
                        format!(
                            "active effect {} references {}@{}, but the compiled definition is {}@{}",
                            effect.instance_id(),
                            effect.definition_id(),
                            effect.definition_version(),
                            definition.definition_id,
                            definition.definition_version
                        ),
                    ));
                }
                sources.push((effect.clone(), definition.clone()));
            }
        }
        sources.sort_by(|left, right| {
            (
                left.0.definition_id(),
                left.0.source_entity_id(),
                left.0.instance_id(),
            )
                .cmp(&(
                    right.0.definition_id(),
                    right.0.source_entity_id(),
                    right.0.instance_id(),
                ))
        });
        Ok(sources)
    }

    fn spend_activation(
        &mut self,
        entity_id: &str,
        activation: &RpgIrActivation,
        path: &str,
    ) -> Result<(), RpgResolutionRejection> {
        activation_rejection(
            self.rules,
            self.workspace.state(),
            entity_id,
            activation,
            path,
        )?;
        let ceiling = self
            .rules
            .accepted_activation_ceiling()
            .expect("validated activation has a variable-model ceiling");
        let accepted_activations = self
            .workspace
            .activation_budgets_owner()
            .accept_activation(ceiling)
            .map_err(|error| self.mutation_rejection(error, path))?;
        for (index, cost) in activation.costs.iter().enumerate() {
            let cost_path = format!("{path}.costs[{index}]");
            let (previous, remaining) = self
                .workspace
                .activation_budgets_owner()
                .spend(entity_id, &cost.budget.id, cost.amount)
                .map_err(|error| self.mutation_rejection(error, &cost_path))?;
            self.events.push(RpgDomainEvent::ActivationBudgetSpent {
                entity_id: entity_id.to_owned(),
                budget_id: cost.budget.id.clone(),
                amount: cost.amount,
                previous,
                remaining,
                accepted_activations,
            });
        }
        self.trace.push(RpgTraceStep {
            path: path.to_owned(),
            code: "RPG_ACTIVATION_STAGED".to_owned(),
            detail: format!(
                "activation {accepted_activations} of {ceiling} accepted for {entity_id}"
            ),
        });
        Ok(())
    }

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
            && !matches!(
                self.action.check,
                RpgIrCheck::NoRoll | RpgIrCheck::HeterogeneousPool { .. }
            ) {
            let (kind, sides) = self.check_random_request("$.action.check")?;
            Some(self.take_random(kind, sides, "$.action.check.sharedRoll")?)
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
                    contribution_selector,
                } => {
                    let roll = match shared_roll {
                        Some(value) => value,
                        None => self.take_random(
                            RpgRandomRequestKind::AttackCheck,
                            20,
                            &format!("{path}.roll"),
                        )?,
                    };
                    let base_modifier = self.eval_formula(modifier, &format!("{path}.modifier"))?;
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
                    let resolved = self.resolve_ordered_scalar(
                        ScalarResolutionRequest {
                            profile_id: "legacy.attack",
                            bands: &legacy_attack_bands(),
                            margin_rules: &legacy_attack_margin_rules(),
                            natural_die_rules: &[],
                            contribution_selector_id: contribution_selector
                                .as_ref()
                                .map(|selector| selector.id.as_str()),
                            domain_minimum: i64::from(i32::MIN),
                            domain_maximum: i64::from(i32::MAX),
                            apply_contextual_shifts: false,
                            roll,
                            base_value: base_modifier,
                            difficulty: defense,
                        },
                        &target_id,
                        &path,
                    )?;
                    let hit = resolved.final_band_id == "hit";
                    self.events.push(RpgDomainEvent::AttackResolved {
                        actor_id: self.intent.actor_id.clone(),
                        target_id: target_id.clone(),
                        roll,
                        total: resolved.total,
                        defense_id: defense_id.clone(),
                        defense,
                        hit,
                        contribution_ledger: resolved.contribution_ledger,
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
                    let resolved = self.resolve_ordered_scalar(
                        ScalarResolutionRequest {
                            profile_id: "legacy.save",
                            bands: &legacy_save_bands(),
                            margin_rules: &legacy_save_margin_rules(),
                            natural_die_rules: &[],
                            contribution_selector_id: None,
                            domain_minimum: i64::from(i32::MIN),
                            domain_maximum: i64::from(i32::MAX),
                            apply_contextual_shifts: false,
                            roll,
                            base_value: defense,
                            difficulty,
                        },
                        &target_id,
                        &path,
                    )?;
                    let saved = resolved.final_band_id == "saved";
                    self.events.push(RpgDomainEvent::SavingThrowResolved {
                        target_id: target_id.clone(),
                        roll,
                        total: resolved.total,
                        difficulty,
                        saved,
                    });
                    if saved {
                        CheckOutcome::Saved
                    } else {
                        CheckOutcome::Failed
                    }
                }
                RpgIrCheck::ScalarTest {
                    profile,
                    base,
                    difficulty,
                } => {
                    let profile_definition = self
                        .rules
                        .scalar_test_profile(&profile.id)
                        .cloned()
                        .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_SCALAR_TEST_PROFILE_UNKNOWN",
                            &format!("{path}.profile"),
                            format!("compiled scalar-test profile {} is unavailable", profile.id),
                        )
                    })?;
                    let roll = match shared_roll {
                        Some(value) => value,
                        None => self.take_random(
                            RpgRandomRequestKind::ScalarTest,
                            profile_definition.definition.die_sides,
                            &format!("{path}.roll"),
                        )?,
                    };
                    let base_value = self.eval_formula(base, &format!("{path}.base"))?;
                    let difficulty = match difficulty {
                        RpgIrScalarTestDifficulty::Explicit { value } => {
                            self.eval_formula(value, &format!("{path}.difficulty.value"))?
                        }
                        RpgIrScalarTestDifficulty::TargetDefense { defense_id } => self
                            .workspace
                            .state()
                            .entity(&target_id)
                            .and_then(|target| target.defense(defense_id))
                            .ok_or_else(|| {
                                self.fail(
                                    "RPG_RUNTIME_DEFENSE_MISSING",
                                    &format!("{path}.difficulty.defense"),
                                    format!("target {target_id} has no defense {defense_id}"),
                                )
                            })?,
                    };
                    let resolved = self.resolve_ordered_scalar(
                        ScalarResolutionRequest {
                            profile_id: &profile_definition.definition.id,
                            bands: &profile_definition.definition.bands,
                            margin_rules: &profile_definition.definition.margin_rules,
                            natural_die_rules: &profile_definition.definition.natural_die_rules,
                            contribution_selector_id: profile_definition
                                .definition
                                .contribution_selector_id
                                .as_deref(),
                            domain_minimum: profile_definition.minimum,
                            domain_maximum: profile_definition.maximum,
                            apply_contextual_shifts: true,
                            roll,
                            base_value,
                            difficulty,
                        },
                        &target_id,
                        &path,
                    )?;
                    let final_band_id = resolved.final_band_id.clone();
                    self.events.push(RpgDomainEvent::ScalarTestResolved {
                        actor_id: self.intent.actor_id.clone(),
                        target_id: target_id.clone(),
                        profile_id: profile_definition.definition.id.clone(),
                        roll,
                        base_value,
                        contribution_ledger: resolved.contribution_ledger,
                        difficulty,
                        total: resolved.total,
                        margin: resolved.margin,
                        base_band_id: resolved.base_band_id,
                        natural_die_resolution: resolved.natural_die_resolution,
                        band_shift_ledger: Box::new(resolved.band_shift_ledger),
                        final_band_id: final_band_id.clone(),
                    });
                    CheckOutcome::Scalar {
                        profile_id: profile_definition.definition.id,
                        band_id: final_band_id,
                    }
                }
                RpgIrCheck::HeterogeneousPool {
                    profile,
                    base_dice,
                    automatic_axes,
                } => {
                    let profile_definition = self
                        .rules
                        .heterogeneous_pool_profile(&profile.id)
                        .cloned()
                        .ok_or_else(|| {
                            self.fail(
                                "RPG_RUNTIME_HETEROGENEOUS_POOL_PROFILE_UNKNOWN",
                                &format!("{path}.profile"),
                                format!(
                                    "compiled heterogeneous-pool profile {} is unavailable",
                                    profile.id
                                ),
                            )
                        })?;
                    let final_band_id = self.resolve_heterogeneous_pool(
                        &profile_definition,
                        base_dice,
                        automatic_axes,
                        &target_id,
                        &path,
                    )?;
                    CheckOutcome::Vector {
                        profile_id: profile_definition.id,
                        band_id: final_band_id,
                    }
                }
            };
            self.outcomes.insert(target_id.clone(), outcome.clone());
            self.trace.push(RpgTraceStep {
                path,
                code: "RPG_CHECK_RESOLVED".to_owned(),
                detail: format!("target {target_id} outcome {outcome:?}"),
            });
        }
        self.current_target = None;
        Ok(())
    }

    fn check_random_request(
        &self,
        path: &str,
    ) -> Result<(RpgRandomRequestKind, u32), RpgResolutionRejection> {
        match &self.action.check {
            RpgIrCheck::Attack { .. } => Ok((RpgRandomRequestKind::AttackCheck, 20)),
            RpgIrCheck::SavingThrow { .. } => Ok((RpgRandomRequestKind::SavingThrowCheck, 20)),
            RpgIrCheck::ScalarTest { profile, .. } => self
                .rules
                .scalar_test_profile(&profile.id)
                .map(|compiled| {
                    (
                        RpgRandomRequestKind::ScalarTest,
                        compiled.definition.die_sides,
                    )
                })
                .ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_SCALAR_TEST_PROFILE_UNKNOWN",
                        path,
                        format!("compiled scalar-test profile {} is unavailable", profile.id),
                    )
                }),
            RpgIrCheck::HeterogeneousPool { .. } => Err(self.fail(
                "RPG_RUNTIME_RANDOM_REQUEST_INVALID",
                path,
                "heterogeneous pool checks use an exact typed random request",
            )),
            RpgIrCheck::NoRoll => Err(self.fail(
                "RPG_RUNTIME_RANDOM_REQUEST_INVALID",
                path,
                "a no-roll action cannot request check randomness",
            )),
        }
    }

    fn resolve_heterogeneous_pool(
        &mut self,
        profile: &RulesetHeterogeneousPoolProfile,
        base_terms: &[rpg_ir::RpgIrPoolDieTerm],
        base_automatic_axes: &[rpg_ir::RpgIrPoolAxisValue],
        target_id: &str,
        path: &str,
    ) -> Result<String, RpgResolutionRejection> {
        let die_types = profile
            .die_types
            .iter()
            .map(|die| (die.id.as_str(), die))
            .collect::<BTreeMap<_, _>>();
        let axis_ids = profile
            .axes
            .iter()
            .map(|axis| axis.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut base_dice = BTreeMap::<String, u32>::new();
        for (index, term) in base_terms.iter().enumerate() {
            if !die_types.contains_key(term.die_type_id.as_str()) {
                return Err(self.fail(
                    "RPG_RUNTIME_POOL_DIE_TYPE_UNKNOWN",
                    &format!("{path}.baseDice[{index}].dieTypeId"),
                    format!("unknown pool die type {}", term.die_type_id),
                ));
            }
            base_dice.insert(term.die_type_id.clone(), term.count);
        }
        let mut pending = Vec::<PendingPoolContribution>::new();
        for feature in &self.character_features {
            for contribution in &feature.pool_contributions {
                if contribution.profile.id == profile.id {
                    pending.push(PendingPoolContribution {
                        source_definition_id: feature.definition_id.clone(),
                        source_instance_id: None,
                        source_label: feature.label.clone(),
                        definition: contribution.clone(),
                    });
                }
            }
        }
        if let (Some(item), Some(binding)) = (self.bound_item, self.intent.item_binding.as_ref()) {
            for contribution in &item.pool_contributions {
                if contribution.profile.id == profile.id {
                    pending.push(PendingPoolContribution {
                        source_definition_id: item.definition_id.clone(),
                        source_instance_id: Some(binding.item_instance_id.clone()),
                        source_label: item.label.clone(),
                        definition: contribution.clone(),
                    });
                }
            }
        }
        for (effect, definition) in self.active_effect_sources(target_id, path)? {
            for contribution in definition.pool_contributions {
                if contribution.profile.id == profile.id {
                    pending.push(PendingPoolContribution {
                        source_definition_id: definition.definition_id.clone(),
                        source_instance_id: Some(effect.instance_id().to_owned()),
                        source_label: definition.label.clone(),
                        definition: contribution,
                    });
                }
            }
        }
        pending.sort_by(|left, right| left.canonical_key().cmp(&right.canonical_key()));
        if pending.len() > 256 {
            return Err(self.fail(
                "RPG_RUNTIME_POOL_CONTRIBUTION_LIMIT_EXCEEDED",
                &format!("{path}.contributionLedger"),
                "one pool evaluation may consider at most 256 contributions",
            ));
        }
        for adjacent in pending.windows(2) {
            if adjacent[0].canonical_key() == adjacent[1].canonical_key() {
                return Err(self.fail(
                    "RPG_RUNTIME_POOL_CONTRIBUTION_IDENTITY_DUPLICATE",
                    &format!("{path}.contributionLedger"),
                    format!(
                        "duplicate pool contribution identity {}",
                        adjacent[0].display_key()
                    ),
                ));
            }
        }

        let mut decisions = Vec::<RpgPoolContributionDecision>::with_capacity(pending.len());
        for (index, candidate) in pending.iter().enumerate() {
            let candidate_path = format!("{path}.contributionLedger.candidates[{index}]");
            let inapplicable_reason = self.evaluate_contribution_predicate(
                &candidate.definition.predicate,
                target_id,
                &candidate_path,
            )?;
            decisions.push(RpgPoolContributionDecision {
                source_definition_id: candidate.source_definition_id.clone(),
                source_instance_id: candidate.source_instance_id.clone(),
                source_label: candidate.source_label.clone(),
                contribution_id: candidate.definition.id.clone(),
                profile_id: profile.id.clone(),
                stacking_group_id: candidate.definition.stacking_group.id.clone(),
                effect: candidate.definition.effect.clone(),
                disposition: if let Some(reason) = inapplicable_reason {
                    RpgContributionDisposition::Inapplicable { reason }
                } else {
                    RpgContributionDisposition::Applied
                },
            });
        }
        let mut group_indices = BTreeMap::<String, Vec<usize>>::new();
        for (index, decision) in decisions.iter().enumerate() {
            if matches!(decision.disposition, RpgContributionDisposition::Applied) {
                group_indices
                    .entry(decision.stacking_group_id.clone())
                    .or_default()
                    .push(index);
            }
        }
        for (group_id, indices) in group_indices {
            let policy = self
                .rules
                .contribution_stacking_policy(&group_id)
                .ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_POOL_CONTRIBUTION_GROUP_UNKNOWN",
                        &format!("{path}.contributionLedger"),
                        format!("compiled stacking group {group_id} is unavailable"),
                    )
                })?;
            let retained = retained_pool_contribution_indices(policy, &indices, &decisions);
            let retained_keys = retained
                .iter()
                .map(|index| pool_decision_key(&decisions[*index]))
                .collect::<Vec<_>>();
            let retained_set = retained.into_iter().collect::<BTreeSet<_>>();
            for index in indices {
                if !retained_set.contains(&index) {
                    decisions[index].disposition = RpgContributionDisposition::Suppressed {
                        policy,
                        retained_contribution_ids: retained_keys.clone(),
                    };
                }
            }
        }

        let mut grouped_die_deltas = BTreeMap::<String, i32>::new();
        let mut grouped_axis_values = BTreeMap::<String, i32>::new();
        let mut replacements = Vec::<usize>::new();
        let mut reduction_operations = 0_u32;
        for (index, decision) in decisions.iter().enumerate() {
            if !matches!(decision.disposition, RpgContributionDisposition::Applied) {
                continue;
            }
            match &decision.effect {
                RpgPoolContributionEffect::AddDice { die_type_id, delta } => {
                    if !die_types.contains_key(die_type_id.as_str()) {
                        return Err(self.fail(
                            "RPG_RUNTIME_POOL_DIE_TYPE_UNKNOWN",
                            &format!("{path}.contributionLedger.candidates[{index}].effect"),
                            format!("unknown pool die type {die_type_id}"),
                        ));
                    }
                    let total = grouped_die_deltas.entry(die_type_id.clone()).or_default();
                    *total = total.checked_add(*delta).ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_POOL_DIE_DELTA_OVERFLOW",
                            &format!("{path}.contributionLedger.groupedDieDeltas"),
                            "grouped pool die deltas exceeded the runtime integer domain",
                        )
                    })?;
                    reduction_operations = reduction_operations.saturating_add(1);
                }
                RpgPoolContributionEffect::AddAxis { axis_id, value } => {
                    if !axis_ids.contains(axis_id.as_str()) {
                        return Err(self.fail(
                            "RPG_RUNTIME_POOL_AXIS_UNKNOWN",
                            &format!("{path}.contributionLedger.candidates[{index}].effect"),
                            format!("unknown pool result axis {axis_id}"),
                        ));
                    }
                    let total = grouped_axis_values.entry(axis_id.clone()).or_default();
                    *total = total.checked_add(*value).ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_POOL_AXIS_OVERFLOW",
                            &format!("{path}.contributionLedger.groupedAxisValues"),
                            "grouped automatic axis values exceeded the runtime integer domain",
                        )
                    })?;
                    reduction_operations = reduction_operations.saturating_add(1);
                }
                RpgPoolContributionEffect::ReplaceOrAddDie {
                    from_die_type_id,
                    to_die_type_id,
                    count,
                    fallback_die_type_id,
                } => {
                    if from_die_type_id == to_die_type_id
                        || *count == 0
                        || !die_types.contains_key(from_die_type_id.as_str())
                        || !die_types.contains_key(to_die_type_id.as_str())
                        || !die_types.contains_key(fallback_die_type_id.as_str())
                    {
                        return Err(self.fail(
                            "RPG_RUNTIME_POOL_REPLACEMENT_INVALID",
                            &format!("{path}.contributionLedger.candidates[{index}].effect"),
                            "pool replacement requires known ids, positive count, and distinct from/to ids",
                        ));
                    }
                    reduction_operations = reduction_operations.saturating_add(*count);
                    replacements.push(index);
                }
            }
        }
        if reduction_operations > 128 {
            return Err(self.fail(
                "RPG_RUNTIME_POOL_REDUCTION_LIMIT_EXCEEDED",
                &format!("{path}.contributionLedger"),
                "pool reduction requires more than 128 operations",
            ));
        }

        let mut frozen_dice = base_dice.clone();
        for (die_type_id, delta) in &grouped_die_deltas {
            let previous = i64::from(*frozen_dice.get(die_type_id).unwrap_or(&0));
            let current = previous.checked_add(i64::from(*delta)).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_POOL_DIE_COUNT_OVERFLOW",
                    &format!("{path}.contributionLedger.groupedDieDeltas.{die_type_id}"),
                    "pool die count exceeded the runtime integer domain",
                )
            })?;
            if !(0..=256).contains(&current) {
                return Err(self.fail(
                    "RPG_RUNTIME_POOL_DIE_COUNT_INVALID",
                    &format!("{path}.contributionLedger.groupedDieDeltas.{die_type_id}"),
                    format!("pool die count {current} is outside 0..=256"),
                ));
            }
            frozen_dice.insert(die_type_id.clone(), current as u32);
        }
        ensure_pool_total_bound(&frozen_dice, path, self)?;
        let mut replacement_units = Vec::new();
        for decision_index in replacements {
            let decision = &decisions[decision_index];
            let RpgPoolContributionEffect::ReplaceOrAddDie {
                from_die_type_id,
                to_die_type_id,
                count,
                fallback_die_type_id,
            } = &decision.effect
            else {
                unreachable!("replacement indices contain only replacement effects");
            };
            for unit in 1..=*count {
                let before_from_count = *frozen_dice.get(from_die_type_id).unwrap_or(&0);
                let used_fallback = before_from_count == 0;
                let added_die_type_id = if used_fallback {
                    fallback_die_type_id
                } else {
                    frozen_dice.insert(from_die_type_id.clone(), before_from_count - 1);
                    to_die_type_id
                };
                let after_added_count = frozen_dice
                    .get(added_die_type_id)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_POOL_DIE_COUNT_OVERFLOW",
                            &format!("{path}.contributionLedger.replacementUnits"),
                            "pool replacement exceeded the die-count domain",
                        )
                    })?;
                frozen_dice.insert(added_die_type_id.clone(), after_added_count);
                ensure_pool_total_bound(&frozen_dice, path, self)?;
                let after_from_count = *frozen_dice.get(from_die_type_id).unwrap_or(&0);
                replacement_units.push(RpgPoolReplacementUnit {
                    contribution_id: pool_decision_key(decision),
                    unit,
                    from_die_type_id: from_die_type_id.clone(),
                    added_die_type_id: added_die_type_id.clone(),
                    used_fallback,
                    before_from_count,
                    after_from_count,
                    after_added_count,
                });
                self.trace.push(RpgTraceStep {
                    path: format!(
                        "{path}.contributionLedger.replacementUnits[{}]",
                        replacement_units.len() - 1
                    ),
                    code: "RPG_POOL_REPLACEMENT_UNIT_APPLIED".to_owned(),
                    detail: format!(
                        "{} unit {unit}: from {from_die_type_id} {before_from_count}->{after_from_count}, added {added_die_type_id} -> {after_added_count}, fallback {used_fallback}",
                        pool_decision_key(decision)
                    ),
                });
            }
        }
        frozen_dice.retain(|_, count| *count > 0);
        let request_terms = frozen_dice
            .iter()
            .map(|(die_type_id, count)| RpgHeterogeneousRandomTerm {
                die_type_id: die_type_id.clone(),
                count: *count,
                sides: die_types
                    .get(die_type_id.as_str())
                    .expect("validated frozen die type exists")
                    .sides,
            })
            .collect::<Vec<_>>();
        let (request, values, typed_values) =
            self.take_heterogeneous_random(&request_terms, &format!("{path}.pool"))?;

        let mut raw_axes = profile
            .axes
            .iter()
            .map(|axis| (axis.id.clone(), 0_i32))
            .collect::<BTreeMap<_, _>>();
        for typed in &typed_values {
            let die = die_types
                .get(typed.die_type_id.as_str())
                .expect("validated random die type exists");
            let face = die
                .faces
                .get((typed.value - 1) as usize)
                .expect("validated complete face table contains drawn face");
            for axis in &face.vector {
                let total = raw_axes.get_mut(&axis.axis_id).ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_POOL_AXIS_UNKNOWN",
                        &format!("{path}.pool.faceVector"),
                        format!("face table references unknown axis {}", axis.axis_id),
                    )
                })?;
                *total = total.checked_add(axis.value).ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_POOL_AXIS_OVERFLOW",
                        &format!("{path}.pool.rawAxes.{}", axis.axis_id),
                        "face-vector aggregation exceeded the runtime integer domain",
                    )
                })?;
            }
        }
        let mut automatic_axes = BTreeMap::<String, i32>::new();
        for (index, axis) in base_automatic_axes.iter().enumerate() {
            if !axis_ids.contains(axis.axis_id.as_str()) {
                return Err(self.fail(
                    "RPG_RUNTIME_POOL_AXIS_UNKNOWN",
                    &format!("{path}.automaticAxes[{index}].axisId"),
                    format!("unknown pool result axis {}", axis.axis_id),
                ));
            }
            automatic_axes.insert(axis.axis_id.clone(), axis.value);
        }
        for (axis_id, value) in &grouped_axis_values {
            let total = automatic_axes.entry(axis_id.clone()).or_default();
            *total = total.checked_add(*value).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_POOL_AXIS_OVERFLOW",
                    &format!("{path}.pool.automaticAxes.{axis_id}"),
                    "automatic axis aggregation exceeded the runtime integer domain",
                )
            })?;
        }
        let mut net_axes = raw_axes.clone();
        for (axis_id, value) in &automatic_axes {
            let total = net_axes.get_mut(axis_id).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_POOL_AXIS_UNKNOWN",
                    &format!("{path}.pool.automaticAxes.{axis_id}"),
                    format!("unknown pool result axis {axis_id}"),
                )
            })?;
            *total = total.checked_add(*value).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_POOL_AXIS_OVERFLOW",
                    &format!("{path}.pool.netAxes.{axis_id}"),
                    "raw and automatic axes exceeded the runtime integer domain",
                )
            })?;
        }
        let mut cancellations = Vec::with_capacity(profile.cancellations.len());
        for cancellation in &profile.cancellations {
            let positive = *net_axes
                .get(&cancellation.positive_axis_id)
                .expect("validated cancellation positive axis exists");
            let negative = *net_axes
                .get(&cancellation.negative_axis_id)
                .expect("validated cancellation negative axis exists");
            if positive < 0 || negative < 0 {
                return Err(self.fail(
                    "RPG_RUNTIME_POOL_CANCELLATION_AXIS_NEGATIVE",
                    &format!("{path}.pool.cancellations.{}", cancellation.id),
                    "paired cancellation axes must be non-negative before cancellation",
                ));
            }
            let cancelled = positive.min(negative);
            let positive_remaining = positive - cancelled;
            let negative_remaining = negative - cancelled;
            net_axes.insert(cancellation.positive_axis_id.clone(), positive_remaining);
            net_axes.insert(cancellation.negative_axis_id.clone(), negative_remaining);
            cancellations.push(RpgPoolCancellationResult {
                cancellation_id: cancellation.id.clone(),
                positive_axis_id: cancellation.positive_axis_id.clone(),
                negative_axis_id: cancellation.negative_axis_id.clone(),
                cancelled,
                positive_remaining,
                negative_remaining,
            });
        }
        let final_band_id = profile
            .outcome_rules
            .iter()
            .find(|rule| {
                rule.requirements.iter().all(|requirement| {
                    let value = net_axes.get(&requirement.axis_id).copied().unwrap_or(0);
                    requirement.minimum.is_none_or(|minimum| value >= minimum)
                        && requirement.maximum.is_none_or(|maximum| value <= maximum)
                })
            })
            .map_or_else(
                || profile.default_band_id.clone(),
                |rule| rule.band_id.clone(),
            );
        let contribution_ledger = RpgPoolContributionLedger {
            profile_id: profile.id.clone(),
            candidates: decisions,
            grouped_die_deltas,
            grouped_axis_values,
            replacement_units,
        };
        for (index, decision) in contribution_ledger.candidates.iter().enumerate() {
            self.trace.push(RpgTraceStep {
                path: format!("{path}.contributionLedger.candidates[{index}]"),
                code: "RPG_POOL_CONTRIBUTION_EVALUATED".to_owned(),
                detail: format!(
                    "{} effect {:?} status {:?}",
                    pool_decision_key(decision),
                    decision.effect,
                    decision.disposition
                ),
            });
        }
        self.trace.push(RpgTraceStep {
            path: format!("{path}.pool"),
            code: "RPG_HETEROGENEOUS_POOL_RESOLVED".to_owned(),
            detail: format!(
                "profile {} request {:?} net {:?} band {}",
                profile.id, request.heterogeneous_terms, net_axes, final_band_id
            ),
        });
        self.events.push(RpgDomainEvent::HeterogeneousPoolResolved {
            actor_id: self.intent.actor_id.clone(),
            target_id: target_id.to_owned(),
            profile_id: profile.id.clone(),
            base_dice,
            contribution_ledger: Box::new(contribution_ledger),
            frozen_dice,
            evidence: typed_values,
            raw_axes,
            automatic_axes,
            cancellations,
            net_axes,
            final_band_id: final_band_id.clone(),
        });
        debug_assert_eq!(request.count as usize, values.len());
        Ok(final_band_id)
    }

    fn take_heterogeneous_random(
        &mut self,
        terms: &[RpgHeterogeneousRandomTerm],
        path: &str,
    ) -> Result<
        (RpgRandomRequest, Vec<u32>, Vec<RpgHeterogeneousRandomValue>),
        RpgResolutionRejection,
    > {
        let count = terms.iter().try_fold(0_u32, |total, term| {
            total.checked_add(term.count).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_POOL_DIE_COUNT_OVERFLOW",
                    path,
                    "heterogeneous request count exceeded the runtime integer domain",
                )
            })
        })?;
        if count == 0 || count > 256 {
            return Err(self.fail(
                "RPG_RUNTIME_POOL_DIE_COUNT_INVALID",
                path,
                "a heterogeneous request requires 1..=256 total dice",
            ));
        }
        let request = RpgRandomRequest {
            kind: RpgRandomRequestKind::HeterogeneousPool,
            count,
            sides: 0,
            path: path.to_owned(),
            heterogeneous_terms: terms.to_vec(),
        };
        let available = u32::try_from(self.workspace.random_remaining()).unwrap_or(u32::MAX);
        if available < count {
            if available != 0 {
                return Err(self.fail(
                    "RPG_RANDOM_HETEROGENEOUS_EVIDENCE_PARTIAL",
                    path,
                    "heterogeneous evidence must be supplied as one exact complete request",
                ));
            }
            let mut rejection = self.fail(
                "RPG_RANDOM_EXHAUSTED",
                path,
                "deterministic random stream is exhausted",
            );
            rejection.random_request = Some(Box::new(request));
            return Err(rejection);
        }
        let mut values = Vec::with_capacity(count as usize);
        let mut typed_values = Vec::with_capacity(count as usize);
        for term in terms {
            for ordinal in 1..=term.count {
                let value = self.workspace.random_owner().take().ok_or_else(|| {
                    self.fail(
                        "RPG_RANDOM_HETEROGENEOUS_EVIDENCE_PARTIAL",
                        path,
                        "heterogeneous evidence ended inside an exact request",
                    )
                })?;
                if value == 0 || value > term.sides {
                    return Err(self.fail(
                        "RPG_RANDOM_VALUE_OUT_OF_RANGE",
                        path,
                        format!(
                            "{} ordinal {ordinal} value {value} is outside 1..={}",
                            term.die_type_id, term.sides
                        ),
                    ));
                }
                values.push(value);
                typed_values.push(RpgHeterogeneousRandomValue {
                    die_type_id: term.die_type_id.clone(),
                    ordinal,
                    sides: term.sides,
                    value,
                });
                self.trace.push(RpgTraceStep {
                    path: format!("{path}.{}.{}", term.die_type_id, ordinal),
                    code: "RPG_RANDOM_CONSUMED".to_owned(),
                    detail: format!("{} d{}={value}", term.die_type_id, term.sides),
                });
            }
        }
        self.random_evidence.push(RpgRandomEvidence {
            request: request.clone(),
            values: values.clone(),
            heterogeneous_values: typed_values.clone(),
        });
        Ok((request, values, typed_values))
    }

    fn resolve_ordered_scalar(
        &mut self,
        request: ScalarResolutionRequest<'_>,
        target_id: &str,
        path: &str,
    ) -> Result<ResolvedScalar, RpgResolutionRejection> {
        ensure_scalar_domain(
            request.base_value,
            request.domain_minimum,
            request.domain_maximum,
            &format!("{path}.baseValue"),
            self,
        )?;
        ensure_scalar_domain(
            request.difficulty,
            request.domain_minimum,
            request.domain_maximum,
            &format!("{path}.difficulty"),
            self,
        )?;
        let contribution_ledger = self.evaluate_contribution_ledger(
            request.contribution_selector_id,
            request.base_value,
            target_id,
            &format!("{path}.contributionLedger"),
        )?;
        let total = i32::try_from(request.roll)
            .unwrap_or(i32::MAX)
            .checked_add(contribution_ledger.final_value)
            .ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_ROLL_TOTAL_OVERFLOW",
                    &format!("{path}.total"),
                    "roll and resolved scalar base exceeded the runtime integer domain",
                )
            })?;
        let margin = total.checked_sub(request.difficulty).ok_or_else(|| {
            self.fail(
                "RPG_RUNTIME_SCALAR_MARGIN_OVERFLOW",
                &format!("{path}.margin"),
                "scalar-test total minus difficulty exceeded the runtime integer domain",
            )
        })?;
        let base_band_id = request
            .margin_rules
            .iter()
            .find(|rule| {
                rule.minimum
                    .is_none_or(|minimum| i64::from(margin) >= minimum)
                    && rule
                        .maximum
                        .is_none_or(|maximum| i64::from(margin) <= maximum)
            })
            .map(|rule| rule.band_id.clone())
            .ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_SCALAR_MARGIN_UNCLASSIFIED",
                    &format!("{path}.margin"),
                    format!("margin {margin} has no outcome band"),
                )
            })?;
        let base_index = band_index(request.bands, &base_band_id).ok_or_else(|| {
            self.fail(
                "RPG_RUNTIME_SCALAR_BAND_UNKNOWN",
                &format!("{path}.baseBandId"),
                format!("margin selected unknown band {base_band_id}"),
            )
        })?;
        let natural_rule = request
            .natural_die_rules
            .iter()
            .find(|rule| request.roll >= rule.minimum && request.roll <= rule.maximum);
        let (natural_index, natural_die_resolution) = if let Some(rule) = natural_rule {
            let resulting_index = match &rule.effect {
                RpgNaturalDieEffect::SetBand { band_id } => band_index(request.bands, band_id)
                    .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_SCALAR_BAND_UNKNOWN",
                            &format!("{path}.naturalDieRule"),
                            format!("natural-die rule selected unknown band {band_id}"),
                        )
                    })?,
                RpgNaturalDieEffect::Shift { amount } => {
                    shifted_band_index(base_index, *amount, request.bands.len())
                }
            };
            (
                resulting_index,
                Some(RpgNaturalDieResolution {
                    rule_id: rule.id.clone(),
                    effect: rule.effect.clone(),
                    resulting_band_id: request.bands[resulting_index].id.clone(),
                }),
            )
        } else {
            (base_index, None)
        };
        let (band_shift_ledger, final_index) = if request.apply_contextual_shifts {
            self.evaluate_outcome_band_shift_ledger(
                request.profile_id,
                target_id,
                request.bands,
                natural_index,
                &format!("{path}.bandShiftLedger"),
            )?
        } else {
            (
                RpgOutcomeBandShiftLedger {
                    profile_id: request.profile_id.to_owned(),
                    starting_band_id: request.bands[natural_index].id.clone(),
                    candidates: Vec::new(),
                    total_shift: 0,
                    final_band_id: request.bands[natural_index].id.clone(),
                },
                natural_index,
            )
        };
        Ok(ResolvedScalar {
            contribution_ledger,
            total,
            margin,
            base_band_id,
            natural_die_resolution,
            band_shift_ledger,
            final_band_id: request.bands[final_index].id.clone(),
        })
    }

    fn evaluate_outcome_band_shift_ledger(
        &mut self,
        profile_id: &str,
        target_id: &str,
        bands: &[RulesetOutcomeBand],
        starting_index: usize,
        path: &str,
    ) -> Result<(RpgOutcomeBandShiftLedger, usize), RpgResolutionRejection> {
        let mut pending = Vec::<PendingOutcomeBandShift>::new();
        for feature in &self.character_features {
            for shift in &feature.outcome_band_shifts {
                if shift.profile.id == profile_id {
                    pending.push(PendingOutcomeBandShift {
                        source_definition_id: feature.definition_id.clone(),
                        source_instance_id: None,
                        source_label: feature.label.clone(),
                        definition: shift.clone(),
                    });
                }
            }
        }
        if let (Some(item), Some(binding)) = (self.bound_item, self.intent.item_binding.as_ref()) {
            for shift in &item.outcome_band_shifts {
                if shift.profile.id == profile_id {
                    pending.push(PendingOutcomeBandShift {
                        source_definition_id: item.definition_id.clone(),
                        source_instance_id: Some(binding.item_instance_id.clone()),
                        source_label: item.label.clone(),
                        definition: shift.clone(),
                    });
                }
            }
        }
        for (effect, definition) in self.active_effect_sources(target_id, path)? {
            for shift in definition.outcome_band_shifts {
                if shift.profile.id == profile_id {
                    pending.push(PendingOutcomeBandShift {
                        source_definition_id: definition.definition_id.clone(),
                        source_instance_id: Some(effect.instance_id().to_owned()),
                        source_label: definition.label.clone(),
                        definition: shift,
                    });
                }
            }
        }
        pending.sort_by(|left, right| left.canonical_key().cmp(&right.canonical_key()));
        if pending.len() > 256 {
            return Err(self.fail(
                "RPG_RUNTIME_OUTCOME_BAND_SHIFT_LIMIT_EXCEEDED",
                path,
                "one scalar evaluation may consider at most 256 outcome-band shifts",
            ));
        }
        for adjacent in pending.windows(2) {
            if adjacent[0].canonical_key() == adjacent[1].canonical_key() {
                return Err(self.fail(
                    "RPG_RUNTIME_OUTCOME_BAND_SHIFT_IDENTITY_DUPLICATE",
                    path,
                    format!(
                        "duplicate outcome-band shift identity {}",
                        adjacent[0].display_key()
                    ),
                ));
            }
        }
        let mut candidates = Vec::with_capacity(pending.len());
        let mut total_shift = 0_i32;
        let mut current_index = starting_index;
        for (index, candidate) in pending.iter().enumerate() {
            let candidate_path = format!("{path}.candidates[{index}]");
            let inapplicable_reason = self.evaluate_contribution_predicate(
                &candidate.definition.predicate,
                target_id,
                &candidate_path,
            )?;
            let applied_shift = if inapplicable_reason.is_none() {
                candidate.definition.shift
            } else {
                0
            };
            total_shift = total_shift.checked_add(applied_shift).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_OUTCOME_BAND_SHIFT_OVERFLOW",
                    path,
                    "outcome-band shifts exceeded the runtime integer domain",
                )
            })?;
            current_index = shifted_band_index(current_index, applied_shift, bands.len());
            let disposition = if let Some(reason) = inapplicable_reason {
                RpgOutcomeBandShiftDisposition::Inapplicable { reason }
            } else {
                RpgOutcomeBandShiftDisposition::Applied
            };
            let decision = RpgOutcomeBandShiftDecision {
                source_definition_id: candidate.source_definition_id.clone(),
                source_instance_id: candidate.source_instance_id.clone(),
                source_label: candidate.source_label.clone(),
                shift_id: candidate.definition.id.clone(),
                profile_id: profile_id.to_owned(),
                declared_shift: candidate.definition.shift,
                applied_shift,
                disposition,
                resulting_band_id: bands[current_index].id.clone(),
            };
            self.trace.push(RpgTraceStep {
                path: candidate_path,
                code: "RPG_OUTCOME_BAND_SHIFT_EVALUATED".to_owned(),
                detail: format!(
                    "{} declared {} applied {} status {:?} resulting band {}",
                    candidate.display_key(),
                    decision.declared_shift,
                    decision.applied_shift,
                    decision.disposition,
                    decision.resulting_band_id,
                ),
            });
            candidates.push(decision);
        }
        self.trace.push(RpgTraceStep {
            path: path.to_owned(),
            code: "RPG_OUTCOME_BAND_SHIFT_LEDGER_RESOLVED".to_owned(),
            detail: format!("profile {profile_id} total shift {total_shift}"),
        });
        Ok((
            RpgOutcomeBandShiftLedger {
                profile_id: profile_id.to_owned(),
                starting_band_id: bands[starting_index].id.clone(),
                candidates,
                total_shift,
                final_band_id: bands[current_index].id.clone(),
            },
            current_index,
        ))
    }

    fn evaluate_contribution_ledger(
        &mut self,
        selector_id: Option<&str>,
        base_value: i32,
        target_id: &str,
        path: &str,
    ) -> Result<RpgScalarContributionLedger, RpgResolutionRejection> {
        let Some(selector_id) = selector_id else {
            return Ok(RpgScalarContributionLedger {
                selector_id: "action.base-modifier".to_owned(),
                base_value,
                candidates: Vec::new(),
                final_value: base_value,
            });
        };
        let selector = self
            .rules
            .calculation_selector(selector_id)
            .ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_CONTRIBUTION_SELECTOR_UNKNOWN",
                    path,
                    format!("compiled selector {selector_id} is unavailable"),
                )
            })?;

        let mut pending = Vec::<PendingContribution>::new();
        for feature in &self.character_features {
            for contribution in &feature.contributions {
                if contribution.selector.id == selector_id {
                    pending.push(PendingContribution {
                        source_definition_id: feature.definition_id.clone(),
                        source_instance_id: None,
                        source_label: feature.label.clone(),
                        definition: contribution.clone(),
                    });
                }
            }
        }
        if let (Some(item), Some(binding)) = (self.bound_item, self.intent.item_binding.as_ref()) {
            for contribution in &item.contributions {
                if contribution.selector.id == selector_id {
                    pending.push(PendingContribution {
                        source_definition_id: item.definition_id.clone(),
                        source_instance_id: Some(binding.item_instance_id.clone()),
                        source_label: item.label.clone(),
                        definition: contribution.clone(),
                    });
                }
            }
        }
        for (effect, definition) in self.active_effect_sources(target_id, path)? {
            for contribution in definition.contributions {
                if contribution.selector.id == selector_id {
                    pending.push(PendingContribution {
                        source_definition_id: definition.definition_id.clone(),
                        source_instance_id: Some(effect.instance_id().to_owned()),
                        source_label: definition.label.clone(),
                        definition: contribution,
                    });
                }
            }
        }
        pending.sort_by(|left, right| left.canonical_key().cmp(&right.canonical_key()));
        if pending.len() > 256 {
            return Err(self.fail(
                "RPG_RUNTIME_CONTRIBUTION_LIMIT_EXCEEDED",
                path,
                "one scalar evaluation may consider at most 256 contributions",
            ));
        }
        for adjacent in pending.windows(2) {
            if adjacent[0].canonical_key() == adjacent[1].canonical_key() {
                return Err(self.fail(
                    "RPG_RUNTIME_CONTRIBUTION_IDENTITY_DUPLICATE",
                    path,
                    format!(
                        "duplicate contribution identity {}",
                        adjacent[0].display_key()
                    ),
                ));
            }
        }

        let mut decisions = Vec::with_capacity(pending.len());
        for (index, candidate) in pending.iter().enumerate() {
            let candidate_path = format!("{path}.candidates[{index}]");
            let declared_value = self.evaluate_contribution_value(
                &candidate.definition.value,
                target_id,
                &candidate_path,
            )?;
            let inapplicable_reason = self.evaluate_contribution_predicate(
                &candidate.definition.predicate,
                target_id,
                &candidate_path,
            )?;
            decisions.push(RpgScalarContributionDecision {
                source_definition_id: candidate.source_definition_id.clone(),
                source_instance_id: candidate.source_instance_id.clone(),
                source_label: candidate.source_label.clone(),
                contribution_id: candidate.definition.id.clone(),
                selector_id: selector_id.to_owned(),
                stacking_group_id: candidate.definition.stacking_group.id.clone(),
                declared_value,
                applied_value: 0,
                disposition: if let Some(reason) = inapplicable_reason {
                    RpgContributionDisposition::Inapplicable { reason }
                } else {
                    RpgContributionDisposition::Applied
                },
            });
        }

        let mut group_indices = BTreeMap::<String, Vec<usize>>::new();
        for (index, decision) in decisions.iter().enumerate() {
            if matches!(decision.disposition, RpgContributionDisposition::Applied) {
                group_indices
                    .entry(decision.stacking_group_id.clone())
                    .or_default()
                    .push(index);
            }
        }
        for (group_id, indices) in group_indices {
            let policy = self
                .rules
                .contribution_stacking_policy(&group_id)
                .ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_CONTRIBUTION_GROUP_UNKNOWN",
                        path,
                        format!("compiled stacking group {group_id} is unavailable"),
                    )
                })?;
            let retained = retained_contribution_indices(policy, &indices, &decisions);
            let retained_keys = retained
                .iter()
                .map(|index| decision_key(&decisions[*index]))
                .collect::<Vec<_>>();
            let retained_set = retained.into_iter().collect::<BTreeSet<_>>();
            for index in indices {
                if retained_set.contains(&index) {
                    decisions[index].applied_value = decisions[index].declared_value;
                } else {
                    decisions[index].disposition = RpgContributionDisposition::Suppressed {
                        policy,
                        retained_contribution_ids: retained_keys.clone(),
                    };
                }
            }
        }

        let final_value = decisions.iter().try_fold(base_value, |total, decision| {
            total.checked_add(decision.applied_value).ok_or_else(|| {
                self.fail(
                    "RPG_RUNTIME_CONTRIBUTION_TOTAL_OVERFLOW",
                    path,
                    "scalar contribution total exceeded the runtime integer domain",
                )
            })
        })?;
        let final_value_i64 = i64::from(final_value);
        if final_value_i64 < selector.minimum || final_value_i64 > selector.maximum {
            return Err(self.fail(
                "RPG_RUNTIME_CONTRIBUTION_DOMAIN_EXCEEDED",
                &format!("{path}.finalValue"),
                format!(
                    "resolved value {final_value} is outside selector {selector_id} domain {}..={}",
                    selector.minimum, selector.maximum
                ),
            ));
        }
        for (index, decision) in decisions.iter().enumerate() {
            self.trace.push(RpgTraceStep {
                path: format!("{path}.candidates[{index}]"),
                code: "RPG_CONTRIBUTION_EVALUATED".to_owned(),
                detail: format!(
                    "{} declared {} applied {} status {:?}",
                    decision_key(decision),
                    decision.declared_value,
                    decision.applied_value,
                    decision.disposition
                ),
            });
        }
        self.trace.push(RpgTraceStep {
            path: path.to_owned(),
            code: "RPG_CONTRIBUTION_LEDGER_RESOLVED".to_owned(),
            detail: format!("selector {selector_id} base {base_value} final {final_value}"),
        });
        Ok(RpgScalarContributionLedger {
            selector_id: selector_id.to_owned(),
            base_value,
            candidates: decisions,
            final_value,
        })
    }

    fn evaluate_contribution_value(
        &self,
        expression: &RpgContributionValueExpression,
        target_id: &str,
        path: &str,
    ) -> Result<i32, RpgResolutionRejection> {
        let value = self.evaluate_contribution_value_i64(expression, target_id, path)?;
        i32::try_from(value).map_err(|_| {
            self.fail(
                "RPG_RUNTIME_CONTRIBUTION_VALUE_OVERFLOW",
                path,
                format!("contribution value {value} does not fit the runtime integer domain"),
            )
        })
    }

    fn evaluate_contribution_value_i64(
        &self,
        expression: &RpgContributionValueExpression,
        target_id: &str,
        path: &str,
    ) -> Result<i64, RpgResolutionRejection> {
        match expression {
            RpgContributionValueExpression::Constant { value } => Ok(*value),
            RpgContributionValueExpression::ReadValue {
                subject,
                value_kind,
                value_id,
                ..
            } => Ok(i64::from(self.read_contribution_value(
                *subject,
                *value_kind,
                value_id,
                target_id,
                path,
            )?)),
            RpgContributionValueExpression::Add { terms } => {
                let mut total = 0_i64;
                for (index, term) in terms.iter().enumerate() {
                    let value = self.evaluate_contribution_value_i64(
                        term,
                        target_id,
                        &format!("{path}.terms[{index}]"),
                    )?;
                    total = total.checked_add(value).ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_CONTRIBUTION_VALUE_OVERFLOW",
                            path,
                            "contribution addition exceeded the i64 domain",
                        )
                    })?;
                }
                Ok(total)
            }
            RpgContributionValueExpression::Subtract {
                minuend,
                subtrahend,
            } => {
                let left = self.evaluate_contribution_value_i64(
                    minuend,
                    target_id,
                    &format!("{path}.minuend"),
                )?;
                let right = self.evaluate_contribution_value_i64(
                    subtrahend,
                    target_id,
                    &format!("{path}.subtrahend"),
                )?;
                left.checked_sub(right).ok_or_else(|| {
                    self.fail(
                        "RPG_RUNTIME_CONTRIBUTION_VALUE_OVERFLOW",
                        path,
                        "contribution subtraction exceeded the i64 domain",
                    )
                })
            }
        }
    }

    fn evaluate_contribution_predicate(
        &self,
        predicate: &RpgContributionPredicate,
        target_id: &str,
        path: &str,
    ) -> Result<Option<String>, RpgResolutionRejection> {
        match predicate {
            RpgContributionPredicate::Always => Ok(None),
            RpgContributionPredicate::Not { predicate } => {
                let inner = self.evaluate_contribution_predicate(
                    predicate,
                    target_id,
                    &format!("{path}.not"),
                )?;
                Ok(inner.is_none().then(|| "not.innerMatched".to_owned()))
            }
            RpgContributionPredicate::All { predicates } => {
                for (index, predicate) in predicates.iter().enumerate() {
                    if let Some(reason) = self.evaluate_contribution_predicate(
                        predicate,
                        target_id,
                        &format!("{path}.all[{index}]"),
                    )? {
                        return Ok(Some(format!("all[{index}].{reason}")));
                    }
                }
                Ok(None)
            }
            RpgContributionPredicate::Any { predicates } => {
                let mut reasons = Vec::with_capacity(predicates.len());
                for (index, predicate) in predicates.iter().enumerate() {
                    let outcome = self.evaluate_contribution_predicate(
                        predicate,
                        target_id,
                        &format!("{path}.any[{index}]"),
                    )?;
                    match outcome {
                        None => return Ok(None),
                        Some(reason) => reasons.push(format!("{index}:{reason}")),
                    }
                }
                Ok(Some(format!("any.noneMatched({})", reasons.join(","))))
            }
            RpgContributionPredicate::ActorIsTarget { expected } => {
                Ok(((self.intent.actor_id == target_id) != *expected)
                    .then(|| format!("actorIsTarget.expected.{expected}")))
            }
            RpgContributionPredicate::TeamRelation { relation } => {
                let actor =
                    self.contribution_entity(RpgContributionSubject::Actor, target_id, path)?;
                let target =
                    self.contribution_entity(RpgContributionSubject::Target, target_id, path)?;
                let applies = match relation {
                    RpgContributionTeamRelation::Same => actor.team() == target.team(),
                    RpgContributionTeamRelation::Different => actor.team() != target.team(),
                };
                Ok((!applies)
                    .then(|| format!("teamRelation.required.{}", team_relation_name(*relation))))
            }
            RpgContributionPredicate::Living { subject, expected } => {
                let entity = self.contribution_entity(*subject, target_id, path)?;
                Ok(((entity.vitality().current > 0) != *expected).then(|| {
                    format!(
                        "living.{}.expected.{expected}",
                        contribution_subject_name(*subject)
                    )
                }))
            }
            RpgContributionPredicate::NamedValue {
                subject,
                value_kind,
                value_id,
                comparison,
                value,
                ..
            } => {
                let actual = i64::from(self.read_contribution_value(
                    *subject,
                    *value_kind,
                    value_id,
                    target_id,
                    path,
                )?);
                Ok((!compare_i64(actual, *comparison, *value)).then(|| {
                    format!(
                        "namedValue.{}.{}.{value_id}.{}.{value}",
                        contribution_subject_name(*subject),
                        ruleset_value_kind_name(*value_kind),
                        contribution_comparison_name(*comparison),
                    )
                }))
            }
            RpgContributionPredicate::Distance { comparison, value } => {
                let actor =
                    self.contribution_entity(RpgContributionSubject::Actor, target_id, path)?;
                let target =
                    self.contribution_entity(RpgContributionSubject::Target, target_id, path)?;
                let distance = i64::from(cardinal_distance(actor.position(), target.position()));
                Ok(
                    (!compare_i64(distance, *comparison, i64::from(*value))).then(|| {
                        format!(
                            "distance.{}.{value}.actual.{distance}",
                            contribution_comparison_name(*comparison)
                        )
                    }),
                )
            }
            RpgContributionPredicate::ActorFlanksTarget => Ok((!self
                .actor_flanks_target(target_id))
            .then(|| "actorFlanksTarget.false".to_owned())),
            RpgContributionPredicate::ActorSurrounded { minimum_hostiles } => {
                let actual = self.actor_adjacent_living_hostile_count();
                Ok((actual < *minimum_hostiles)
                    .then(|| format!("actorSurrounded.minimum.{minimum_hostiles}.actual.{actual}")))
            }
            RpgContributionPredicate::BoundItemDefinition { definition_id } => {
                let applies = self
                    .intent
                    .item_binding
                    .as_ref()
                    .is_some_and(|binding| binding.item_definition_id == *definition_id);
                Ok((!applies).then(|| format!("boundItemDefinition.required.{definition_id}")))
            }
            RpgContributionPredicate::BoundItemTag { tag } => {
                let applies = self
                    .bound_item
                    .is_some_and(|item| item.tags.binary_search(tag).is_ok());
                Ok((!applies).then(|| format!("boundItemTag.required.{tag}")))
            }
            RpgContributionPredicate::ActionTag { tag } => Ok(self
                .action
                .tags
                .binary_search(tag)
                .is_err()
                .then(|| format!("actionTag.required.{tag}"))),
            RpgContributionPredicate::CellCapability {
                subject,
                capability_id,
            } => {
                let entity_id = self.contribution_subject_id(*subject, target_id);
                let applies = self
                    .context
                    .entity_cell_capability_ids
                    .get(entity_id)
                    .is_some_and(|ids| ids.binary_search(capability_id).is_ok());
                Ok((!applies).then(|| {
                    format!(
                        "cellCapability.{}.required.{capability_id}",
                        contribution_subject_name(*subject)
                    )
                }))
            }
            RpgContributionPredicate::EffectActive {
                subject,
                definition_id,
            } => {
                let entity = self.contribution_entity(*subject, target_id, path)?;
                let applies = entity.has_effect_definition(definition_id);
                Ok((!applies).then(|| {
                    format!(
                        "effectActive.{}.required.{definition_id}",
                        contribution_subject_name(*subject)
                    )
                }))
            }
        }
    }

    fn read_contribution_value(
        &self,
        subject: RpgContributionSubject,
        value_kind: RpgRulesetValueKind,
        value_id: &str,
        target_id: &str,
        path: &str,
    ) -> Result<i32, RpgResolutionRejection> {
        let entity = self.contribution_entity(subject, target_id, path)?;
        let value = match value_kind {
            RpgRulesetValueKind::Defense => entity.defense(value_id),
            RpgRulesetValueKind::Stat => entity.stat(value_id),
        };
        value.ok_or_else(|| {
            self.fail(
                "RPG_RUNTIME_CONTRIBUTION_VALUE_MISSING",
                path,
                format!(
                    "entity {} has no {:?} {}",
                    entity.id(),
                    value_kind,
                    value_id
                ),
            )
        })
    }

    fn contribution_entity(
        &self,
        subject: RpgContributionSubject,
        target_id: &str,
        path: &str,
    ) -> Result<&rpg_core::RpgEntityState, RpgResolutionRejection> {
        let entity_id = self.contribution_subject_id(subject, target_id);
        self.workspace.state().entity(entity_id).ok_or_else(|| {
            self.fail(
                "RPG_RUNTIME_CONTRIBUTION_SUBJECT_MISSING",
                path,
                format!("contribution subject entity {entity_id} is unavailable"),
            )
        })
    }

    fn contribution_subject_id<'a>(
        &'a self,
        subject: RpgContributionSubject,
        target_id: &'a str,
    ) -> &'a str {
        match subject {
            RpgContributionSubject::Actor => &self.intent.actor_id,
            RpgContributionSubject::Target => target_id,
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
                    CheckOutcome::Scalar { .. } => {
                        return Err(self.fail(
                            "RPG_RUNTIME_CHECK_BRANCH_INCOMPATIBLE",
                            path,
                            "a scalar-test outcome cannot select an on-check branch",
                        ));
                    }
                    CheckOutcome::Vector { .. } => {
                        return Err(self.fail(
                            "RPG_RUNTIME_CHECK_BRANCH_INCOMPATIBLE",
                            path,
                            "a heterogeneous-pool outcome cannot select an on-check branch",
                        ));
                    }
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
            CompiledProgram::OnOutcome { branches, default } => {
                let outcome = self.current_outcome(path)?;
                let (profile_id, band_id, vector) = match outcome {
                    CheckOutcome::Scalar {
                        profile_id,
                        band_id,
                    } => (profile_id, band_id, false),
                    CheckOutcome::Vector {
                        profile_id,
                        band_id,
                    } => (profile_id, band_id, true),
                    _ => {
                        return Err(self.fail(
                            "RPG_RUNTIME_OUTCOME_BRANCH_INCOMPATIBLE",
                            path,
                            "on-outcome requires a scalar or heterogeneous-pool result",
                        ));
                    }
                };
                let (selected_branch_id, selected) = if let Some(selected) = branches.get(&band_id)
                {
                    (band_id.clone(), selected.as_ref())
                } else {
                    ("default".to_owned(), default.as_ref())
                };
                let target_id = self.target_id(path)?;
                let branch_event = if vector {
                    RpgDomainEvent::VectorOutcomeBranchSelected {
                        target_id,
                        profile_id,
                        final_band_id: band_id,
                        selected_branch_id: selected_branch_id.clone(),
                    }
                } else {
                    RpgDomainEvent::ScalarOutcomeBranchSelected {
                        target_id,
                        profile_id,
                        final_band_id: band_id,
                        selected_branch_id: selected_branch_id.clone(),
                    }
                };
                self.events.push(branch_event);
                self.trace.push(RpgTraceStep {
                    path: path.to_owned(),
                    code: "RPG_OUTCOME_BRANCH_SELECTED".to_owned(),
                    detail: format!("branch {selected_branch_id}"),
                });
                self.execute_program(selected, &format!("{path}.{selected_branch_id}"))
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
            RpgIrOperation::ApplyEffect { .. } | RpgIrOperation::RemoveEffect { .. } => {
                RpgCapabilityId::Effects
            }
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
            RpgIrOperation::ApplyEffect {
                effect_definition_id,
                rank,
            } => {
                let target_id = self.target_id(path)?;
                let rank = self.eval_formula(rank, &format!("{path}.rank"))?;
                let definition = self
                    .rules
                    .effect(effect_definition_id)
                    .cloned()
                    .ok_or_else(|| {
                        self.fail(
                            "RPG_RUNTIME_EFFECT_DEFINITION_UNKNOWN",
                            &format!("{path}.effectDefinitionId"),
                            format!("effect definition {effect_definition_id} is unavailable"),
                        )
                    })?;
                if rank < definition.rank_minimum || rank > definition.rank_maximum {
                    return Err(self.fail(
                        "RPG_RUNTIME_EFFECT_RANK_OUT_OF_BOUNDS",
                        &format!("{path}.rank"),
                        format!(
                            "effect rank {rank} must be within {}..={}",
                            definition.rank_minimum, definition.rank_maximum
                        ),
                    ));
                }
                let application_revision = self.workspace.state().revision().saturating_add(1);
                let ordinal = self.next_effect_ordinal;
                self.next_effect_ordinal = self.next_effect_ordinal.saturating_add(1);
                let instance_id = format!(
                    "{}:{}:{}:{}",
                    effect_definition_id, self.intent.actor_id, application_revision, ordinal
                );
                let mutation = self
                    .workspace
                    .effects_owner()
                    .apply(
                        &target_id,
                        &self.intent.actor_id,
                        &instance_id,
                        &definition.definition_id,
                        definition.definition_version,
                        &definition.stacking_id,
                        definition.stacking,
                        rank,
                        definition.duration_count,
                        application_revision,
                        definition.duration_anchor,
                    )
                    .map_err(|error| self.mutation_rejection(error, path))?;
                match mutation {
                    RpgEffectMutation::Applied {
                        effect,
                        replaced_effects,
                    } => {
                        let replaced_instance_ids = replaced_effects
                            .iter()
                            .map(|replaced| replaced.instance_id().to_owned())
                            .collect();
                        for replaced in replaced_effects {
                            self.events.push(RpgDomainEvent::EffectRemoved {
                                source_id: self.intent.actor_id.clone(),
                                target_id: target_id.clone(),
                                instance_id: replaced.instance_id().to_owned(),
                                definition_id: replaced.definition_id().to_owned(),
                                definition_version: replaced.definition_version(),
                                reason: "replaced".to_owned(),
                            });
                        }
                        self.events.push(RpgDomainEvent::EffectApplied {
                            source_id: self.intent.actor_id.clone(),
                            target_id,
                            instance_id: effect.instance_id().to_owned(),
                            definition_id: effect.definition_id().to_owned(),
                            definition_version: effect.definition_version(),
                            stacking_id: effect.stacking_id().to_owned(),
                            stacking: effect.stacking(),
                            rank: effect.rank(),
                            duration_anchor: effect.duration_anchor(),
                            remaining_count: effect.remaining_count(),
                            application_revision: effect.application_revision(),
                            replaced_instance_ids,
                        });
                    }
                    RpgEffectMutation::Refreshed {
                        previous,
                        current,
                        removed_effects,
                    } => {
                        let removed_instance_ids = removed_effects
                            .iter()
                            .map(|removed| removed.instance_id().to_owned())
                            .collect();
                        for removed in removed_effects {
                            self.events.push(RpgDomainEvent::EffectRemoved {
                                source_id: self.intent.actor_id.clone(),
                                target_id: target_id.clone(),
                                instance_id: removed.instance_id().to_owned(),
                                definition_id: removed.definition_id().to_owned(),
                                definition_version: removed.definition_version(),
                                reason: "refreshCollapsed".to_owned(),
                            });
                        }
                        self.events.push(RpgDomainEvent::EffectRefreshed {
                            source_id: self.intent.actor_id.clone(),
                            target_id,
                            instance_id: current.instance_id().to_owned(),
                            definition_id: current.definition_id().to_owned(),
                            definition_version: current.definition_version(),
                            stacking_id: current.stacking_id().to_owned(),
                            stacking: current.stacking(),
                            rank: current.rank(),
                            duration_anchor: current.duration_anchor(),
                            previous_count: previous.remaining_count(),
                            remaining_count: current.remaining_count(),
                            application_revision: current.application_revision(),
                            removed_instance_ids,
                        });
                    }
                }
            }
            RpgIrOperation::RemoveEffect {
                effect_definition_id,
            } => {
                if self.rules.effect(effect_definition_id).is_none() {
                    return Err(self.fail(
                        "RPG_RUNTIME_EFFECT_DEFINITION_UNKNOWN",
                        &format!("{path}.effectDefinitionId"),
                        format!("effect definition {effect_definition_id} is unavailable"),
                    ));
                }
                let target_id = self.target_id(path)?;
                let removed = self
                    .workspace
                    .effects_owner()
                    .remove_definition(&target_id, effect_definition_id)
                    .map_err(|error| self.mutation_rejection(error, path))?;
                for effect in removed {
                    self.events.push(RpgDomainEvent::EffectRemoved {
                        source_id: self.intent.actor_id.clone(),
                        target_id: target_id.clone(),
                        instance_id: effect.instance_id().to_owned(),
                        definition_id: effect.definition_id().to_owned(),
                        definition_version: effect.definition_version(),
                        reason: "explicit".to_owned(),
                    });
                }
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
                let request_options = options
                    .iter()
                    .map(|option| {
                        let unavailable = option.activation.as_ref().and_then(|activation| {
                            activation_rejection(
                                self.rules,
                                self.workspace.state(),
                                &target_id,
                                activation,
                                path,
                            )
                            .err()
                            .map(|rejection| RpgReactionUnavailable {
                                code: rejection.code,
                                path: rejection.path,
                                message: rejection.message,
                            })
                        });
                        let activation_costs = option
                            .activation
                            .as_ref()
                            .into_iter()
                            .flat_map(|activation| activation.costs.iter())
                            .map(|cost| RpgReactionActivationBudgetCost {
                                budget_id: cost.budget.id.clone(),
                                amount: cost.amount,
                                remaining: self
                                    .workspace
                                    .state()
                                    .entity(&target_id)
                                    .and_then(|entity| entity.activation_budget(&cost.budget.id))
                                    .unwrap_or(0),
                            })
                            .collect();
                        RpgReactionOption {
                            id: option.id.clone(),
                            label: option.label.clone(),
                            damage_reduction: option.damage_reduction,
                            activation_costs,
                            unavailable,
                        }
                    })
                    .collect();
                let request = RpgReactionRequest {
                    reaction_id: reaction_id.clone(),
                    actor_id: self.intent.actor_id.clone(),
                    target_id: target_id.clone(),
                    action_id: self.intent.action_id.clone(),
                    options: request_options,
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
                let selected_option = match &decision.option_id {
                    Some(option_id) => Some(
                        options
                            .iter()
                            .find(|option| option.id == *option_id)
                            .ok_or_else(|| {
                                self.fail(
                                    "RPG_REACTION_OPTION_UNKNOWN",
                                    "$.reaction.optionId",
                                    format!("unknown reaction option {option_id}"),
                                )
                            })?,
                    ),
                    None => None,
                };
                if let Some(activation) =
                    selected_option.and_then(|option| option.activation.as_ref())
                {
                    self.spend_activation(&target_id, activation, "$.reaction.option.activation")?;
                }
                let damage_reduction = selected_option.map_or(0, |option| option.damage_reduction);
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
                heterogeneous_terms: Vec::new(),
            },
            values: vec![value],
            heterogeneous_values: Vec::new(),
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
            heterogeneous_terms: Vec::new(),
        }));
        rejection
    }

    fn current_outcome(&self, path: &str) -> Result<CheckOutcome, RpgResolutionRejection> {
        let target_id = self.target_id(path)?;
        self.outcomes.get(&target_id).cloned().ok_or_else(|| {
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
            RpgCapabilityMutationError::UnknownActivationBudget => (
                "RPG_MUTATION_ACTIVATION_BUDGET_UNKNOWN",
                "activation budget is unknown",
            ),
            RpgCapabilityMutationError::InsufficientActivationBudget => (
                "RPG_MUTATION_ACTIVATION_BUDGET_INSUFFICIENT",
                "activation budget is insufficient",
            ),
            RpgCapabilityMutationError::ActivationCeilingExceeded => (
                "RPG_MUTATION_ACTIVATION_CEILING_EXCEEDED",
                "activation ceiling is exceeded",
            ),
            RpgCapabilityMutationError::ResourceOutOfBounds => (
                "RPG_MUTATION_RESOURCE_OUT_OF_BOUNDS",
                "resource transition exceeds its declared bounds",
            ),
            RpgCapabilityMutationError::ModifierTenureInvalid => (
                "RPG_MUTATION_MODIFIER_TENURE_INVALID",
                "modifier tenure is outside the supported turn bounds",
            ),
            RpgCapabilityMutationError::EffectTenureInvalid => (
                "RPG_MUTATION_EFFECT_TENURE_INVALID",
                "effect tenure is outside the supported boundary bounds",
            ),
            RpgCapabilityMutationError::TooManyActiveEffects => (
                "RPG_MUTATION_EFFECT_LIMIT_EXCEEDED",
                "target has the maximum number of active effect instances",
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

struct ScalarResolutionRequest<'a> {
    profile_id: &'a str,
    bands: &'a [RulesetOutcomeBand],
    margin_rules: &'a [RulesetMarginBandRule],
    natural_die_rules: &'a [RulesetNaturalDieRule],
    contribution_selector_id: Option<&'a str>,
    domain_minimum: i64,
    domain_maximum: i64,
    apply_contextual_shifts: bool,
    roll: u32,
    base_value: i32,
    difficulty: i32,
}

struct ResolvedScalar {
    contribution_ledger: RpgScalarContributionLedger,
    total: i32,
    margin: i32,
    base_band_id: String,
    natural_die_resolution: Option<RpgNaturalDieResolution>,
    band_shift_ledger: RpgOutcomeBandShiftLedger,
    final_band_id: String,
}

fn legacy_attack_bands() -> Vec<RulesetOutcomeBand> {
    vec![
        RulesetOutcomeBand {
            id: "miss".to_owned(),
            label: "Miss".to_owned(),
        },
        RulesetOutcomeBand {
            id: "hit".to_owned(),
            label: "Hit".to_owned(),
        },
    ]
}

fn legacy_attack_margin_rules() -> Vec<RulesetMarginBandRule> {
    vec![
        RulesetMarginBandRule {
            minimum: None,
            maximum: Some(-1),
            band_id: "miss".to_owned(),
        },
        RulesetMarginBandRule {
            minimum: Some(0),
            maximum: None,
            band_id: "hit".to_owned(),
        },
    ]
}

fn legacy_save_bands() -> Vec<RulesetOutcomeBand> {
    vec![
        RulesetOutcomeBand {
            id: "failed".to_owned(),
            label: "Failed".to_owned(),
        },
        RulesetOutcomeBand {
            id: "saved".to_owned(),
            label: "Saved".to_owned(),
        },
    ]
}

fn legacy_save_margin_rules() -> Vec<RulesetMarginBandRule> {
    vec![
        RulesetMarginBandRule {
            minimum: None,
            maximum: Some(-1),
            band_id: "failed".to_owned(),
        },
        RulesetMarginBandRule {
            minimum: Some(0),
            maximum: None,
            band_id: "saved".to_owned(),
        },
    ]
}

fn band_index(bands: &[RulesetOutcomeBand], band_id: &str) -> Option<usize> {
    bands.iter().position(|band| band.id == band_id)
}

fn shifted_band_index(current: usize, shift: i32, band_count: usize) -> usize {
    let maximum = i64::try_from(band_count.saturating_sub(1)).unwrap_or(i64::MAX);
    let shifted = i64::try_from(current)
        .unwrap_or(i64::MAX)
        .saturating_add(i64::from(shift))
        .clamp(0, maximum);
    usize::try_from(shifted).unwrap_or(band_count.saturating_sub(1))
}

fn ensure_scalar_domain(
    value: i32,
    minimum: i64,
    maximum: i64,
    path: &str,
    execution: &Execution<'_>,
) -> Result<(), RpgResolutionRejection> {
    let value = i64::from(value);
    if value < minimum || value > maximum {
        return Err(execution.fail(
            "RPG_RUNTIME_SCALAR_DOMAIN_EXCEEDED",
            path,
            format!("scalar value {value} is outside profile domain {minimum}..={maximum}"),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct PendingOutcomeBandShift {
    source_definition_id: String,
    source_instance_id: Option<String>,
    source_label: String,
    definition: RpgOutcomeBandShiftDefinition,
}

impl PendingOutcomeBandShift {
    fn canonical_key(&self) -> (&str, &str, &str) {
        (
            self.source_definition_id.as_str(),
            self.definition.id.as_str(),
            self.source_instance_id.as_deref().unwrap_or(""),
        )
    }

    fn display_key(&self) -> String {
        format!(
            "{}#{}:{}",
            self.source_definition_id,
            self.source_instance_id.as_deref().unwrap_or("-"),
            self.definition.id
        )
    }
}

#[derive(Debug, Clone)]
struct PendingContribution {
    source_definition_id: String,
    source_instance_id: Option<String>,
    source_label: String,
    definition: RpgScalarContributionDefinition,
}

#[derive(Debug, Clone)]
struct PendingPoolContribution {
    source_definition_id: String,
    source_instance_id: Option<String>,
    source_label: String,
    definition: RpgPoolContributionDefinition,
}

impl PendingPoolContribution {
    fn canonical_key(&self) -> (&str, &str, &str) {
        (
            self.source_definition_id.as_str(),
            self.source_instance_id.as_deref().unwrap_or(""),
            self.definition.id.as_str(),
        )
    }

    fn display_key(&self) -> String {
        format!(
            "{}#{}:{}",
            self.source_definition_id,
            self.source_instance_id.as_deref().unwrap_or("-"),
            self.definition.id
        )
    }
}

impl PendingContribution {
    fn canonical_key(&self) -> (&str, &str, &str) {
        (
            self.source_definition_id.as_str(),
            self.definition.id.as_str(),
            self.source_instance_id.as_deref().unwrap_or(""),
        )
    }

    fn display_key(&self) -> String {
        format!(
            "{}#{}:{}",
            self.source_definition_id,
            self.source_instance_id.as_deref().unwrap_or("-"),
            self.definition.id
        )
    }
}

fn retained_contribution_indices(
    policy: RpgContributionStackingPolicy,
    indices: &[usize],
    decisions: &[RpgScalarContributionDecision],
) -> Vec<usize> {
    match policy {
        RpgContributionStackingPolicy::Sum => indices.to_vec(),
        RpgContributionStackingPolicy::Greatest => {
            select_first_extreme(indices, decisions, |candidate, selected| {
                candidate > selected
            })
            .into_iter()
            .collect()
        }
        RpgContributionStackingPolicy::Least => {
            select_first_extreme(indices, decisions, |candidate, selected| {
                candidate < selected
            })
            .into_iter()
            .collect()
        }
        RpgContributionStackingPolicy::SignedExtremes => {
            let positive_indices = indices
                .iter()
                .copied()
                .filter(|index| decisions[*index].declared_value > 0)
                .collect::<Vec<_>>();
            let negative_indices = indices
                .iter()
                .copied()
                .filter(|index| decisions[*index].declared_value < 0)
                .collect::<Vec<_>>();
            let positive =
                select_first_extreme(&positive_indices, decisions, |candidate, selected| {
                    candidate > selected
                });
            let negative =
                select_first_extreme(&negative_indices, decisions, |candidate, selected| {
                    candidate < selected
                });
            positive.into_iter().chain(negative).collect()
        }
    }
}

fn select_first_extreme(
    indices: &[usize],
    decisions: &[RpgScalarContributionDecision],
    replaces: impl Fn(i32, i32) -> bool,
) -> Option<usize> {
    let mut selected = indices.first().copied()?;
    for index in indices.iter().copied().skip(1) {
        if replaces(
            decisions[index].declared_value,
            decisions[selected].declared_value,
        ) {
            selected = index;
        }
    }
    Some(selected)
}

fn decision_key(decision: &RpgScalarContributionDecision) -> String {
    format!(
        "{}#{}:{}",
        decision.source_definition_id,
        decision.source_instance_id.as_deref().unwrap_or("-"),
        decision.contribution_id
    )
}

fn retained_pool_contribution_indices(
    policy: RpgContributionStackingPolicy,
    indices: &[usize],
    decisions: &[RpgPoolContributionDecision],
) -> Vec<usize> {
    match policy {
        RpgContributionStackingPolicy::Sum => indices.to_vec(),
        RpgContributionStackingPolicy::Greatest => {
            select_first_pool_extreme(indices, decisions, |candidate, selected| {
                candidate > selected
            })
            .into_iter()
            .collect()
        }
        RpgContributionStackingPolicy::Least => {
            select_first_pool_extreme(indices, decisions, |candidate, selected| {
                candidate < selected
            })
            .into_iter()
            .collect()
        }
        RpgContributionStackingPolicy::SignedExtremes => {
            let positive_indices = indices
                .iter()
                .copied()
                .filter(|index| decisions[*index].effect.stacking_value() > 0)
                .collect::<Vec<_>>();
            let negative_indices = indices
                .iter()
                .copied()
                .filter(|index| decisions[*index].effect.stacking_value() < 0)
                .collect::<Vec<_>>();
            let positive =
                select_first_pool_extreme(&positive_indices, decisions, |candidate, selected| {
                    candidate > selected
                });
            let negative =
                select_first_pool_extreme(&negative_indices, decisions, |candidate, selected| {
                    candidate < selected
                });
            positive.into_iter().chain(negative).collect()
        }
    }
}

fn select_first_pool_extreme(
    indices: &[usize],
    decisions: &[RpgPoolContributionDecision],
    replaces: impl Fn(i32, i32) -> bool,
) -> Option<usize> {
    let mut selected = indices.first().copied()?;
    for index in indices.iter().copied().skip(1) {
        if replaces(
            decisions[index].effect.stacking_value(),
            decisions[selected].effect.stacking_value(),
        ) {
            selected = index;
        }
    }
    Some(selected)
}

fn pool_decision_key(decision: &RpgPoolContributionDecision) -> String {
    format!(
        "{}#{}:{}",
        decision.source_definition_id,
        decision.source_instance_id.as_deref().unwrap_or("-"),
        decision.contribution_id
    )
}

fn ensure_pool_total_bound(
    dice: &BTreeMap<String, u32>,
    path: &str,
    execution: &Execution<'_>,
) -> Result<(), RpgResolutionRejection> {
    let total = dice.values().try_fold(0_u32, |total, count| {
        total.checked_add(*count).ok_or_else(|| {
            execution.fail(
                "RPG_RUNTIME_POOL_DIE_COUNT_OVERFLOW",
                path,
                "pool total die count exceeded the runtime integer domain",
            )
        })
    })?;
    if total == 0 || total > 256 {
        return Err(execution.fail(
            "RPG_RUNTIME_POOL_DIE_COUNT_INVALID",
            path,
            format!("pool total die count {total} is outside 1..=256"),
        ));
    }
    Ok(())
}

fn compare_i64(left: i64, comparison: RpgContributionComparison, right: i64) -> bool {
    match comparison {
        RpgContributionComparison::LessThan => left < right,
        RpgContributionComparison::LessThanOrEqual => left <= right,
        RpgContributionComparison::Equal => left == right,
        RpgContributionComparison::GreaterThanOrEqual => left >= right,
        RpgContributionComparison::GreaterThan => left > right,
    }
}

fn contribution_subject_name(subject: RpgContributionSubject) -> &'static str {
    match subject {
        RpgContributionSubject::Actor => "actor",
        RpgContributionSubject::Target => "target",
    }
}

fn contribution_comparison_name(comparison: RpgContributionComparison) -> &'static str {
    match comparison {
        RpgContributionComparison::LessThan => "lessThan",
        RpgContributionComparison::LessThanOrEqual => "lessThanOrEqual",
        RpgContributionComparison::Equal => "equal",
        RpgContributionComparison::GreaterThanOrEqual => "greaterThanOrEqual",
        RpgContributionComparison::GreaterThan => "greaterThan",
    }
}

fn ruleset_value_kind_name(kind: RpgRulesetValueKind) -> &'static str {
    match kind {
        RpgRulesetValueKind::Defense => "defense",
        RpgRulesetValueKind::Stat => "stat",
    }
}

fn team_relation_name(relation: RpgContributionTeamRelation) -> &'static str {
    match relation {
        RpgContributionTeamRelation::Same => "same",
        RpgContributionTeamRelation::Different => "different",
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
    }
}
