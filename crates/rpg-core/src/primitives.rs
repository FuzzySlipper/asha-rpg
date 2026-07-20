use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GridPosition {
    pub x: u32,
    pub y: u32,
}

/// An open, stable team identity used for ally/hostile comparisons.
///
/// The two constructors preserve the initial profile's familiar identities while
/// `named` allows downstream rulesets to use any stable taxonomy without a
/// Rust enum change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpgTeamId(String);

impl RpgTeamId {
    pub fn named(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn ally() -> Self {
        Self::named("team.ally")
    }

    pub fn enemy() -> Self {
        Self::named("team.enemy")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RpgTeamId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for RpgTeamId {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RpgTeamId {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let id = String::deserialize(deserializer)?;
        if id.trim().is_empty() {
            return Err(de::Error::custom("team identity must not be empty"));
        }
        Ok(Self::named(id))
    }
}

/// Compatibility alias for the pre-setup public name.
pub type Team = RpgTeamId;

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
        assert_eq!(Team::ally(), Team::ally());
        assert_ne!(Team::ally(), Team::enemy());
        assert_eq!(Team::named("team.azure").as_str(), "team.azure");
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
