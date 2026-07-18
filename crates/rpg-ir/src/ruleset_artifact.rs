use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PREPARED_RULESET_IDENTITY: &str = "asha.rpg.ruleset.prepared";
pub const COMPILED_RULESET_IDENTITY: &str = "asha.rpg.ruleset.compiled";
pub const RULESET_ARTIFACT_MAJOR: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetArtifactSchema {
    pub identity: String,
    pub major: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledRulesetIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedRulesetSourcePackage {
    pub id: String,
    pub version: String,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetDependencyLockEntry {
    pub requester: String,
    pub package_id: String,
    pub requested_version: String,
    pub resolved_version: String,
    pub source_fingerprint: String,
    pub import_as: String,
    pub relationship: RulesetDependencyRelationship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetDependencyRelationship {
    DependsOn,
    Contributes,
    Patches,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VersionedRulesetRequirement {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetSourceLocation {
    pub module: String,
    pub declaration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetDefinitionProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub source: RulesetSourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterializedRulesetDefinitionKind {
    Action,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterializedRulesetVisibility {
    Exported,
    Support,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetExtensionPolicy {
    Sealed,
    Derivable,
    Patchable,
    Configurable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterializedRulesetDefinition {
    pub id: String,
    pub kind: MaterializedRulesetDefinitionKind,
    pub visibility: MaterializedRulesetVisibility,
    pub extension_policy: RulesetExtensionPolicy,
    pub semantic: Value,
    pub presentation: Value,
    pub references: Vec<String>,
    pub provenance: RulesetDefinitionProvenance,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetImpactPlane {
    Semantic,
    Presentation,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetConflictPolicy {
    Reject,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetPatchChangeProvenance {
    pub plane: RulesetImpactPlane,
    pub path: String,
    #[serde(default)]
    pub path_segments: Vec<RulesetPatchPathSegment>,
    pub before: Value,
    pub after: Value,
    pub effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesetPatchMemberKey {
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
pub enum RulesetPatchPathSegment {
    Field {
        name: String,
    },
    Member {
        key: RulesetPatchMemberKey,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetDerivationMixinProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub fingerprint: String,
    pub parameters: BTreeMap<String, Value>,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetDerivationProvenance {
    pub definition_id: String,
    pub package_id: String,
    pub package_version: String,
    pub base_definition_id: String,
    pub base_package_id: String,
    pub base_package_version: String,
    pub base_fingerprint: String,
    pub mixins: Vec<RulesetDerivationMixinProvenance>,
    pub local_patch_fingerprint: String,
    pub materialized_fingerprint: String,
    pub changes: Vec<RulesetPatchChangeProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetOverlayProvenance {
    pub overlay_package_id: String,
    pub overlay_package_version: String,
    pub target_definition_id: String,
    pub target_package_id: String,
    pub target_package_version: String,
    pub expected_fingerprint: String,
    pub before_fingerprint: String,
    pub after_fingerprint: String,
    pub plane: RulesetImpactPlane,
    pub conflict_policy: RulesetConflictPolicy,
    pub patch_fingerprint: String,
    pub order: usize,
    pub changes: Vec<RulesetPatchChangeProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledRulesetPolicyBinding {
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
pub enum RulesetRelationshipKind {
    DependsOn,
    Contributes,
    DerivesFrom,
    Patches,
    Configures,
    Exports,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetRelationshipProvenance {
    pub kind: RulesetRelationshipKind,
    pub source: String,
    pub target: String,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedRulesetCompilation {
    pub schema: RulesetArtifactSchema,
    pub composition_identity: CompiledRulesetIdentity,
    pub language_identity: CompiledRulesetIdentity,
    pub source_packages: Vec<ResolvedRulesetSourcePackage>,
    pub dependency_lock: Vec<RulesetDependencyLockEntry>,
    pub required_operations: Vec<VersionedRulesetRequirement>,
    pub required_capabilities: Vec<VersionedRulesetRequirement>,
    pub exported_roots: Vec<String>,
    pub materialized_definitions: Vec<MaterializedRulesetDefinition>,
    pub compiled_policy_bindings: Vec<CompiledRulesetPolicyBinding>,
    pub definition_provenance: Vec<RulesetDefinitionProvenance>,
    pub relationships: Vec<RulesetRelationshipProvenance>,
    #[serde(default)]
    pub derivation_provenance: Vec<RulesetDerivationProvenance>,
    #[serde(default)]
    pub overlay_provenance: Vec<RulesetOverlayProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RulesetArtifactFingerprints {
    pub source: String,
    pub semantic: String,
    pub presentation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledRulesetArtifact {
    pub artifact_schema: RulesetArtifactSchema,
    pub artifact_id: String,
    pub composition_identity: CompiledRulesetIdentity,
    pub language_identity: CompiledRulesetIdentity,
    pub source_packages: Vec<ResolvedRulesetSourcePackage>,
    pub dependency_lock: Vec<RulesetDependencyLockEntry>,
    pub required_operations: Vec<VersionedRulesetRequirement>,
    pub required_capabilities: Vec<VersionedRulesetRequirement>,
    pub exported_roots: Vec<String>,
    pub materialized_definitions: Vec<MaterializedRulesetDefinition>,
    pub compiled_policy_bindings: Vec<CompiledRulesetPolicyBinding>,
    pub definition_provenance: Vec<RulesetDefinitionProvenance>,
    pub relationships: Vec<RulesetRelationshipProvenance>,
    pub derivation_provenance: Vec<RulesetDerivationProvenance>,
    pub overlay_provenance: Vec<RulesetOverlayProvenance>,
    pub fingerprints: RulesetArtifactFingerprints,
}
