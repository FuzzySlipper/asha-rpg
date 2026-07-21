use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PREPARED_PLAY_BUNDLE_IDENTITY: &str = "asha.rpg.play-bundle.prepared";
pub const COMPILED_PLAY_BUNDLE_IDENTITY: &str = "asha.rpg.play-bundle.compiled";
pub const PLAY_BUNDLE_ARTIFACT_MAJOR: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayBundleArtifactSchema {
    pub identity: String,
    pub major: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgVersionedIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetSchema {
    pub identity: String,
    pub major: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetValueKind {
    Defense,
    Stat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetValueFormulaSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RulesetValueExpression {
    Constant {
        value: i64,
    },
    ReadValue {
        ruleset_id: String,
        value_kind: RulesetValueKind,
        value_id: String,
    },
    Subtract {
        minuend: Box<RulesetValueExpression>,
        subtrahend: Box<RulesetValueExpression>,
    },
    FloorDivide {
        dividend: Box<RulesetValueExpression>,
        divisor: Box<RulesetValueExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetValueFormula {
    pub schema: RulesetValueFormulaSchema,
    pub expression: RulesetValueExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RulesetValueSource {
    Input,
    Derived { formula: RulesetValueFormula },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetValueContract {
    pub kind: RulesetValueKind,
    pub id: String,
    pub label: String,
    pub numeric_domain_id: String,
    pub source: RulesetValueSource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetNumericDomain {
    pub id: String,
    pub minimum: i64,
    pub maximum: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetProvisions {
    pub operations: Vec<VersionedRpgRequirement>,
    pub capabilities: Vec<VersionedRpgRequirement>,
    pub values: Vec<RulesetValueContract>,
    pub numeric_domains: Vec<RulesetNumericDomain>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetModels {
    pub checks: VersionedRpgRequirement,
    pub turns: VersionedRpgRequirement,
    pub initiative: VersionedRpgRequirement,
    pub reactions: VersionedRpgRequirement,
    pub action_economy: VersionedRpgRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ruleset {
    pub schema: RulesetSchema,
    pub identity: RpgVersionedIdentity,
    pub language: RpgVersionedIdentity,
    pub models: RulesetModels,
    pub provides: RulesetProvisions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedContentPack {
    pub id: String,
    pub version: String,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPackDependencyLockEntry {
    pub requester: String,
    pub package_id: String,
    pub requested_version: String,
    pub resolved_version: String,
    pub source_fingerprint: String,
    pub import_as: String,
    pub relationship: ContentPackDependencyRelationship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentPackDependencyRelationship {
    DependsOn,
    Contributes,
    Patches,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionedRpgRequirement {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentValueRequirement {
    pub kind: RulesetValueKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPackRequirements {
    pub operations: Vec<VersionedRpgRequirement>,
    pub capabilities: Vec<VersionedRpgRequirement>,
    pub values: Vec<ContentValueRequirement>,
    pub numeric_domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentSourceLocation {
    pub module: String,
    pub declaration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentDefinitionProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub source: ContentSourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterializedContentDefinitionKind {
    Action,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterializedContentVisibility {
    Exported,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentExtensionPolicy {
    Sealed,
    Derivable,
    Patchable,
    Configurable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterializedContentDefinition {
    pub id: String,
    pub kind: MaterializedContentDefinitionKind,
    pub visibility: MaterializedContentVisibility,
    pub extension_policy: ContentExtensionPolicy,
    pub semantic: Value,
    pub presentation: Value,
    pub references: Vec<String>,
    pub provenance: ContentDefinitionProvenance,
    pub fingerprint: String,
}

pub const PARTICIPANT_PROFILE_IDENTITY: &str = "asha.rpg.participant-profile";
pub const PARTICIPANT_PROFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticipantProfileSchema {
    pub identity: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParticipantProfileRole {
    Player,
    Creature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticipantProfileBoundedValue {
    pub current: i32,
    pub max: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "owner",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ParticipantProfileInitialCapability {
    Vitality {
        value: ParticipantProfileBoundedValue,
    },
    Stat {
        id: String,
        value: i32,
    },
    Defense {
        id: String,
        value: i32,
    },
    Resource {
        id: String,
        value: ParticipantProfileBoundedValue,
    },
    Modifier {
        stacking_group: String,
        id: String,
        value: i32,
        remaining_turns: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterializedParticipantProfileData {
    pub schema: ParticipantProfileSchema,
    pub role: ParticipantProfileRole,
    pub definition_ids: Vec<String>,
    pub capabilities: Vec<ParticipantProfileInitialCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledParticipantProfile {
    pub definition_id: String,
    pub profile_id: String,
    pub label: String,
    pub description: Option<String>,
    pub role: ParticipantProfileRole,
    pub definition_ids: Vec<String>,
    pub capabilities: Vec<ParticipantProfileInitialCapability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentImpactPlane {
    Semantic,
    Presentation,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentConflictPolicy {
    Reject,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPatchChangeProvenance {
    pub plane: ContentImpactPlane,
    pub path: String,
    #[serde(default)]
    pub path_segments: Vec<ContentPatchPathSegment>,
    pub before: Value,
    pub after: Value,
    pub effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentPatchMemberKey {
    Id,
    ResourceId,
    StatId,
    DefenseId,
    ModifierId,
    DamageType,
    Kind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPatchPathSegment {
    Field {
        name: String,
    },
    Member {
        key: ContentPatchMemberKey,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPatchMemberSelector {
    pub key: ContentPatchMemberKey,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPatchPosition {
    Start,
    End,
    Before { anchor: ContentPatchMemberSelector },
    After { anchor: ContentPatchMemberSelector },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContentPatchOperation {
    SetScalar {
        plane: ContentImpactPlane,
        path: Vec<ContentPatchPathSegment>,
        value: Value,
    },
    UpsertScalar {
        plane: ContentImpactPlane,
        path: Vec<ContentPatchPathSegment>,
        value: Value,
    },
    AdjustNumber {
        plane: ContentImpactPlane,
        path: Vec<ContentPatchPathSegment>,
        multiply: Value,
        add: Value,
    },
    AppendMember {
        plane: ContentImpactPlane,
        path: Vec<ContentPatchPathSegment>,
        identity: ContentPatchMemberSelector,
        value: BTreeMap<String, Value>,
        position: ContentPatchPosition,
    },
    RemoveMember {
        plane: ContentImpactPlane,
        path: Vec<ContentPatchPathSegment>,
        identity: ContentPatchMemberSelector,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPatch {
    pub version: u32,
    pub operations: Vec<ContentPatchOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentMaterializationValue {
    pub semantic: Value,
    pub presentation: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentMaterializationStage {
    pub id: String,
    pub kind: MaterializedContentDefinitionKind,
    pub extension_policy: ContentExtensionPolicy,
    pub value: ContentMaterializationValue,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentMixinParameterType {
    String,
    Number,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentMixinParameterCommitment {
    pub id: String,
    #[serde(rename = "type")]
    pub value_type: ContentMixinParameterType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentMixinDefinitionCommitmentValue {
    pub parameters: Vec<ContentMixinParameterCommitment>,
    pub patch: ContentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ContentDefinitionCommitment {
    Concrete {
        package_id: String,
        package_version: String,
        package_source_fingerprint: String,
        definition_id: String,
        fingerprint: String,
        stage: ContentMaterializationStage,
    },
    Mixin {
        package_id: String,
        package_version: String,
        package_source_fingerprint: String,
        definition_id: String,
        fingerprint: String,
        value: ContentMixinDefinitionCommitmentValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentDerivationMixinProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub fingerprint: String,
    pub patch: ContentPatch,
    pub parameters: BTreeMap<String, Value>,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentDerivationProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub base_definition_id: String,
    pub base_package_id: String,
    pub base_package_version: String,
    pub base_fingerprint: String,
    pub base: ContentMaterializationStage,
    pub mixins: Vec<ContentDerivationMixinProvenance>,
    pub local_patch_fingerprint: String,
    pub local_patch: ContentPatch,
    pub materialized_fingerprint: String,
    pub materialized: ContentMaterializationStage,
    pub changes: Vec<ContentPatchChangeProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentOverlayProvenance {
    pub overlay_package_id: String,
    pub overlay_package_version: String,
    pub target_definition_id: String,
    pub target_package_id: String,
    pub target_package_version: String,
    pub expected_fingerprint: String,
    pub before_fingerprint: String,
    pub after_fingerprint: String,
    pub plane: ContentImpactPlane,
    pub conflict_policy: ContentConflictPolicy,
    pub patch_fingerprint: String,
    pub patch: ContentPatch,
    pub before: ContentMaterializationStage,
    pub order: usize,
    pub changes: Vec<ContentPatchChangeProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledContentPolicyBinding {
    pub id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub view_kind: String,
    pub view_version: u32,
    pub intent_kinds: Vec<String>,
    pub decision_moments: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentRelationshipKind {
    DependsOn,
    Contributes,
    DerivesFrom,
    Patches,
    Configures,
    Exports,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentRelationshipProvenance {
    pub kind: ContentRelationshipKind,
    pub source: String,
    pub target: String,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedPlayBundle {
    pub schema: PlayBundleArtifactSchema,
    pub play_bundle_identity: RpgVersionedIdentity,
    pub ruleset: Ruleset,
    pub content_packs: Vec<ResolvedContentPack>,
    pub dependency_lock: Vec<ContentPackDependencyLockEntry>,
    pub content_requirements: ContentPackRequirements,
    pub exported_roots: Vec<String>,
    pub materialized_definitions: Vec<MaterializedContentDefinition>,
    pub compiled_policy_bindings: Vec<CompiledContentPolicyBinding>,
    pub definition_provenance: Vec<ContentDefinitionProvenance>,
    #[serde(default)]
    pub definition_commitments: Vec<ContentDefinitionCommitment>,
    pub relationships: Vec<ContentRelationshipProvenance>,
    #[serde(default)]
    pub derivation_provenance: Vec<ContentDerivationProvenance>,
    #[serde(default)]
    pub overlay_provenance: Vec<ContentOverlayProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlayBundleFingerprints {
    pub source: String,
    pub semantic: String,
    pub presentation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledPlayBundleArtifact {
    pub artifact_schema: PlayBundleArtifactSchema,
    pub artifact_id: String,
    pub play_bundle_identity: RpgVersionedIdentity,
    pub ruleset: Ruleset,
    pub content_packs: Vec<ResolvedContentPack>,
    pub dependency_lock: Vec<ContentPackDependencyLockEntry>,
    pub content_requirements: ContentPackRequirements,
    pub exported_roots: Vec<String>,
    pub materialized_definitions: Vec<MaterializedContentDefinition>,
    pub compiled_policy_bindings: Vec<CompiledContentPolicyBinding>,
    pub definition_provenance: Vec<ContentDefinitionProvenance>,
    #[serde(default)]
    pub definition_commitments: Vec<ContentDefinitionCommitment>,
    pub relationships: Vec<ContentRelationshipProvenance>,
    pub derivation_provenance: Vec<ContentDerivationProvenance>,
    pub overlay_provenance: Vec<ContentOverlayProvenance>,
    pub fingerprints: PlayBundleFingerprints,
}
