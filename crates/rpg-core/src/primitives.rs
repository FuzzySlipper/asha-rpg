use serde::{Deserialize, Serialize};

/// A current value constrained by its declared maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedValue {
    pub current: i32,
    pub max: i32,
}

/// A stable identifier, display label, and integer value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedNumber {
    pub id: String,
    pub label: String,
    pub value: i32,
}

/// A position on a rectangular combat grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GridPosition {
    pub x: u32,
    pub y: u32,
}

/// The side a combat participant belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Team {
    Ally,
    Enemy,
}

/// A deterministic, non-cryptographic state identity emitted by Rust authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateFingerprint {
    pub algorithm: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::{BoundedValue, GridPosition, NamedNumber, StateFingerprint, Team};

    #[test]
    fn core_values_preserve_equality_and_copy_semantics() {
        let hit_points = BoundedValue {
            current: 7,
            max: 12,
        };
        let position = GridPosition { x: 3, y: 5 };

        assert_eq!(
            hit_points,
            BoundedValue {
                current: 7,
                max: 12
            }
        );
        assert_eq!(position, GridPosition { x: 3, y: 5 });
        assert_eq!(Team::Ally, Team::Ally);
        assert_ne!(Team::Ally, Team::Enemy);
    }

    #[test]
    fn named_numbers_and_fingerprints_include_all_authoritative_fields() {
        let mind = NamedNumber {
            id: "mind".to_string(),
            label: "Mind".to_string(),
            value: 3,
        };
        let state = StateFingerprint {
            algorithm: "fnv1a64.rpg-state.v0".to_string(),
            value: "cafe".to_string(),
        };

        assert_eq!(mind.value, 3);
        assert_ne!(
            state,
            StateFingerprint {
                algorithm: "another-algorithm".to_string(),
                value: "cafe".to_string(),
            }
        );
    }
}
