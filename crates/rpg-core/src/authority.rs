use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{BoundedValue, GridPosition, Team};

pub const MAXIMUM_RPG_MODIFIER_TURNS: u32 = 1_000;
pub const MAXIMUM_RPG_EFFECT_DURATION: u32 = 1_000;
pub const MAXIMUM_ACTIVE_RPG_EFFECTS: usize = 64;
pub const MAXIMUM_RPG_DAMAGE_PARTS: usize = 16;
pub const MAXIMUM_RPG_DAMAGE_RESPONSES: usize = 64;
pub const MAXIMUM_RPG_DAMAGE_TAGS: usize = 16;
pub const MAXIMUM_RPG_DAMAGE_SCALE_COMPONENT: u32 = 1_000;

/// Closed identities for the private capability workspaces owned by RPG authority.
///
/// Operations bind to these values at compile time and must acquire the matching
/// owner before they can stage a mutation. Strings remain only at the serialized
/// vocabulary boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RpgCapabilityId {
    Vitality,
    Stats,
    Defenses,
    Resources,
    Modifiers,
    Position,
    Random,
    Reactions,
    ActivationBudgets,
    Effects,
}

impl RpgCapabilityId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vitality => "capability.vitality",
            Self::Stats => "capability.stats",
            Self::Defenses => "capability.defenses",
            Self::Resources => "capability.resources",
            Self::Modifiers => "capability.modifiers",
            Self::Position => "capability.position",
            Self::Random => "capability.random",
            Self::Reactions => "capability.reactions",
            Self::ActivationBudgets => "capability.activation-budgets",
            Self::Effects => "capability.effects",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgEntityState {
    id: String,
    team: Team,
    position: GridPosition,
    class_definition_id: Option<String>,
    character_feature_ids: Vec<String>,
    vitality: BoundedValue,
    stats: BTreeMap<String, i32>,
    defenses: BTreeMap<String, i32>,
    resources: BTreeMap<String, BoundedValue>,
    modifiers: BTreeMap<String, ActiveRpgModifier>,
    activation_budgets: BTreeMap<String, i32>,
    effects: BTreeMap<String, ActiveRpgEffect>,
}

impl RpgEntityState {
    pub fn new(id: impl Into<String>, team: Team, position: GridPosition, vitality: i32) -> Self {
        Self {
            id: id.into(),
            team,
            position,
            class_definition_id: None,
            character_feature_ids: Vec::new(),
            vitality: BoundedValue {
                current: vitality,
                max: vitality,
            },
            stats: BTreeMap::new(),
            defenses: BTreeMap::new(),
            resources: BTreeMap::new(),
            modifiers: BTreeMap::new(),
            activation_budgets: BTreeMap::new(),
            effects: BTreeMap::new(),
        }
    }

    pub fn with_stat(mut self, id: impl Into<String>, value: i32) -> Self {
        self.stats.insert(id.into(), value);
        self
    }

    pub fn with_defense(mut self, id: impl Into<String>, value: i32) -> Self {
        self.defenses.insert(id.into(), value);
        self
    }

    pub fn with_resource(mut self, id: impl Into<String>, current: i32, maximum: i32) -> Self {
        self.resources.insert(
            id.into(),
            BoundedValue {
                current,
                max: maximum,
            },
        );
        self
    }

    pub fn restore(
        id: impl Into<String>,
        team: Team,
        position: GridPosition,
        vitality: BoundedValue,
    ) -> Result<Self, RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if vitality.max < 0 || vitality.current < 0 || vitality.current > vitality.max {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        Ok(Self {
            id,
            team,
            position,
            class_definition_id: None,
            character_feature_ids: Vec::new(),
            vitality,
            stats: BTreeMap::new(),
            defenses: BTreeMap::new(),
            resources: BTreeMap::new(),
            modifiers: BTreeMap::new(),
            activation_budgets: BTreeMap::new(),
            effects: BTreeMap::new(),
        })
    }

    pub fn restore_resource(
        &mut self,
        id: impl Into<String>,
        value: BoundedValue,
    ) -> Result<(), RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if value.max < 0 || value.current < 0 || value.current > value.max {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        if self.resources.insert(id, value).is_some() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_stat(
        &mut self,
        id: impl Into<String>,
        value: i32,
    ) -> Result<(), RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if self.stats.insert(id, value).is_some() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_defense(
        &mut self,
        id: impl Into<String>,
        value: i32,
    ) -> Result<(), RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if self.defenses.insert(id, value).is_some() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_modifier(
        &mut self,
        stacking_group: impl Into<String>,
        modifier: ActiveRpgModifier,
    ) -> Result<(), RpgStateRestoreError> {
        let stacking_group = stacking_group.into();
        if stacking_group.is_empty() || modifier.id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if self.modifiers.insert(stacking_group, modifier).is_some() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_activation_budget(
        &mut self,
        id: impl Into<String>,
        value: i32,
    ) -> Result<(), RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if value < 0 {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        if self.activation_budgets.insert(id, value).is_some() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_effect(&mut self, effect: ActiveRpgEffect) -> Result<(), RpgStateRestoreError> {
        if self.effects.len() >= MAXIMUM_ACTIVE_RPG_EFFECTS {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        if self
            .effects
            .insert(effect.instance_id.clone(), effect)
            .is_some()
        {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        Ok(())
    }

    pub fn restore_character_selection(
        &mut self,
        class_definition_id: Option<String>,
        character_feature_ids: Vec<String>,
    ) -> Result<(), RpgStateRestoreError> {
        if self.class_definition_id.is_some() || !self.character_feature_ids.is_empty() {
            return Err(RpgStateRestoreError::DuplicateIdentity);
        }
        if class_definition_id
            .as_deref()
            .is_some_and(|definition_id| definition_id.trim().is_empty())
        {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        let mut previous = None::<&str>;
        for feature_id in &character_feature_ids {
            if feature_id.trim().is_empty()
                || previous.is_some_and(|previous| previous >= feature_id.as_str())
            {
                return Err(RpgStateRestoreError::DuplicateIdentity);
            }
            previous = Some(feature_id);
        }
        if class_definition_id.is_none() && !character_feature_ids.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        self.class_definition_id = class_definition_id;
        self.character_feature_ids = character_feature_ids;
        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn team(&self) -> &Team {
        &self.team
    }

    pub fn position(&self) -> GridPosition {
        self.position
    }

    pub fn vitality(&self) -> BoundedValue {
        self.vitality
    }

    pub fn class_definition_id(&self) -> Option<&str> {
        self.class_definition_id.as_deref()
    }

    pub fn character_feature_ids(&self) -> &[String] {
        &self.character_feature_ids
    }

    pub fn stat(&self, id: &str) -> Option<i32> {
        self.stats.get(id).copied()
    }

    pub fn defense(&self, id: &str) -> Option<i32> {
        self.defenses.get(id).copied()
    }

    pub fn resource(&self, id: &str) -> Option<BoundedValue> {
        self.resources.get(id).copied()
    }

    pub fn modifier(&self, id: &str) -> Option<&ActiveRpgModifier> {
        self.modifiers.values().find(|modifier| modifier.id == id)
    }

    pub fn modifier_in_group(&self, group: &str) -> Option<&ActiveRpgModifier> {
        self.modifiers.get(group)
    }

    pub fn activation_budget(&self, id: &str) -> Option<i32> {
        self.activation_budgets.get(id).copied()
    }

    pub fn effect(&self, instance_id: &str) -> Option<&ActiveRpgEffect> {
        self.effects.get(instance_id)
    }

    pub fn has_effect_definition(&self, definition_id: &str) -> bool {
        self.effects
            .values()
            .any(|effect| effect.definition_id == definition_id)
    }

    pub fn stats(&self) -> impl Iterator<Item = (&str, i32)> {
        self.stats.iter().map(|(id, value)| (id.as_str(), *value))
    }

    pub fn defenses(&self) -> impl Iterator<Item = (&str, i32)> {
        self.defenses
            .iter()
            .map(|(id, value)| (id.as_str(), *value))
    }

    pub fn resources(&self) -> impl Iterator<Item = (&str, BoundedValue)> {
        self.resources
            .iter()
            .map(|(id, value)| (id.as_str(), *value))
    }

    pub fn modifiers(&self) -> impl Iterator<Item = (&str, &ActiveRpgModifier)> {
        self.modifiers
            .iter()
            .map(|(group, modifier)| (group.as_str(), modifier))
    }

    pub fn activation_budgets(&self) -> impl Iterator<Item = (&str, i32)> {
        self.activation_budgets
            .iter()
            .map(|(id, value)| (id.as_str(), *value))
    }

    pub fn effects(&self) -> impl Iterator<Item = &ActiveRpgEffect> {
        self.effects.values()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgEffectDurationAnchor {
    GlobalTurnTransition,
    RoundTransition,
    SourceTurnStart,
    TargetTurnStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgEffectStackingPolicy {
    IndependentBySource,
    Replace,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRpgEffect {
    instance_id: String,
    definition_id: String,
    definition_version: u32,
    source_entity_id: String,
    stacking_id: String,
    stacking: RpgEffectStackingPolicy,
    rank: i32,
    remaining_count: u32,
    application_revision: u64,
    duration_anchor: RpgEffectDurationAnchor,
}

impl ActiveRpgEffect {
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        instance_id: impl Into<String>,
        definition_id: impl Into<String>,
        definition_version: u32,
        source_entity_id: impl Into<String>,
        stacking_id: impl Into<String>,
        stacking: RpgEffectStackingPolicy,
        rank: i32,
        remaining_count: u32,
        application_revision: u64,
        duration_anchor: RpgEffectDurationAnchor,
    ) -> Result<Self, RpgStateRestoreError> {
        let instance_id = instance_id.into();
        let definition_id = definition_id.into();
        let source_entity_id = source_entity_id.into();
        let stacking_id = stacking_id.into();
        if instance_id.is_empty()
            || definition_id.is_empty()
            || source_entity_id.is_empty()
            || stacking_id.is_empty()
            || definition_version == 0
        {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if !(1..=MAXIMUM_RPG_EFFECT_DURATION).contains(&remaining_count)
            || application_revision == 0
        {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        Ok(Self {
            instance_id,
            definition_id,
            definition_version,
            source_entity_id,
            stacking_id,
            stacking,
            rank,
            remaining_count,
            application_revision,
            duration_anchor,
        })
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn definition_id(&self) -> &str {
        &self.definition_id
    }

    pub fn source_entity_id(&self) -> &str {
        &self.source_entity_id
    }

    pub fn definition_version(&self) -> u32 {
        self.definition_version
    }

    pub fn stacking_id(&self) -> &str {
        &self.stacking_id
    }

    pub fn stacking(&self) -> RpgEffectStackingPolicy {
        self.stacking
    }

    pub fn rank(&self) -> i32 {
        self.rank
    }

    pub fn remaining_count(&self) -> u32 {
        self.remaining_count
    }

    pub fn application_revision(&self) -> u64 {
        self.application_revision
    }

    pub fn duration_anchor(&self) -> RpgEffectDurationAnchor {
        self.duration_anchor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRpgModifier {
    id: String,
    value: i32,
    remaining_turns: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgModifierStackingPolicy {
    Replace,
    Refresh,
}

impl ActiveRpgModifier {
    pub fn restore(
        id: impl Into<String>,
        value: i32,
        remaining_turns: u32,
    ) -> Result<Self, RpgStateRestoreError> {
        let id = id.into();
        if id.is_empty() {
            return Err(RpgStateRestoreError::EmptyIdentity);
        }
        if !(1..=MAXIMUM_RPG_MODIFIER_TURNS).contains(&remaining_turns) {
            return Err(RpgStateRestoreError::ValueOutOfBounds);
        }
        Ok(Self {
            id,
            value,
            remaining_turns,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn remaining_turns(&self) -> u32 {
        self.remaining_turns
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RpgCapabilityState {
    revision: u64,
    entities: BTreeMap<String, RpgEntityState>,
    accepted_activations_this_turn: u32,
}

impl RpgCapabilityState {
    pub fn restore(
        revision: u64,
        entities: impl IntoIterator<Item = RpgEntityState>,
    ) -> Result<Self, RpgStateRestoreError> {
        let mut restored = Self {
            revision,
            entities: BTreeMap::new(),
            accepted_activations_this_turn: 0,
        };
        for entity in entities {
            if restored
                .entities
                .insert(entity.id.clone(), entity)
                .is_some()
            {
                return Err(RpgStateRestoreError::DuplicateIdentity);
            }
        }
        Ok(restored)
    }

    pub fn restore_with_activation_count(
        revision: u64,
        entities: impl IntoIterator<Item = RpgEntityState>,
        accepted_activations_this_turn: u32,
    ) -> Result<Self, RpgStateRestoreError> {
        let mut restored = Self::restore(revision, entities)?;
        restored.accepted_activations_this_turn = accepted_activations_this_turn;
        Ok(restored)
    }

    pub fn entity(&self, id: &str) -> Option<&RpgEntityState> {
        self.entities.get(id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn entities(&self) -> impl Iterator<Item = &RpgEntityState> {
        self.entities.values()
    }

    pub fn accepted_activations_this_turn(&self) -> u32 {
        self.accepted_activations_this_turn
    }

    pub fn insert_entity(&mut self, entity: RpgEntityState) -> Option<RpgEntityState> {
        self.entities.insert(entity.id.clone(), entity)
    }

    pub fn vitality_owner(&mut self) -> RpgVitalityOwner<'_> {
        RpgVitalityOwner { state: self }
    }

    pub fn resources_owner(&mut self) -> RpgResourcesOwner<'_> {
        RpgResourcesOwner { state: self }
    }

    pub fn modifiers_owner(&mut self) -> RpgModifiersOwner<'_> {
        RpgModifiersOwner { state: self }
    }

    pub fn position_owner(&mut self) -> RpgPositionOwner<'_> {
        RpgPositionOwner { state: self }
    }

    pub fn activation_budgets_owner(&mut self) -> RpgActivationBudgetsOwner<'_> {
        RpgActivationBudgetsOwner { state: self }
    }

    pub fn effects_owner(&mut self) -> RpgEffectsOwner<'_> {
        RpgEffectsOwner { state: self }
    }

    pub fn advance_revision(&mut self) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.revision
    }

    fn entity_mut_for_owner(
        &mut self,
        entity_id: &str,
    ) -> Result<&mut RpgEntityState, RpgCapabilityMutationError> {
        self.entities
            .get_mut(entity_id)
            .ok_or(RpgCapabilityMutationError::UnknownEntity)
    }

    fn resource_mut(
        &mut self,
        entity_id: &str,
        resource_id: &str,
    ) -> Result<&mut BoundedValue, RpgCapabilityMutationError> {
        self.entity_mut_for_owner(entity_id)?
            .resources
            .get_mut(resource_id)
            .ok_or(RpgCapabilityMutationError::UnknownResource)
    }

    fn activation_budget_mut(
        &mut self,
        entity_id: &str,
        budget_id: &str,
    ) -> Result<&mut i32, RpgCapabilityMutationError> {
        self.entity_mut_for_owner(entity_id)?
            .activation_budgets
            .get_mut(budget_id)
            .ok_or(RpgCapabilityMutationError::UnknownActivationBudget)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpgStateRestoreError {
    EmptyIdentity,
    DuplicateIdentity,
    ValueOutOfBounds,
}

/// One atomic transaction over all RPG capability owners and deterministic
/// random evidence. The authoritative session stages this workspace, and only
/// an accepted resolution can commit it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgCapabilityWorkspace {
    state: RpgCapabilityState,
    random: DeterministicRandomStream,
}

impl RpgCapabilityWorkspace {
    pub fn stage(state: &RpgCapabilityState, random: &DeterministicRandomStream) -> Self {
        Self {
            state: state.clone(),
            random: random.clone(),
        }
    }

    pub fn state(&self) -> &RpgCapabilityState {
        &self.state
    }

    pub fn vitality_owner(&mut self) -> RpgVitalityOwner<'_> {
        self.state.vitality_owner()
    }

    pub fn resources_owner(&mut self) -> RpgResourcesOwner<'_> {
        self.state.resources_owner()
    }

    pub fn modifiers_owner(&mut self) -> RpgModifiersOwner<'_> {
        self.state.modifiers_owner()
    }

    pub fn position_owner(&mut self) -> RpgPositionOwner<'_> {
        self.state.position_owner()
    }

    pub fn activation_budgets_owner(&mut self) -> RpgActivationBudgetsOwner<'_> {
        self.state.activation_budgets_owner()
    }

    pub fn effects_owner(&mut self) -> RpgEffectsOwner<'_> {
        self.state.effects_owner()
    }

    pub fn random_owner(&mut self) -> RpgRandomOwner<'_> {
        RpgRandomOwner {
            random: &mut self.random,
        }
    }

    pub fn random_remaining(&self) -> usize {
        self.random.remaining()
    }

    pub fn random_consumed(&self) -> usize {
        self.random.consumed()
    }

    pub fn advance_revision(&mut self) -> u64 {
        self.state.advance_revision()
    }

    pub fn commit(self, state: &mut RpgCapabilityState, random: &mut DeterministicRandomStream) {
        *state = self.state;
        *random = self.random;
    }
}

pub struct RpgRandomOwner<'a> {
    random: &'a mut DeterministicRandomStream,
}

impl RpgRandomOwner<'_> {
    pub fn take(&mut self) -> Option<u32> {
        self.random.take()
    }
}

pub struct RpgVitalityOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

impl RpgVitalityOwner<'_> {
    pub fn apply_damage(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        if amount < 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let entity = self.state.entity_mut_for_owner(entity_id)?;
        entity.vitality.current = entity.vitality.current.saturating_sub(amount).max(0);
        Ok(entity.vitality.current)
    }

    pub fn apply_healing(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        if amount < 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let entity = self.state.entity_mut_for_owner(entity_id)?;
        entity.vitality.current = entity
            .vitality
            .current
            .saturating_add(amount)
            .min(entity.vitality.max);
        Ok(entity.vitality.current)
    }
}

pub struct RpgResourcesOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

impl RpgResourcesOwner<'_> {
    pub fn spend(
        &mut self,
        entity_id: &str,
        resource_id: &str,
        amount: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        if amount <= 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let resource = self.state.resource_mut(entity_id, resource_id)?;
        if resource.current < amount {
            return Err(RpgCapabilityMutationError::InsufficientResource);
        }
        resource.current -= amount;
        Ok(resource.current)
    }

    pub fn change(
        &mut self,
        entity_id: &str,
        resource_id: &str,
        delta: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        let resource = self.state.resource_mut(entity_id, resource_id)?;
        let next = resource
            .current
            .checked_add(delta)
            .ok_or(RpgCapabilityMutationError::ResourceOutOfBounds)?;
        if next < 0 || next > resource.max {
            return Err(RpgCapabilityMutationError::ResourceOutOfBounds);
        }
        resource.current = next;
        Ok(next)
    }
}

pub struct RpgModifiersOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpgModifierTurnChange {
    Aged {
        entity_id: String,
        stacking_group: String,
        modifier_id: String,
        remaining_turns: u32,
    },
    Expired {
        entity_id: String,
        stacking_group: String,
        modifier_id: String,
    },
}

impl RpgModifiersOwner<'_> {
    pub fn apply(
        &mut self,
        entity_id: &str,
        modifier_id: &str,
        stacking_group: &str,
        stacking: RpgModifierStackingPolicy,
        value: i32,
        remaining_turns: u32,
    ) -> Result<(), RpgCapabilityMutationError> {
        if !(1..=MAXIMUM_RPG_MODIFIER_TURNS).contains(&remaining_turns) {
            return Err(RpgCapabilityMutationError::ModifierTenureInvalid);
        }
        let entity = self.state.entity_mut_for_owner(entity_id)?;
        match stacking {
            RpgModifierStackingPolicy::Replace => {
                entity.modifiers.insert(
                    stacking_group.to_owned(),
                    ActiveRpgModifier {
                        id: modifier_id.to_owned(),
                        value,
                        remaining_turns,
                    },
                );
            }
            RpgModifierStackingPolicy::Refresh => {
                let modifier = entity
                    .modifiers
                    .entry(stacking_group.to_owned())
                    .or_insert_with(|| ActiveRpgModifier {
                        id: modifier_id.to_owned(),
                        value,
                        remaining_turns,
                    });
                modifier.id = modifier_id.to_owned();
                modifier.value = value;
                modifier.remaining_turns = remaining_turns;
            }
        }
        Ok(())
    }

    /// Ages only modifiers that were present and unchanged before the accepted
    /// action. A modifier applied, replaced, or refreshed by that action starts
    /// its full authored tenure at the new turn boundary.
    pub fn advance_turn(
        &mut self,
        previous_state: &RpgCapabilityState,
        refreshed_modifiers: &BTreeSet<(String, String)>,
    ) -> Vec<RpgModifierTurnChange> {
        let mut changes = Vec::new();
        for previous_entity in previous_state.entities() {
            for (stacking_group, previous_modifier) in previous_entity.modifiers() {
                if refreshed_modifiers
                    .contains(&(previous_entity.id().to_owned(), stacking_group.to_owned()))
                {
                    continue;
                }
                let Some(entity) = self.state.entities.get_mut(previous_entity.id()) else {
                    continue;
                };
                if entity.modifiers.get(stacking_group) != Some(previous_modifier) {
                    continue;
                }
                if previous_modifier.remaining_turns > 1 {
                    let modifier = entity
                        .modifiers
                        .get_mut(stacking_group)
                        .expect("unchanged modifier remains present");
                    modifier.remaining_turns -= 1;
                    changes.push(RpgModifierTurnChange::Aged {
                        entity_id: previous_entity.id().to_owned(),
                        stacking_group: stacking_group.to_owned(),
                        modifier_id: modifier.id.clone(),
                        remaining_turns: modifier.remaining_turns,
                    });
                } else {
                    let modifier = entity
                        .modifiers
                        .remove(stacking_group)
                        .expect("unchanged modifier remains present");
                    changes.push(RpgModifierTurnChange::Expired {
                        entity_id: previous_entity.id().to_owned(),
                        stacking_group: stacking_group.to_owned(),
                        modifier_id: modifier.id,
                    });
                }
            }
        }
        changes
    }
}

pub struct RpgPositionOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

pub struct RpgEffectsOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpgEffectMutation {
    Applied {
        effect: ActiveRpgEffect,
        replaced_effects: Vec<ActiveRpgEffect>,
    },
    Refreshed {
        previous: ActiveRpgEffect,
        current: ActiveRpgEffect,
        removed_effects: Vec<ActiveRpgEffect>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpgEffectBoundaryChange {
    Aged {
        target_entity_id: String,
        effect: ActiveRpgEffect,
        previous_count: u32,
    },
    Expired {
        target_entity_id: String,
        effect: ActiveRpgEffect,
    },
}

impl RpgEffectsOwner<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        target_entity_id: &str,
        source_entity_id: &str,
        instance_id: &str,
        definition_id: &str,
        definition_version: u32,
        stacking_id: &str,
        stacking: RpgEffectStackingPolicy,
        rank: i32,
        remaining_count: u32,
        application_revision: u64,
        duration_anchor: RpgEffectDurationAnchor,
    ) -> Result<RpgEffectMutation, RpgCapabilityMutationError> {
        if !(1..=MAXIMUM_RPG_EFFECT_DURATION).contains(&remaining_count)
            || application_revision == 0
        {
            return Err(RpgCapabilityMutationError::EffectTenureInvalid);
        }
        if self.state.entity(source_entity_id).is_none() {
            return Err(RpgCapabilityMutationError::UnknownEntity);
        }
        let target = self.state.entity_mut_for_owner(target_entity_id)?;
        let matching = target
            .effects
            .values()
            .filter(|effect| {
                effect.stacking_id == stacking_id
                    && (stacking != RpgEffectStackingPolicy::IndependentBySource
                        || effect.source_entity_id == source_entity_id)
            })
            .map(|effect| effect.instance_id.clone())
            .collect::<Vec<_>>();
        let retained_id = matching.first().cloned();
        let actual_instance_id = match stacking {
            RpgEffectStackingPolicy::IndependentBySource | RpgEffectStackingPolicy::Refresh => {
                retained_id.as_deref().unwrap_or(instance_id)
            }
            RpgEffectStackingPolicy::Replace => instance_id,
        };
        let current = ActiveRpgEffect::restore(
            actual_instance_id,
            definition_id,
            definition_version,
            source_entity_id,
            stacking_id,
            stacking,
            rank,
            remaining_count,
            application_revision,
            duration_anchor,
        )
        .map_err(|_| RpgCapabilityMutationError::EffectTenureInvalid)?;
        let previous = retained_id
            .as_deref()
            .and_then(|id| target.effects.get(id))
            .cloned();
        let mut removed_effects = Vec::new();
        for id in matching {
            let removed = target
                .effects
                .remove(&id)
                .expect("matching effect remains present");
            if stacking == RpgEffectStackingPolicy::Replace
                || Some(id.as_str()) != retained_id.as_deref()
            {
                removed_effects.push(removed);
            }
        }
        if previous.is_none() && target.effects.len() >= MAXIMUM_ACTIVE_RPG_EFFECTS {
            return Err(RpgCapabilityMutationError::TooManyActiveEffects);
        }
        target
            .effects
            .insert(current.instance_id.clone(), current.clone());
        Ok(match (stacking, previous) {
            (RpgEffectStackingPolicy::Replace, _) => RpgEffectMutation::Applied {
                effect: current,
                replaced_effects: removed_effects,
            },
            (_, Some(previous)) => RpgEffectMutation::Refreshed {
                previous,
                current,
                removed_effects,
            },
            (_, None) => RpgEffectMutation::Applied {
                effect: current,
                replaced_effects: removed_effects,
            },
        })
    }

    pub fn remove_definition(
        &mut self,
        target_entity_id: &str,
        definition_id: &str,
    ) -> Result<Vec<ActiveRpgEffect>, RpgCapabilityMutationError> {
        let target = self.state.entity_mut_for_owner(target_entity_id)?;
        let ids = target
            .effects
            .values()
            .filter(|effect| effect.definition_id == definition_id)
            .map(|effect| effect.instance_id.clone())
            .collect::<Vec<_>>();
        Ok(ids
            .into_iter()
            .filter_map(|id| target.effects.remove(&id))
            .collect())
    }

    pub fn advance_boundaries(
        &mut self,
        transition_revision: u64,
        previous_actor_id: &str,
        current_actor_id: &str,
        round_transitioned: bool,
    ) -> Vec<RpgEffectBoundaryChange> {
        let mut candidates = self
            .state
            .entities()
            .flat_map(|target| {
                target.effects().filter_map(move |effect| {
                    let matches = match effect.duration_anchor {
                        RpgEffectDurationAnchor::GlobalTurnTransition => {
                            previous_actor_id != current_actor_id || round_transitioned
                        }
                        RpgEffectDurationAnchor::RoundTransition => round_transitioned,
                        RpgEffectDurationAnchor::SourceTurnStart => {
                            effect.source_entity_id == current_actor_id
                        }
                        RpgEffectDurationAnchor::TargetTurnStart => target.id == current_actor_id,
                    };
                    (matches && effect.application_revision != transition_revision).then(|| {
                        (
                            effect.duration_anchor,
                            target.id.clone(),
                            effect.definition_id.clone(),
                            effect.source_entity_id.clone(),
                            effect.instance_id.clone(),
                        )
                    })
                })
            })
            .collect::<Vec<_>>();
        candidates.sort();
        let mut changes = Vec::new();
        for (_, target_id, _, _, instance_id) in candidates {
            let Some(target) = self.state.entities.get_mut(&target_id) else {
                continue;
            };
            let Some(effect) = target.effects.get_mut(&instance_id) else {
                continue;
            };
            if effect.remaining_count > 1 {
                let previous_count = effect.remaining_count;
                effect.remaining_count -= 1;
                changes.push(RpgEffectBoundaryChange::Aged {
                    target_entity_id: target_id,
                    effect: effect.clone(),
                    previous_count,
                });
            } else if let Some(effect) = target.effects.remove(&instance_id) {
                changes.push(RpgEffectBoundaryChange::Expired {
                    target_entity_id: target_id,
                    effect,
                });
            }
        }
        changes
    }
}

pub struct RpgActivationBudgetsOwner<'a> {
    state: &'a mut RpgCapabilityState,
}

impl RpgActivationBudgetsOwner<'_> {
    pub fn spend(
        &mut self,
        entity_id: &str,
        budget_id: &str,
        amount: i32,
    ) -> Result<(i32, i32), RpgCapabilityMutationError> {
        if amount < 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let budget = self.state.activation_budget_mut(entity_id, budget_id)?;
        if *budget < amount {
            return Err(RpgCapabilityMutationError::InsufficientActivationBudget);
        }
        let previous = *budget;
        *budget -= amount;
        Ok((previous, *budget))
    }

    pub fn accept_activation(&mut self, ceiling: u32) -> Result<u32, RpgCapabilityMutationError> {
        if self.state.accepted_activations_this_turn >= ceiling {
            return Err(RpgCapabilityMutationError::ActivationCeilingExceeded);
        }
        self.state.accepted_activations_this_turn += 1;
        Ok(self.state.accepted_activations_this_turn)
    }

    pub fn reset_budget(
        &mut self,
        entity_id: &str,
        budget_id: &str,
        value: i32,
    ) -> Result<(i32, i32), RpgCapabilityMutationError> {
        if value < 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let budget = self.state.activation_budget_mut(entity_id, budget_id)?;
        let previous = *budget;
        *budget = value;
        Ok((previous, *budget))
    }

    pub fn reset_activation_count(&mut self) {
        self.state.accepted_activations_this_turn = 0;
    }
}

impl RpgPositionOwner<'_> {
    pub fn move_entity(
        &mut self,
        entity_id: &str,
        delta_x: i32,
        delta_y: i32,
        maximum_distance: u32,
    ) -> Result<(GridPosition, GridPosition), RpgCapabilityMutationError> {
        let distance = delta_x
            .unsigned_abs()
            .saturating_add(delta_y.unsigned_abs());
        if distance == 0 || distance > maximum_distance {
            return Err(RpgCapabilityMutationError::MovementDistanceInvalid);
        }
        let entity = self.state.entity_mut_for_owner(entity_id)?;
        let previous = entity.position;
        let x = i64::from(previous.x).saturating_add(i64::from(delta_x));
        let y = i64::from(previous.y).saturating_add(i64::from(delta_y));
        let x = u32::try_from(x).map_err(|_| RpgCapabilityMutationError::PositionOutOfBounds)?;
        let y = u32::try_from(y).map_err(|_| RpgCapabilityMutationError::PositionOutOfBounds)?;
        entity.position = GridPosition { x, y };
        Ok((previous, entity.position))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpgCapabilityMutationError {
    UnknownEntity,
    UnknownResource,
    UnknownActivationBudget,
    InvalidAmount,
    InsufficientResource,
    InsufficientActivationBudget,
    ActivationCeilingExceeded,
    ResourceOutOfBounds,
    ModifierTenureInvalid,
    EffectTenureInvalid,
    TooManyActiveEffects,
    MovementDistanceInvalid,
    PositionOutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIntentCellTarget {
    pub id: String,
    pub position: GridPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaFilteredParticipant {
    pub participant_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgAreaFilteredCell {
    pub x: i64,
    pub y: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgAreaOrigin {
    Anchor,
    Actor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgAreaShape {
    Diamond { radius: u32 },
    OrthogonalLine { length: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIntentItemBinding {
    pub binding_id: String,
    pub item_instance_id: String,
    pub item_definition_id: String,
    pub slot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIntent {
    pub action_id: String,
    pub actor_id: String,
    pub target_ids: Vec<String>,
    #[serde(default)]
    pub cell_targets: Vec<RpgIntentCellTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicRandomStream {
    values: Vec<u32>,
    cursor: usize,
}

impl DeterministicRandomStream {
    pub fn new(values: Vec<u32>) -> Self {
        Self { values, cursor: 0 }
    }

    pub fn consumed(&self) -> usize {
        self.cursor
    }

    pub fn remaining(&self) -> usize {
        self.values.len().saturating_sub(self.cursor)
    }

    pub fn take(&mut self) -> Option<u32> {
        let value = self.values.get(self.cursor).copied()?;
        self.cursor = self.cursor.saturating_add(1);
        Some(value)
    }

    pub fn extend(&mut self, values: impl IntoIterator<Item = u32>) {
        self.values.extend(values);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgRandomRequestKind {
    AttackCheck,
    SavingThrowCheck,
    ScalarTest,
    FormulaDice,
    HeterogeneousPool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgHeterogeneousRandomTerm {
    pub die_type_id: String,
    pub count: u32,
    pub sides: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomRequest {
    pub kind: RpgRandomRequestKind,
    pub count: u32,
    pub sides: u32,
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heterogeneous_terms: Vec<RpgHeterogeneousRandomTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgHeterogeneousRandomValue {
    pub die_type_id: String,
    pub ordinal: u32,
    pub sides: u32,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomEvidence {
    pub request: RpgRandomRequest,
    pub values: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heterogeneous_values: Vec<RpgHeterogeneousRandomValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionActivationBudgetCost {
    pub budget_id: String,
    pub amount: i32,
    pub remaining: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionUnavailable {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionOption {
    pub id: String,
    pub label: String,
    pub damage_reduction: u32,
    pub activation_costs: Vec<RpgReactionActivationBudgetCost>,
    pub unavailable: Option<RpgReactionUnavailable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionRequest {
    pub reaction_id: String,
    pub actor_id: String,
    pub target_id: String,
    pub action_id: String,
    pub options: Vec<RpgReactionOption>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionDecision {
    pub reaction_id: String,
    pub option_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgRulesetValueKind {
    Defense,
    Stat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgContributionSubject {
    Actor,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgContributionComparison {
    LessThan,
    LessThanOrEqual,
    Equal,
    GreaterThanOrEqual,
    GreaterThan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgContributionValueExpression {
    Constant {
        value: i64,
    },
    ReadValue {
        subject: RpgContributionSubject,
        ruleset_id: String,
        value_kind: RpgRulesetValueKind,
        value_id: String,
    },
    Add {
        terms: Vec<RpgContributionValueExpression>,
    },
    Subtract {
        minuend: Box<RpgContributionValueExpression>,
        subtrahend: Box<RpgContributionValueExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgContributionPredicate {
    Always,
    Not {
        predicate: Box<RpgContributionPredicate>,
    },
    All {
        predicates: Vec<RpgContributionPredicate>,
    },
    Any {
        predicates: Vec<RpgContributionPredicate>,
    },
    ActorIsTarget {
        expected: bool,
    },
    TeamRelation {
        relation: RpgContributionTeamRelation,
    },
    Living {
        subject: RpgContributionSubject,
        expected: bool,
    },
    NamedValue {
        subject: RpgContributionSubject,
        ruleset_id: String,
        value_kind: RpgRulesetValueKind,
        value_id: String,
        comparison: RpgContributionComparison,
        value: i64,
    },
    Distance {
        comparison: RpgContributionComparison,
        value: u32,
    },
    ActorFlanksTarget,
    ActorSurrounded {
        minimum_hostiles: u32,
    },
    BoundItemDefinition {
        definition_id: String,
    },
    BoundItemTag {
        tag: String,
    },
    ActionTag {
        tag: String,
    },
    CellCapability {
        subject: RpgContributionSubject,
        capability_id: String,
    },
    EffectActive {
        subject: RpgContributionSubject,
        definition_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgContributionTeamRelation {
    Same,
    Different,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgContributionStackingPolicy {
    Sum,
    Greatest,
    Least,
    SignedExtremes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgOwnedRulesetReference {
    pub ruleset_id: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScalarContributionSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScalarContributionDefinition {
    pub schema: RpgScalarContributionSchema,
    pub id: String,
    pub selector: RpgOwnedRulesetReference,
    pub stacking_group: RpgOwnedRulesetReference,
    pub value: RpgContributionValueExpression,
    pub predicate: RpgContributionPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgContributionDisposition {
    Applied,
    Inapplicable {
        reason: String,
    },
    Suppressed {
        policy: RpgContributionStackingPolicy,
        retained_contribution_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScalarContributionDecision {
    pub source_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
    pub source_label: String,
    pub contribution_id: String,
    pub selector_id: String,
    pub stacking_group_id: String,
    pub declared_value: i32,
    pub applied_value: i32,
    pub disposition: RpgContributionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgScalarContributionLedger {
    pub selector_id: String,
    pub base_value: i32,
    pub candidates: Vec<RpgScalarContributionDecision>,
    pub final_value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolContributionSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgPoolContributionEffect {
    AddDice {
        die_type_id: String,
        delta: i32,
    },
    AddAxis {
        axis_id: String,
        value: i32,
    },
    ReplaceOrAddDie {
        from_die_type_id: String,
        to_die_type_id: String,
        count: u32,
        fallback_die_type_id: String,
    },
}

impl RpgPoolContributionEffect {
    pub fn stacking_value(&self) -> i32 {
        match self {
            Self::AddDice { delta, .. } => *delta,
            Self::AddAxis { value, .. } => *value,
            Self::ReplaceOrAddDie { count, .. } => i32::try_from(*count).unwrap_or(i32::MAX),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolContributionDefinition {
    pub schema: RpgPoolContributionSchema,
    pub id: String,
    pub profile: RpgOwnedRulesetReference,
    pub stacking_group: RpgOwnedRulesetReference,
    pub effect: RpgPoolContributionEffect,
    pub predicate: RpgContributionPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolContributionDecision {
    pub source_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
    pub source_label: String,
    pub contribution_id: String,
    pub profile_id: String,
    pub stacking_group_id: String,
    pub effect: RpgPoolContributionEffect,
    pub disposition: RpgContributionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolReplacementUnit {
    pub contribution_id: String,
    pub unit: u32,
    pub from_die_type_id: String,
    pub added_die_type_id: String,
    pub used_fallback: bool,
    pub before_from_count: u32,
    pub after_from_count: u32,
    pub after_added_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolContributionLedger {
    pub profile_id: String,
    pub candidates: Vec<RpgPoolContributionDecision>,
    pub grouped_die_deltas: BTreeMap<String, i32>,
    pub grouped_axis_values: BTreeMap<String, i32>,
    pub replacement_units: Vec<RpgPoolReplacementUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDamageResponseSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgDamageResponseEffect {
    Immune,
    Flat { value: i32 },
    Scale { numerator: u32, denominator: u32 },
}

impl RpgDamageResponseEffect {
    pub fn phase(&self) -> RpgDamageResponsePhase {
        match self {
            Self::Immune => RpgDamageResponsePhase::Immune,
            Self::Flat { .. } => RpgDamageResponsePhase::Flat,
            Self::Scale { .. } => RpgDamageResponsePhase::Scale,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDamageResponseDefinition {
    pub schema: RpgDamageResponseSchema,
    pub id: String,
    pub damage_type_id: String,
    pub required_tags: Vec<String>,
    pub bypass_tags: Vec<String>,
    pub effect: RpgDamageResponseEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgDamageResponsePhase {
    Immune,
    Flat,
    Scale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgDamageResponseDisposition {
    Applied,
    Inapplicable { reason: String },
    Suppressed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDamageResponseDecision {
    pub source_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
    pub response_id: String,
    pub phase: RpgDamageResponsePhase,
    pub damage_type_id: String,
    pub required_tags: Vec<String>,
    pub bypass_tags: Vec<String>,
    pub effect: RpgDamageResponseEffect,
    pub disposition: RpgDamageResponseDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDamageScaleStep {
    pub source_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
    pub response_id: String,
    pub numerator: u32,
    pub denominator: u32,
    pub before: i64,
    pub multiplied: i64,
    pub after_floor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDamagePartResolution {
    pub part_id: String,
    pub target_id: String,
    pub damage_type_id: String,
    pub tags: Vec<String>,
    pub random_evidence_path: String,
    pub original_amount: i32,
    pub response_candidates: Vec<RpgDamageResponseDecision>,
    pub flat_sum: i64,
    pub after_flat_before_clamp: i64,
    pub after_flat: i64,
    pub scale_steps: Vec<RpgDamageScaleStep>,
    pub final_amount: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPoolCancellationResult {
    pub cancellation_id: String,
    pub positive_axis_id: String,
    pub negative_axis_id: String,
    pub cancelled: i32,
    pub positive_remaining: i32,
    pub negative_remaining: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgOutcomeBandShiftSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgOutcomeBandShiftDefinition {
    pub schema: RpgOutcomeBandShiftSchema,
    pub id: String,
    pub profile: RpgOwnedRulesetReference,
    pub shift: i32,
    pub predicate: RpgContributionPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgOutcomeBandShiftDisposition {
    Applied,
    Inapplicable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgOutcomeBandShiftDecision {
    pub source_definition_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_instance_id: Option<String>,
    pub source_label: String,
    pub shift_id: String,
    pub profile_id: String,
    pub declared_shift: i32,
    pub applied_shift: i32,
    pub disposition: RpgOutcomeBandShiftDisposition,
    pub resulting_band_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgOutcomeBandShiftLedger {
    pub profile_id: String,
    pub starting_band_id: String,
    pub candidates: Vec<RpgOutcomeBandShiftDecision>,
    pub total_shift: i32,
    pub final_band_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgNaturalDieEffect {
    SetBand { band_id: String },
    Shift { amount: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgNaturalDieResolution {
    pub rule_id: String,
    pub effect: RpgNaturalDieEffect,
    pub resulting_band_id: String,
}

/// Rust-owned facts supplied by the encounter authority for contextual
/// contribution evaluation. The compiler/runtime remains usable without an
/// encounter host by passing the empty default context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RpgResolutionContext {
    pub entity_cell_capability_ids: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RpgDomainEvent {
    ActivationBudgetSpent {
        entity_id: String,
        budget_id: String,
        amount: i32,
        previous: i32,
        remaining: i32,
        accepted_activations: u32,
    },
    ActivationBudgetReset {
        entity_id: String,
        budget_id: String,
        previous: i32,
        current: i32,
    },
    RoundTransitioned {
        previous_round: u64,
        current_round: u64,
    },
    TurnTransitioned {
        previous_actor_id: String,
        current_actor_id: String,
        round: u64,
        turn: u64,
    },
    ResourceSpent {
        entity_id: String,
        resource_id: String,
        amount: i32,
        remaining: i32,
    },
    AttackResolved {
        actor_id: String,
        target_id: String,
        roll: u32,
        total: i32,
        defense_id: String,
        defense: i32,
        hit: bool,
        contribution_ledger: RpgScalarContributionLedger,
    },
    SavingThrowResolved {
        target_id: String,
        roll: u32,
        total: i32,
        difficulty: i32,
        saved: bool,
    },
    ScalarTestResolved {
        actor_id: String,
        target_id: String,
        profile_id: String,
        roll: u32,
        base_value: i32,
        contribution_ledger: RpgScalarContributionLedger,
        difficulty: i32,
        total: i32,
        margin: i32,
        base_band_id: String,
        natural_die_resolution: Option<RpgNaturalDieResolution>,
        band_shift_ledger: Box<RpgOutcomeBandShiftLedger>,
        final_band_id: String,
    },
    ScalarOutcomeBranchSelected {
        target_id: String,
        profile_id: String,
        final_band_id: String,
        selected_branch_id: String,
    },
    HeterogeneousPoolResolved {
        actor_id: String,
        target_id: String,
        profile_id: String,
        base_dice: BTreeMap<String, u32>,
        contribution_ledger: Box<RpgPoolContributionLedger>,
        frozen_dice: BTreeMap<String, u32>,
        evidence: Vec<RpgHeterogeneousRandomValue>,
        raw_axes: BTreeMap<String, i32>,
        automatic_axes: BTreeMap<String, i32>,
        cancellations: Vec<RpgPoolCancellationResult>,
        net_axes: BTreeMap<String, i32>,
        final_band_id: String,
    },
    VectorOutcomeBranchSelected {
        target_id: String,
        profile_id: String,
        final_band_id: String,
        selected_branch_id: String,
    },
    AreaTargetsDerived {
        actor_id: String,
        action_id: String,
        proposal_revision: u64,
        shape: RpgAreaShape,
        origin: RpgAreaOrigin,
        origin_cell_id: String,
        anchor_cell_id: String,
        included_cell_ids: Vec<String>,
        filtered_cells: Vec<RpgAreaFilteredCell>,
        included_participant_ids: Vec<String>,
        filtered_participants: Vec<RpgAreaFilteredParticipant>,
    },
    DamagePacketApplied {
        source_id: String,
        target_id: String,
        parts: Vec<RpgDamagePartResolution>,
        original_packet_sum: i64,
        adjusted_packet_sum: i64,
        bounded_vitality_delta: i32,
        actual_vitality_delta: i32,
        before_vitality: i32,
        after_vitality: i32,
    },
    HealingApplied {
        source_id: String,
        target_id: String,
        amount: i32,
        current_vitality: i32,
    },
    ResourceChanged {
        entity_id: String,
        resource_id: String,
        delta: i32,
        current: i32,
    },
    ModifierApplied {
        source_id: String,
        target_id: String,
        modifier_id: String,
        stacking_group: String,
        stacking: RpgModifierStackingPolicy,
        value: i32,
        remaining_turns: u32,
    },
    ModifierDurationChanged {
        target_id: String,
        modifier_id: String,
        stacking_group: String,
        remaining_turns: u32,
    },
    ModifierExpired {
        target_id: String,
        modifier_id: String,
        stacking_group: String,
    },
    EffectApplied {
        source_id: String,
        target_id: String,
        instance_id: String,
        definition_id: String,
        definition_version: u32,
        stacking_id: String,
        stacking: RpgEffectStackingPolicy,
        rank: i32,
        duration_anchor: RpgEffectDurationAnchor,
        remaining_count: u32,
        application_revision: u64,
        replaced_instance_ids: Vec<String>,
    },
    EffectRefreshed {
        source_id: String,
        target_id: String,
        instance_id: String,
        definition_id: String,
        definition_version: u32,
        stacking_id: String,
        stacking: RpgEffectStackingPolicy,
        rank: i32,
        duration_anchor: RpgEffectDurationAnchor,
        previous_count: u32,
        remaining_count: u32,
        application_revision: u64,
        removed_instance_ids: Vec<String>,
    },
    EffectRemoved {
        source_id: String,
        target_id: String,
        instance_id: String,
        definition_id: String,
        definition_version: u32,
        reason: String,
    },
    EffectDurationChanged {
        target_id: String,
        instance_id: String,
        definition_id: String,
        definition_version: u32,
        duration_anchor: RpgEffectDurationAnchor,
        previous_count: u32,
        remaining_count: u32,
    },
    EffectExpired {
        target_id: String,
        instance_id: String,
        definition_id: String,
        definition_version: u32,
        source_id: String,
        duration_anchor: RpgEffectDurationAnchor,
    },
    PositionChanged {
        source_id: String,
        entity_id: String,
        previous: GridPosition,
        current: GridPosition,
        provokes: bool,
    },
    ReactionOpened {
        reaction_id: String,
        actor_id: String,
        target_id: String,
        action_id: String,
    },
    ReactionResolved {
        reaction_id: String,
        option_id: Option<String>,
        damage_reduction: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgTraceStep {
    pub path: String,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgResolutionReceipt {
    pub action_id: String,
    pub actor_id: String,
    pub target_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_binding: Option<RpgIntentItemBinding>,
    pub events: Vec<RpgDomainEvent>,
    pub trace: Vec<RpgTraceStep>,
    pub random_evidence: Vec<RpgRandomEvidence>,
    pub random_consumed: u64,
    pub state_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgResolutionRejection {
    pub code: String,
    pub path: String,
    pub message: String,
    pub trace: Box<Vec<RpgTraceStep>>,
    pub random_evidence: Box<Vec<RpgRandomEvidence>>,
    pub random_attempted: u64,
    pub random_request: Option<Box<RpgRandomRequest>>,
    pub reaction_request: Option<Box<RpgReactionRequest>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_random_stream_advances_only_when_read() {
        let mut stream = DeterministicRandomStream::new(vec![17, 4]);
        assert_eq!(stream.consumed(), 0);
        assert_eq!(stream.take(), Some(17));
        assert_eq!(stream.consumed(), 1);
        assert_eq!(stream.remaining(), 1);
    }

    #[test]
    fn capability_state_has_explicit_entity_ownership() {
        let entity = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 2, y: 3 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(entity);

        assert_eq!(state.entity("hero").unwrap().vitality().current, 20);
        assert!(state.entity("missing").is_none());
    }

    #[test]
    fn capability_owner_applies_bounded_mutations() {
        let entity = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 2, y: 3 }, 20)
            .with_resource("focus", 2, 3);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(entity);

        assert_eq!(state.resources_owner().spend("hero", "focus", 1), Ok(1));
        assert_eq!(state.vitality_owner().apply_damage("hero", 7), Ok(13));
        assert_eq!(state.vitality_owner().apply_healing("hero", 20), Ok(20));
        assert_eq!(
            state.resources_owner().change("hero", "focus", 9),
            Err(RpgCapabilityMutationError::ResourceOutOfBounds)
        );
        assert_eq!(
            state
                .entity("hero")
                .unwrap()
                .resource("focus")
                .unwrap()
                .current,
            1
        );
        assert_eq!(
            state.modifiers_owner().apply(
                "hero",
                "impeded",
                "movement-control",
                RpgModifierStackingPolicy::Refresh,
                -2,
                1,
            ),
            Ok(())
        );
        assert_eq!(
            state.modifiers_owner().apply(
                "hero",
                "impeded",
                "movement-control",
                RpgModifierStackingPolicy::Refresh,
                -3,
                2,
            ),
            Ok(())
        );
        let modifier = state.entity("hero").unwrap().modifier("impeded").unwrap();
        assert_eq!(modifier.value(), -3);
        assert_eq!(modifier.remaining_turns(), 2);
        assert_eq!(
            state.position_owner().move_entity("hero", 2, -1, 3),
            Ok((GridPosition { x: 2, y: 3 }, GridPosition { x: 4, y: 2 }))
        );
        assert_eq!(
            state.position_owner().move_entity("hero", -9, 0, 9),
            Err(RpgCapabilityMutationError::PositionOutOfBounds)
        );
    }

    #[test]
    fn modifier_owner_ages_unchanged_tenure_and_expires_at_zero() {
        let entity = RpgEntityState::new("hero", Team::ally(), GridPosition { x: 0, y: 0 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(entity);
        assert_eq!(
            state.modifiers_owner().apply(
                "hero",
                "impeded",
                "movement-control",
                RpgModifierStackingPolicy::Refresh,
                -2,
                2,
            ),
            Ok(())
        );
        assert_eq!(
            state.modifiers_owner().apply(
                "hero",
                "invalid",
                "invalid",
                RpgModifierStackingPolicy::Replace,
                0,
                MAXIMUM_RPG_MODIFIER_TURNS + 1,
            ),
            Err(RpgCapabilityMutationError::ModifierTenureInvalid)
        );

        let refreshed_baseline = state.clone();
        assert_eq!(
            state.modifiers_owner().apply(
                "hero",
                "impeded",
                "movement-control",
                RpgModifierStackingPolicy::Refresh,
                -2,
                2,
            ),
            Ok(())
        );
        let refreshed = BTreeSet::from([("hero".to_owned(), "movement-control".to_owned())]);
        assert!(state
            .modifiers_owner()
            .advance_turn(&refreshed_baseline, &refreshed)
            .is_empty());
        assert_eq!(
            state
                .entity("hero")
                .unwrap()
                .modifier("impeded")
                .unwrap()
                .remaining_turns(),
            2
        );

        let first_baseline = state.clone();
        assert_eq!(
            state
                .modifiers_owner()
                .advance_turn(&first_baseline, &BTreeSet::new()),
            vec![RpgModifierTurnChange::Aged {
                entity_id: "hero".to_owned(),
                stacking_group: "movement-control".to_owned(),
                modifier_id: "impeded".to_owned(),
                remaining_turns: 1,
            }]
        );
        assert_eq!(
            state
                .entity("hero")
                .unwrap()
                .modifier("impeded")
                .unwrap()
                .remaining_turns(),
            1
        );

        let second_baseline = state.clone();
        assert_eq!(
            state
                .modifiers_owner()
                .advance_turn(&second_baseline, &BTreeSet::new()),
            vec![RpgModifierTurnChange::Expired {
                entity_id: "hero".to_owned(),
                stacking_group: "movement-control".to_owned(),
                modifier_id: "impeded".to_owned(),
            }]
        );
        assert!(state.entity("hero").unwrap().modifier("impeded").is_none());
        assert_eq!(
            ActiveRpgModifier::restore("impeded", -2, 0),
            Err(RpgStateRestoreError::ValueOutOfBounds)
        );
    }
}
