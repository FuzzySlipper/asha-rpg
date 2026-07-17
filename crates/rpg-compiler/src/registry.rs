#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgOperationRegistration {
    pub id: &'static str,
    pub version: u32,
    pub reads: &'static [&'static str],
    pub mutation_owner: &'static str,
    pub accepted_events: &'static [&'static str],
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
        accepted_events: &["DamageApplied"],
    },
    RpgOperationRegistration {
        id: "operation.heal",
        version: 1,
        reads: &["capability.vitality"],
        mutation_owner: "capability.vitality",
        accepted_events: &["HealingApplied"],
    },
    RpgOperationRegistration {
        id: "operation.changeResource",
        version: 1,
        reads: &["capability.resources"],
        mutation_owner: "capability.resources",
        accepted_events: &["ResourceChanged"],
    },
    RpgOperationRegistration {
        id: "operation.applyModifier",
        version: 1,
        reads: &["capability.modifiers"],
        mutation_owner: "capability.modifiers",
        accepted_events: &["ModifierApplied"],
    },
    RpgOperationRegistration {
        id: "operation.move",
        version: 1,
        reads: &["capability.position"],
        mutation_owner: "capability.position",
        accepted_events: &["PositionChanged"],
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
