//! Portable normalized RPG language and compiled ruleset artifact contracts.
//!
//! This crate exposes the two data representations that cross the Rust
//! authority boundary. It does not expose an alternate provider, module, or
//! action-definition model.

mod normalized;
mod ruleset_artifact;

pub use normalized::*;
pub use ruleset_artifact::*;
