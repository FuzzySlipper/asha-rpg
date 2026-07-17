#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgOperationRegistration {
    pub id: &'static str,
    pub version: u32,
    pub reads: &'static [&'static str],
    pub mutation_owner: &'static str,
    pub accepted_events: &'static [&'static str],
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
];

pub fn operation_registrations() -> &'static [RpgOperationRegistration] {
    REGISTRATIONS
}

pub(crate) fn operation_registration(id: &str) -> Option<&'static RpgOperationRegistration> {
    REGISTRATIONS
        .iter()
        .find(|registration| registration.id == id)
}

pub(crate) fn capability_version(id: &str) -> Option<u32> {
    match id {
        "capability.vitality"
        | "capability.stats"
        | "capability.defenses"
        | "capability.resources"
        | "capability.modifiers"
        | "capability.random" => Some(1),
        _ => None,
    }
}
