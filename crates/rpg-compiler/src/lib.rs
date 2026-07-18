//! Strict compilation and deterministic execution for normalized RPG IR.
//!
//! Decoded declarations are never executable authority. Compilation validates
//! compatibility, requirements, references, bounded composition, and atomic
//! ownership before producing an opaque ruleset.

#![forbid(unsafe_code)]

mod artifact;
mod compile;
mod diagnostic;
mod execute;
mod registry;

pub use artifact::{
    compile_prepared_ruleset, compile_prepared_ruleset_json, load_compiled_ruleset_artifact,
    load_compiled_ruleset_artifact_json, CompiledRulesetBundle,
};
pub use compile::{
    compile_normalized_rpg_ir, compile_normalized_rpg_json, CompiledRpgAction, CompiledRpgRuleset,
    RpgRandomPlanCondition, RpgRandomPlanConditionKind, RpgRandomPlanEntry,
};
pub use diagnostic::{RpgCompileFailure, RpgDiagnostic, RpgDiagnosticSeverity, RpgDiagnosticStage};
pub use registry::{
    capability_registrations, operation_registrations, RpgCapabilityRegistration,
    RpgOperationRegistration,
};
