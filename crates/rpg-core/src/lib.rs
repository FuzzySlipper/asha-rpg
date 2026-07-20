//! Shared authority primitives.
//!
//! This crate owns dependency-free values, positions, team classification, and
//! state fingerprint vocabulary. Rule-specific events and traces remain in the
//! ruleset/combat layers until their fact vocabulary is independent of action
//! resolution.

mod authority;
mod primitives;

pub use authority::*;
pub use primitives::{BoundedValue, GridPosition, NamedNumber, RpgTeamId, StateFingerprint, Team};
