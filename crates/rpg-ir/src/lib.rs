//! Portable normalized RPG language, Ruleset, and compiled PlayBundle contracts.
//!
//! This crate exposes the two data representations that cross the Rust
//! authority boundary. It does not expose an alternate provider, module, or
//! action-definition model.

mod normalized;
mod play_bundle_artifact;

pub use normalized::*;
pub use play_bundle_artifact::*;
