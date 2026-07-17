#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgOperationRegistration {
    pub id: &'static str,
    pub version: u32,
    pub reads: &'static [&'static str],
    pub mutation_owner: &'static str,
    pub validation_behavior: &'static str,
    pub accepted_events: &'static [&'static str],
    pub trace_behavior: &'static str,
    pub replay_implications: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgCapabilityRegistration {
    pub id: &'static str,
    pub version: u32,
}

const REGISTRATIONS: &[RpgOperationRegistration] = &[
    RpgOperationRegistration {
        id: "operation.damage",
        version: 1,
        reads: &["capability.vitality"],
        mutation_owner: "capability.vitality",
        validation_behavior: "Evaluate a bounded amount and require the vitality owner to accept the target transition.",
        accepted_events: &["DamageApplied"],
        trace_behavior: "Record the operation path, evaluated amount, and committed vitality transition.",
        replay_implications: "Replay consumes the same formula randomness and verifies the accepted DamageApplied event.",
    },
    RpgOperationRegistration {
        id: "operation.heal",
        version: 1,
        reads: &["capability.vitality"],
        mutation_owner: "capability.vitality",
        validation_behavior: "Evaluate a bounded amount and require the vitality owner to accept the target transition.",
        accepted_events: &["HealingApplied"],
        trace_behavior: "Record the operation path, evaluated amount, and committed vitality transition.",
        replay_implications: "Replay consumes the same formula randomness and verifies the accepted HealingApplied event.",
    },
    RpgOperationRegistration {
        id: "operation.changeResource",
        version: 1,
        reads: &["capability.resources"],
        mutation_owner: "capability.resources",
        validation_behavior: "Resolve the declared subject and resource, then reject an out-of-bounds resource transition.",
        accepted_events: &["ResourceChanged"],
        trace_behavior: "Record the operation path, resource identity, delta, and committed bounds-preserving transition.",
        replay_implications: "Replay verifies the accepted ResourceChanged event and resulting bounded resource view.",
    },
    RpgOperationRegistration {
        id: "operation.applyModifier",
        version: 1,
        reads: &["capability.modifiers"],
        mutation_owner: "capability.modifiers",
        validation_behavior: "Resolve the declared modifier and require valid duration and closed stacking policy data.",
        accepted_events: &["ModifierApplied"],
        trace_behavior: "Record the operation path, modifier identity, duration, stacking decision, and commit.",
        replay_implications: "Replay verifies the accepted ModifierApplied event and deterministic modifier view.",
    },
    RpgOperationRegistration {
        id: "operation.move",
        version: 1,
        reads: &["capability.position"],
        mutation_owner: "capability.position",
        validation_behavior: "Resolve a bounded destination and reject moves beyond the declared maximum distance.",
        accepted_events: &["PositionChanged"],
        trace_behavior: "Record the operation path, origin, destination, distance validation, and commit.",
        replay_implications: "Replay verifies the accepted PositionChanged event and resulting position view.",
    },
];

const CAPABILITIES: &[RpgCapabilityRegistration] = &[
    RpgCapabilityRegistration {
        id: "capability.vitality",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.stats",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.defenses",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.resources",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.modifiers",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.position",
        version: 1,
    },
    RpgCapabilityRegistration {
        id: "capability.random",
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
        .find(|registration| registration.id == id)
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
            assert!(capabilities.contains(registration.mutation_owner));
            assert!(!registration.validation_behavior.trim().is_empty());
            assert!(!registration.accepted_events.is_empty());
            assert!(!registration.trace_behavior.trim().is_empty());
            assert!(!registration.replay_implications.trim().is_empty());
        }
    }
}
