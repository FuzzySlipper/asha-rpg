//! Strict compilation and deterministic execution for normalized RPG IR.
//!
//! Decoded declarations are never executable authority. Compilation validates
//! compatibility, requirements, references, bounded composition, and atomic
//! ownership before producing opaque compiled rules or a closed PlayBundle.

#![forbid(unsafe_code)]

mod artifact;
mod compile;
mod diagnostic;
mod execute;
mod registry;

pub use artifact::{
    compile_prepared_play_bundle, compile_prepared_play_bundle_json, load_compiled_play_bundle,
    load_compiled_play_bundle_json, materialized_definition_fingerprint, CompiledPlayBundle,
    CompiledRulesetValuePlan, RulesetValueEvaluationFailure, RulesetValueKey,
};
pub use compile::{
    compile_normalized_rpg_ir, compile_normalized_rpg_json, CompiledRpgAction, CompiledRpgRules,
    RpgRandomPlanCondition, RpgRandomPlanConditionKind, RpgRandomPlanEntry,
};
pub use diagnostic::{RpgCompileFailure, RpgDiagnostic, RpgDiagnosticSeverity, RpgDiagnosticStage};
pub use registry::{
    capability_registrations, operation_registrations, RpgCapabilityRegistration,
    RpgOperationRegistration,
};
