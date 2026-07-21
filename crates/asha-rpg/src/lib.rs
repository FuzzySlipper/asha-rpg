//! Supported public facade for portable RPG authority.
//!
//! Rust owns Ruleset semantics, compiled PlayBundle validation, and the
//! deterministic authority loop. Downstream games own immutable Content Pack,
//! PlayBundle, and Scenario declarations plus product workflows.
//!
//! The supported path is one Ruleset plus selected Content Packs into a closed
//! PlayBundle artifact, then an artifact-bound Scenario into an authority
//! Session.
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
    CompiledContentPolicyBinding, CompiledParticipantProfile, CompiledPlayBundleArtifact,
    ContentConflictPolicy, ContentDefinitionCommitment, ContentDefinitionProvenance,
    ContentDerivationMixinProvenance, ContentDerivationProvenance, ContentExtensionPolicy,
    ContentImpactPlane, ContentMaterializationStage, ContentMaterializationValue,
    ContentMixinDefinitionCommitmentValue, ContentMixinParameterCommitment,
    ContentMixinParameterType, ContentOverlayProvenance, ContentPackDependencyLockEntry,
    ContentPackDependencyRelationship, ContentPackRequirements, ContentPatch,
    ContentPatchChangeProvenance, ContentPatchMemberKey, ContentPatchMemberSelector,
    ContentPatchOperation, ContentPatchPathSegment, ContentPatchPosition, ContentRelationshipKind,
    ContentRelationshipProvenance, ContentSourceLocation, ContentValueRequirement,
    MaterializedContentDefinition, MaterializedContentDefinitionKind,
    MaterializedContentVisibility, MaterializedParticipantProfileData, NormalizedRpgIr,
    ParticipantProfileBoundedValue, ParticipantProfileInitialCapability, ParticipantProfileRole,
    ParticipantProfileSchema, PlayBundleArtifactSchema, PlayBundleFingerprints, PreparedPlayBundle,
    ResolvedContentPack, RpgIrAction, RpgIrCatalogs, RpgIrCheck, RpgIrComparison, RpgIrFormula,
    RpgIrOperation, RpgIrPackage, RpgIrPredicate, RpgIrProgram, RpgIrReactionOption,
    RpgIrRequirement, RpgIrRequirementKind, RpgIrResourceCost, RpgIrRollScope, RpgIrSchema,
    RpgIrStackingPolicy, RpgIrSubject, RpgIrTargetSelector, RpgIrTeamConstraint,
    RpgVersionedIdentity, Ruleset, RulesetModels, RulesetNumericDomain, RulesetProvisions,
    RulesetSchema, RulesetValueContract, RulesetValueExpression, RulesetValueFormula,
    RulesetValueFormulaSchema, RulesetValueKind, RulesetValueSource, VersionedRpgRequirement,
    COMPILED_PLAY_BUNDLE_IDENTITY, PARTICIPANT_PROFILE_IDENTITY, PARTICIPANT_PROFILE_VERSION,
    PLAY_BUNDLE_ARTIFACT_MAJOR, PREPARED_PLAY_BUNDLE_IDENTITY, RPG_IR_IDENTITY, RPG_IR_MAJOR,
};
pub use rpg_runtime::*;
