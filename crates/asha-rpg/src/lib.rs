//! Supported public facade for portable RPG authority.
//!
//! Rust owns normalized semantic declarations and the deterministic authority
//! loop. Downstream games own authored content and product workflows.
//!
//! The only supported ruleset path is normalized RPG IR to a compiled artifact
//! to an artifact-bound authority session.
//!
//! Removed predecessor declarations are intentionally unavailable:
//!
//! ```compile_fail
//! use asha_rpg::RulesetMetadata;
//! ```
//!
//! ```compile_fail
//! use asha_rpg::RulesetProviderCatalog;
//! ```
//!
//! ```compile_fail
//! use asha_rpg::{AbilityDefinitionKind, ActionResourceKind};
//! ```

#![forbid(unsafe_code)]

pub use rpg_compiler::*;
pub use rpg_core::*;
pub use rpg_ir::{
    CompiledRulesetArtifact, CompiledRulesetIdentity, CompiledRulesetPolicyBinding,
    MaterializedRulesetDefinition, MaterializedRulesetDefinitionKind,
    MaterializedRulesetVisibility, NormalizedRpgIr, PreparedRulesetCompilation,
    ResolvedRulesetSourcePackage, RpgIrAction, RpgIrCatalogs, RpgIrCheck, RpgIrComparison,
    RpgIrFormula, RpgIrOperation, RpgIrPackage, RpgIrPredicate, RpgIrProgram, RpgIrReactionOption,
    RpgIrRequirement, RpgIrRequirementKind, RpgIrResourceCost, RpgIrRollScope, RpgIrSchema,
    RpgIrStackingPolicy, RpgIrSubject, RpgIrTargetSelector, RpgIrTeamConstraint,
    RulesetArtifactFingerprints, RulesetArtifactSchema, RulesetConflictPolicy,
    RulesetDefinitionProvenance, RulesetDependencyLockEntry, RulesetDependencyRelationship,
    RulesetDerivationMixinProvenance, RulesetDerivationProvenance, RulesetExtensionPolicy,
    RulesetImpactPlane, RulesetMaterializationStage, RulesetMaterializationValue,
    RulesetOverlayProvenance, RulesetPatch, RulesetPatchChangeProvenance, RulesetPatchMemberKey,
    RulesetPatchMemberSelector, RulesetPatchOperation, RulesetPatchPathSegment,
    RulesetPatchPosition, RulesetRelationshipKind, RulesetRelationshipProvenance,
    RulesetSourceLocation, VersionedRulesetRequirement, COMPILED_RULESET_IDENTITY,
    PREPARED_RULESET_IDENTITY, RPG_IR_IDENTITY, RPG_IR_MAJOR, RULESET_ARTIFACT_MAJOR,
};
pub use rpg_runtime::*;
