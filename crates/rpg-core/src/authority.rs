use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{BoundedValue, GridPosition, Team};

pub const MAXIMUM_RPG_MODIFIER_TURNS: u32 = 1_000;

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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgEntityState {
    id: String,
    team: Team,
    position: GridPosition,
    vitality: BoundedValue,
    stats: BTreeMap<String, i32>,
    defenses: BTreeMap<String, i32>,
    resources: BTreeMap<String, BoundedValue>,
    modifiers: BTreeMap<String, ActiveRpgModifier>,
}

impl RpgEntityState {
    pub fn new(id: impl Into<String>, team: Team, position: GridPosition, vitality: i32) -> Self {
        Self {
            id: id.into(),
            team,
            position,
            vitality: BoundedValue {
                current: vitality,
                max: vitality,
            },
            stats: BTreeMap::new(),
            defenses: BTreeMap::new(),
            resources: BTreeMap::new(),
            modifiers: BTreeMap::new(),
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
            vitality,
            stats: BTreeMap::new(),
            defenses: BTreeMap::new(),
            resources: BTreeMap::new(),
            modifiers: BTreeMap::new(),
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
}

impl RpgCapabilityState {
    pub fn restore(
        revision: u64,
        entities: impl IntoIterator<Item = RpgEntityState>,
    ) -> Result<Self, RpgStateRestoreError> {
        let mut restored = Self {
            revision,
            entities: BTreeMap::new(),
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

    pub fn entity(&self, id: &str) -> Option<&RpgEntityState> {
        self.entities.get(id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn entities(&self) -> impl Iterator<Item = &RpgEntityState> {
        self.entities.values()
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
    InvalidAmount,
    InsufficientResource,
    ResourceOutOfBounds,
    ModifierTenureInvalid,
    MovementDistanceInvalid,
    PositionOutOfBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIntent {
    pub action_id: String,
    pub actor_id: String,
    pub target_ids: Vec<String>,
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
    FormulaDice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomRequest {
    pub kind: RpgRandomRequestKind,
    pub count: u32,
    pub sides: u32,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgRandomEvidence {
    pub request: RpgRandomRequest,
    pub values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReactionOption {
    pub id: String,
    pub label: String,
    pub damage_reduction: u32,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RpgDomainEvent {
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
    },
    SavingThrowResolved {
        target_id: String,
        roll: u32,
        total: i32,
        difficulty: i32,
        saved: bool,
    },
    DamageApplied {
        source_id: String,
        target_id: String,
        amount: i32,
        damage_type: String,
        remaining_vitality: i32,
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
