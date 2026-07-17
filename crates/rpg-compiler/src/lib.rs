//! Strict compilation and deterministic execution for normalized RPG IR.
//!
//! Decoded declarations are never executable authority. Compilation validates
//! compatibility, requirements, references, bounded composition, and atomic
//! ownership before producing an opaque ruleset.

#![forbid(unsafe_code)]

mod compile;
mod diagnostic;
mod execute;
mod registry;

pub use compile::{compile_normalized_rpg_ir, compile_normalized_rpg_json, CompiledRpgRuleset};
pub use diagnostic::{RpgCompileFailure, RpgDiagnostic, RpgDiagnosticSeverity, RpgDiagnosticStage};
pub use registry::{operation_registrations, RpgOperationRegistration};
