use std::collections::BTreeMap;

use crate::{BoundedValue, GridPosition, Team};

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

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn team(&self) -> Team {
        self.team
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRpgModifier {
    id: String,
    value: i32,
    remaining_turns: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpgModifierStackingPolicy {
    Replace,
    Refresh,
}

impl ActiveRpgModifier {
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
    pub fn entity(&self, id: &str) -> Option<&RpgEntityState> {
        self.entities.get(id)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn insert_entity(&mut self, entity: RpgEntityState) -> Option<RpgEntityState> {
        self.entities.insert(entity.id.clone(), entity)
    }

    pub fn spend_resource(
        &mut self,
        entity_id: &str,
        resource_id: &str,
        amount: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        if amount <= 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let resource = self.resource_mut(entity_id, resource_id)?;
        if resource.current < amount {
            return Err(RpgCapabilityMutationError::InsufficientResource);
        }
        resource.current -= amount;
        Ok(resource.current)
    }

    pub fn apply_damage(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        if amount < 0 {
            return Err(RpgCapabilityMutationError::InvalidAmount);
        }
        let entity = self.entity_mut_for_owner(entity_id)?;
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
        let entity = self.entity_mut_for_owner(entity_id)?;
        entity.vitality.current = entity
            .vitality
            .current
            .saturating_add(amount)
            .min(entity.vitality.max);
        Ok(entity.vitality.current)
    }

    pub fn change_resource(
        &mut self,
        entity_id: &str,
        resource_id: &str,
        delta: i32,
    ) -> Result<i32, RpgCapabilityMutationError> {
        let resource = self.resource_mut(entity_id, resource_id)?;
        resource.current = resource
            .current
            .saturating_add(delta)
            .clamp(0, resource.max);
        Ok(resource.current)
    }

    pub fn apply_modifier(
        &mut self,
        entity_id: &str,
        modifier_id: &str,
        stacking_group: &str,
        stacking: RpgModifierStackingPolicy,
        value: i32,
        remaining_turns: u32,
    ) -> Result<(), RpgCapabilityMutationError> {
        let entity = self.entity_mut_for_owner(entity_id)?;
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
pub enum RpgCapabilityMutationError {
    UnknownEntity,
    UnknownResource,
    InvalidAmount,
    InsufficientResource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgTraceStep {
    pub path: String,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgResolutionReceipt {
    pub action_id: String,
    pub actor_id: String,
    pub target_ids: Vec<String>,
    pub events: Vec<RpgDomainEvent>,
    pub trace: Vec<RpgTraceStep>,
    pub random_consumed: usize,
    pub state_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgResolutionRejection {
    pub code: String,
    pub path: String,
    pub message: String,
    pub trace: Vec<RpgTraceStep>,
    pub random_attempted: usize,
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
        let entity = RpgEntityState::new("hero", Team::Ally, GridPosition { x: 2, y: 3 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(entity);

        assert_eq!(state.entity("hero").unwrap().vitality().current, 20);
        assert!(state.entity("missing").is_none());
    }

    #[test]
    fn capability_owner_applies_bounded_mutations() {
        let entity = RpgEntityState::new("hero", Team::Ally, GridPosition { x: 2, y: 3 }, 20)
            .with_resource("focus", 2, 3);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(entity);

        assert_eq!(state.spend_resource("hero", "focus", 1), Ok(1));
        assert_eq!(state.apply_damage("hero", 7), Ok(13));
        assert_eq!(state.apply_healing("hero", 20), Ok(20));
        assert_eq!(state.change_resource("hero", "focus", 9), Ok(3));
        assert_eq!(
            state.apply_modifier(
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
            state.apply_modifier(
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
    }
}
