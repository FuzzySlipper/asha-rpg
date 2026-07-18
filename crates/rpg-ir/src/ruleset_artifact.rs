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
    pub derivation_provenance: Vec<Value>,
    #[serde(default)]
    pub overlay_provenance: Vec<Value>,
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
    pub derivation_provenance: Vec<Value>,
    pub overlay_provenance: Vec<Value>,
    pub fingerprints: RulesetArtifactFingerprints,
}
