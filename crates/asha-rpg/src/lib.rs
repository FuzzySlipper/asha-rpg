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
    CompiledContentPolicyBinding, CompiledPlayBundleArtifact, ContentConflictPolicy,
    ContentDefinitionCommitment, ContentDefinitionProvenance, ContentDerivationMixinProvenance,
    ContentDerivationProvenance, ContentExtensionPolicy, ContentImpactPlane,
    ContentMaterializationStage, ContentMaterializationValue,
    ContentMixinDefinitionCommitmentValue, ContentMixinParameterCommitment,
    ContentMixinParameterType, ContentOverlayProvenance, ContentPackDependencyLockEntry,
    ContentPackDependencyRelationship, ContentPackRequirements, ContentPatch,
    ContentPatchChangeProvenance, ContentPatchMemberKey, ContentPatchMemberSelector,
    ContentPatchOperation, ContentPatchPathSegment, ContentPatchPosition, ContentRelationshipKind,
    ContentRelationshipProvenance, ContentSourceLocation, ContentValueRequirement,
    MaterializedContentDefinition, MaterializedContentDefinitionKind,
    MaterializedContentVisibility, NormalizedRpgIr, PlayBundleArtifactSchema,
    PlayBundleFingerprints, PreparedPlayBundle, ResolvedContentPack, RpgIrAction, RpgIrCatalogs,
    RpgIrCheck, RpgIrComparison, RpgIrFormula, RpgIrOperation, RpgIrPackage, RpgIrPredicate,
    RpgIrProgram, RpgIrReactionOption, RpgIrRequirement, RpgIrRequirementKind, RpgIrResourceCost,
    RpgIrRollScope, RpgIrSchema, RpgIrStackingPolicy, RpgIrSubject, RpgIrTargetSelector,
    RpgIrTeamConstraint, RpgVersionedIdentity, Ruleset, RulesetModels, RulesetNumericDomain,
    RulesetProvisions, RulesetSchema, RulesetValueContract, RulesetValueKind,
    VersionedRpgRequirement, COMPILED_PLAY_BUNDLE_IDENTITY, PLAY_BUNDLE_ARTIFACT_MAJOR,
    PREPARED_PLAY_BUNDLE_IDENTITY, RPG_IR_IDENTITY, RPG_IR_MAJOR,
};
pub use rpg_runtime::*;
