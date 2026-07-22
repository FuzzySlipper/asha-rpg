use rpg_core::RpgCapabilityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgOperationRegistration {
    pub id: &'static str,
    pub version: u32,
    pub reads: &'static [RpgCapabilityId],
    pub mutation_owner: RpgCapabilityId,
    pub validation_behavior: &'static str,
    pub accepted_events: &'static [&'static str],
    pub trace_behavior: &'static str,
    pub replay_implications: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpgCapabilityOwnerMismatch {
    pub registered: RpgCapabilityId,
    pub required: RpgCapabilityId,
}

impl RpgOperationRegistration {
    pub fn bind_mutation_owner(
        &self,
        required: RpgCapabilityId,
    ) -> Result<RpgCapabilityId, RpgCapabilityOwnerMismatch> {
        if self.mutation_owner == required {
            return Ok(required);
        }
        Err(RpgCapabilityOwnerMismatch {
            registered: self.mutation_owner,
            required,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgCapabilityRegistration {
    pub id: RpgCapabilityId,
    pub version: u32,
}

const REGISTRATIONS: &[RpgOperationRegistration] = &[
    RpgOperationRegistration {
        id: "operation.damage",
        version: 1,
        reads: &[RpgCapabilityId::Vitality],
        mutation_owner: RpgCapabilityId::Vitality,
        validation_behavior: "Evaluate a bounded amount and require the vitality owner to accept the target transition.",
        accepted_events: &["DamageApplied"],
        trace_behavior: "Record the operation path, evaluated amount, and committed vitality transition.",
        replay_implications: "Replay consumes the same formula randomness and verifies the accepted DamageApplied event.",
    },
    RpgOperationRegistration {
        id: "operation.heal",
        version: 1,
        reads: &[RpgCapabilityId::Vitality],
        mutation_owner: RpgCapabilityId::Vitality,
        validation_behavior: "Evaluate a bounded amount and require the vitality owner to accept the target transition.",
        accepted_events: &["HealingApplied"],
        trace_behavior: "Record the operation path, evaluated amount, and committed vitality transition.",
        replay_implications: "Replay consumes the same formula randomness and verifies the accepted HealingApplied event.",
    },
    RpgOperationRegistration {
        id: "operation.changeResource",
        version: 1,
        reads: &[RpgCapabilityId::Resources],
        mutation_owner: RpgCapabilityId::Resources,
        validation_behavior: "Resolve the declared subject and resource, then reject an out-of-bounds resource transition.",
        accepted_events: &["ResourceChanged"],
        trace_behavior: "Record the operation path, resource identity, delta, and committed bounds-preserving transition.",
        replay_implications: "Replay verifies the accepted ResourceChanged event and resulting bounded resource view.",
    },
    RpgOperationRegistration {
        id: "operation.applyModifier",
        version: 1,
        reads: &[RpgCapabilityId::Modifiers],
        mutation_owner: RpgCapabilityId::Modifiers,
        validation_behavior: "Resolve the declared modifier and require valid duration and closed stacking policy data.",
        accepted_events: &["ModifierApplied"],
        trace_behavior: "Record the operation path, modifier identity, duration, stacking decision, and commit.",
        replay_implications: "Replay verifies the accepted ModifierApplied event and deterministic modifier view.",
    },
    RpgOperationRegistration {
        id: "operation.move",
        version: 1,
        reads: &[RpgCapabilityId::Position],
        mutation_owner: RpgCapabilityId::Position,
        validation_behavior: "Resolve a bounded destination and reject moves beyond the declared maximum distance.",
        accepted_events: &["PositionChanged"],
        trace_behavior: "Record the operation path, origin, destination, distance validation, and commit.",
        replay_implications: "Replay verifies the accepted PositionChanged event and resulting position view.",
    },
    RpgOperationRegistration {
        id: "operation.moveToCell",
        version: 1,
        reads: &[RpgCapabilityId::Position],
        mutation_owner: RpgCapabilityId::Position,
        validation_behavior: "Resolve one authority-bound cell destination and reject movement beyond the declared maximum distance.",
        accepted_events: &["PositionChanged"],
        trace_behavior: "Record the selected cell, origin, destination, distance validation, and commit.",
        replay_implications: "Replay verifies the selected cell binding, accepted PositionChanged event, and resulting position view.",
    },
    RpgOperationRegistration {
        id: "operation.openReaction",
        version: 1,
        reads: &[RpgCapabilityId::Reactions],
        mutation_owner: RpgCapabilityId::Reactions,
        validation_behavior: "Suspend the staged command at the declared reaction window and accept only one typed option before resuming the same transaction.",
        accepted_events: &["ReactionOpened", "ReactionResolved"],
        trace_behavior: "Record the reaction identity, selected option, and bounded damage adjustment in the resumed transaction.",
        replay_implications: "Replay must supply the same typed reaction decision before the command can commit.",
    },
];

const CAPABILITIES: &[RpgCapabilityRegistration] = &[
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Vitality,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Stats,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Defenses,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Resources,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Modifiers,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Position,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Random,
        version: 1,
    },
    RpgCapabilityRegistration {
        id: RpgCapabilityId::Reactions,
        version: 1,
    },
];

pub fn operation_registrations() -> &'static [RpgOperationRegistration] {
    REGISTRATIONS
}

pub fn capability_registrations() -> &'static [RpgCapabilityRegistration] {
    CAPABILITIES
}

pub(crate) fn operation_registration(id: &str) -> Option<&'static RpgOperationRegistration> {
    REGISTRATIONS
        .iter()
        .find(|registration| registration.id == id)
}

pub(crate) fn capability_version(id: &str) -> Option<u32> {
    CAPABILITIES
        .iter()
        .find(|registration| registration.id.as_str() == id)
        .map(|registration| registration.version)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_operation_declares_the_complete_rust_first_extension_contract() {
        let capabilities = CAPABILITIES
            .iter()
            .map(|registration| registration.id)
            .collect::<BTreeSet<_>>();
        let mut operation_ids = BTreeSet::new();

        for registration in REGISTRATIONS {
            assert!(operation_ids.insert(registration.id));
            assert!(!registration.reads.is_empty());
            assert!(registration
                .reads
                .iter()
                .all(|capability| capabilities.contains(capability)));
            assert!(capabilities.contains(&registration.mutation_owner));
            assert!(!registration.validation_behavior.trim().is_empty());
            assert!(!registration.accepted_events.is_empty());
            assert!(!registration.trace_behavior.trim().is_empty());
            assert!(!registration.replay_implications.trim().is_empty());
        }
    }

    #[test]
    fn mismatched_mutation_owner_registration_is_rejected() {
        let registration = RpgOperationRegistration {
            id: "operation.invalidDamage",
            version: 1,
            reads: &[RpgCapabilityId::Vitality],
            mutation_owner: RpgCapabilityId::Resources,
            validation_behavior: "test registration",
            accepted_events: &["DamageApplied"],
            trace_behavior: "test registration",
            replay_implications: "test registration",
        };

        assert_eq!(
            registration.bind_mutation_owner(RpgCapabilityId::Vitality),
            Err(RpgCapabilityOwnerMismatch {
                registered: RpgCapabilityId::Resources,
                required: RpgCapabilityId::Vitality,
            })
        );
    }
}
