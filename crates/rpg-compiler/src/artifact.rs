use std::collections::{BTreeMap, BTreeSet};

use rpg_ir::{
    ActionProcedureImplementation, ActionProcedureParameter, CompiledParticipantProfile,
    CompiledPlayBundleArtifact, ContentDefinitionCommitment, ContentImpactPlane,
    ContentMaterializationStage, ContentMaterializationValue, ContentMixinParameterCommitment,
    ContentMixinParameterType, ContentPatch, ContentPatchChangeProvenance, ContentPatchMemberKey,
    ContentPatchMemberSelector, ContentPatchOperation, ContentPatchPathSegment,
    ContentPatchPosition, ContentRelationshipKind, MaterializedActionProcedureSemantic,
    MaterializedActionSemantic, MaterializedContentDefinition, MaterializedContentDefinitionKind,
    MaterializedContentVisibility, MaterializedParticipantProfileData, NormalizedRpgIr,
    ParticipantProfileInitialCapability, PlayBundleArtifactSchema, PlayBundleFingerprints,
    PreparedPlayBundle, RpgIrAction, RpgIrActionBody, RpgIrCatalogs, RpgIrCheck, RpgIrFormula,
    RpgIrOperation, RpgIrPackage, RpgIrPredicate, RpgIrProgram, RpgIrRequirement,
    RpgIrRequirementKind, RpgIrResourceCost, RpgIrSchema, RpgIrTargetSelector, Ruleset,
    RulesetValueExpression, RulesetValueKind, RulesetValueSource, VersionedRpgRequirement,
    ACTION_DEFINITION_IDENTITY, ACTION_DEFINITION_VERSION, ACTION_PROCEDURE_IDENTITY,
    ACTION_PROCEDURE_VERSION, COMPILED_PLAY_BUNDLE_IDENTITY, PARTICIPANT_PROFILE_IDENTITY,
    PARTICIPANT_PROFILE_VERSION, PLAY_BUNDLE_ARTIFACT_MAJOR, PREPARED_PLAY_BUNDLE_IDENTITY,
    RPG_IR_IDENTITY, RPG_IR_MAJOR,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    capability_registrations, compile_normalized_rpg_ir, operation_registrations, CompiledRpgRules,
    RpgCompileFailure, RpgDiagnostic, RpgDiagnosticStage,
};

#[derive(Debug, Clone)]
pub struct CompiledPlayBundle {
    artifact: CompiledPlayBundleArtifact,
    rules: CompiledRpgRules,
    value_plan: CompiledRulesetValuePlan,
    participant_profiles: Vec<CompiledParticipantProfile>,
}

impl CompiledPlayBundle {
    pub fn artifact(&self) -> &CompiledPlayBundleArtifact {
        &self.artifact
    }

    pub fn rules(&self) -> &CompiledRpgRules {
        &self.rules
    }

    pub fn value_plan(&self) -> &CompiledRulesetValuePlan {
        &self.value_plan
    }

    pub fn participant_profiles(&self) -> &[CompiledParticipantProfile] {
        &self.participant_profiles
    }

    pub fn into_artifact(self) -> CompiledPlayBundleArtifact {
        self.artifact
    }
}

pub const RULESET_VALUE_FORMULA_IDENTITY: &str = "asha.rpg.ruleset-value-formula";
pub const RULESET_VALUE_FORMULA_VERSION: u32 = 1;
const MAX_RULESET_VALUE_FORMULA_DEPTH: usize = 16;
const MAX_RULESET_VALUE_FORMULA_NODES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RulesetValueKey {
    pub kind: RulesetValueKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetValueEvaluationFailure {
    pub code: &'static str,
    pub target: RulesetValueKey,
    pub message: String,
}

#[derive(Debug, Clone)]
struct CompiledRulesetValueDerivation {
    expression: RulesetValueExpression,
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Default)]
pub struct CompiledRulesetValuePlan {
    derivations: BTreeMap<RulesetValueKey, CompiledRulesetValueDerivation>,
    order: Vec<RulesetValueKey>,
}

impl CompiledRulesetValuePlan {
    pub fn is_derived(&self, kind: RulesetValueKind, id: &str) -> bool {
        self.derivations.contains_key(&RulesetValueKey {
            kind,
            id: id.to_owned(),
        })
    }

    pub fn evaluate(
        &self,
        supplied: &BTreeMap<RulesetValueKey, i32>,
    ) -> Result<BTreeMap<RulesetValueKey, i32>, RulesetValueEvaluationFailure> {
        let mut values = supplied
            .iter()
            .map(|(key, value)| (key.clone(), i64::from(*value)))
            .collect::<BTreeMap<_, _>>();
        let mut derived = BTreeMap::new();
        for target in &self.order {
            let derivation = self
                .derivations
                .get(target)
                .expect("compiled derivation order resolves");
            let value = evaluate_ruleset_value_expression(&derivation.expression, &values)
                .map_err(|message| RulesetValueEvaluationFailure {
                    code: "RPG_SCENARIO_RULESET_VALUE_DERIVATION_FAILED",
                    target: target.clone(),
                    message,
                })?;
            if value < derivation.minimum || value > derivation.maximum {
                return Err(RulesetValueEvaluationFailure {
                    code: "RPG_SCENARIO_RULESET_VALUE_DERIVATION_OUT_OF_DOMAIN",
                    target: target.clone(),
                    message: format!(
                        "derived value {value} must be within {}..={}",
                        derivation.minimum, derivation.maximum
                    ),
                });
            }
            let value = i32::try_from(value).map_err(|_| RulesetValueEvaluationFailure {
                code: "RPG_SCENARIO_RULESET_VALUE_DERIVATION_OVERFLOW",
                target: target.clone(),
                message: "derived value does not fit the runtime integer domain".to_owned(),
            })?;
            values.insert(target.clone(), i64::from(value));
            derived.insert(target.clone(), value);
        }
        Ok(derived)
    }
}

pub fn compile_prepared_play_bundle_json(
    source: &[u8],
) -> Result<CompiledPlayBundle, RpgCompileFailure> {
    let prepared = serde_json::from_slice::<PreparedPlayBundle>(source).map_err(|error| {
        RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Decode,
                "PLAY_BUNDLE_PREPARED_DECODE_FAILED",
                "$",
                error.to_string(),
            )],
        }
    })?;
    compile_prepared_play_bundle(prepared)
}

pub fn compile_prepared_play_bundle(
    prepared: PreparedPlayBundle,
) -> Result<CompiledPlayBundle, RpgCompileFailure> {
    let diagnostics = validate_prepared(&prepared);
    if !diagnostics.is_empty() {
        return Err(RpgCompileFailure { diagnostics });
    }

    let normalized_ir = normalized_ir_from_materialized(&prepared)?;
    let rules = compile_normalized_rpg_ir(normalized_ir)?;
    let value_plan = compile_ruleset_value_plan(&prepared.ruleset)?;
    let participant_profiles = compile_participant_profiles(&prepared)?;
    let fingerprints = fingerprints(&prepared)?;
    let artifact_schema = PlayBundleArtifactSchema {
        identity: COMPILED_PLAY_BUNDLE_IDENTITY.to_owned(),
        major: PLAY_BUNDLE_ARTIFACT_MAJOR,
    };
    let artifact_id = fingerprint(&json!({
        "artifactSchema": &artifact_schema,
        "playBundleIdentity": &prepared.play_bundle_identity,
        "ruleset": &prepared.ruleset,
        "contentPacks": &prepared.content_packs,
        "dependencyLock": &prepared.dependency_lock,
        "contentRequirements": &prepared.content_requirements,
        "exportedRoots": &prepared.exported_roots,
        "materializedDefinitions": &prepared.materialized_definitions,
        "compiledPolicyBindings": &prepared.compiled_policy_bindings,
        "definitionProvenance": &prepared.definition_provenance,
        "definitionCommitments": &prepared.definition_commitments,
        "relationships": &prepared.relationships,
        "derivationProvenance": &prepared.derivation_provenance,
        "overlayProvenance": &prepared.overlay_provenance,
        "fingerprints": &fingerprints,
    }))?;
    let artifact = CompiledPlayBundleArtifact {
        artifact_schema,
        artifact_id: format!(
            "{}@{}:{artifact_id}",
            prepared.play_bundle_identity.id, prepared.play_bundle_identity.version
        ),
        play_bundle_identity: prepared.play_bundle_identity,
        ruleset: prepared.ruleset,
        content_packs: prepared.content_packs,
        dependency_lock: prepared.dependency_lock,
        content_requirements: prepared.content_requirements,
        exported_roots: prepared.exported_roots,
        materialized_definitions: prepared.materialized_definitions,
        compiled_policy_bindings: prepared.compiled_policy_bindings,
        definition_provenance: prepared.definition_provenance,
        definition_commitments: prepared.definition_commitments,
        relationships: prepared.relationships,
        derivation_provenance: prepared.derivation_provenance,
        overlay_provenance: prepared.overlay_provenance,
        fingerprints,
    };
    Ok(CompiledPlayBundle {
        artifact,
        rules,
        value_plan,
        participant_profiles,
    })
}

pub fn load_compiled_play_bundle_json(
    source: &[u8],
) -> Result<CompiledPlayBundle, RpgCompileFailure> {
    let artifact =
        serde_json::from_slice::<CompiledPlayBundleArtifact>(source).map_err(|error| {
            RpgCompileFailure {
                diagnostics: vec![RpgDiagnostic::error(
                    RpgDiagnosticStage::Decode,
                    "PLAY_BUNDLE_ARTIFACT_DECODE_FAILED",
                    "$",
                    error.to_string(),
                )],
            }
        })?;
    load_compiled_play_bundle(artifact)
}

pub fn load_compiled_play_bundle(
    artifact: CompiledPlayBundleArtifact,
) -> Result<CompiledPlayBundle, RpgCompileFailure> {
    if artifact.artifact_schema.identity != COMPILED_PLAY_BUNDLE_IDENTITY
        || artifact.artifact_schema.major != PLAY_BUNDLE_ARTIFACT_MAJOR
    {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "PLAY_BUNDLE_ARTIFACT_SCHEMA_UNSUPPORTED",
                "$.artifactSchema",
                format!("expected {COMPILED_PLAY_BUNDLE_IDENTITY}@{PLAY_BUNDLE_ARTIFACT_MAJOR}"),
            )],
        });
    }

    let prepared = prepared_from_artifact(&artifact);
    let recompiled = compile_prepared_play_bundle(prepared)?;
    if recompiled.artifact != artifact {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "PLAY_BUNDLE_ARTIFACT_FINGERPRINT_MISMATCH",
                "$.fingerprints",
                "artifact identity or fingerprint planes do not match its closed contents",
            )],
        });
    }
    Ok(recompiled)
}

fn prepared_from_artifact(artifact: &CompiledPlayBundleArtifact) -> PreparedPlayBundle {
    PreparedPlayBundle {
        schema: PlayBundleArtifactSchema {
            identity: PREPARED_PLAY_BUNDLE_IDENTITY.to_owned(),
            major: PLAY_BUNDLE_ARTIFACT_MAJOR,
        },
        play_bundle_identity: artifact.play_bundle_identity.clone(),
        ruleset: artifact.ruleset.clone(),
        content_packs: artifact.content_packs.clone(),
        dependency_lock: artifact.dependency_lock.clone(),
        content_requirements: artifact.content_requirements.clone(),
        exported_roots: artifact.exported_roots.clone(),
        materialized_definitions: artifact.materialized_definitions.clone(),
        compiled_policy_bindings: artifact.compiled_policy_bindings.clone(),
        definition_provenance: artifact.definition_provenance.clone(),
        definition_commitments: artifact.definition_commitments.clone(),
        relationships: artifact.relationships.clone(),
        derivation_provenance: artifact.derivation_provenance.clone(),
        overlay_provenance: artifact.overlay_provenance.clone(),
    }
}

fn validate_prepared(prepared: &PreparedPlayBundle) -> Vec<RpgDiagnostic> {
    let mut diagnostics = Vec::new();
    if prepared.schema.identity != PREPARED_PLAY_BUNDLE_IDENTITY
        || prepared.schema.major != PLAY_BUNDLE_ARTIFACT_MAJOR
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "PLAY_BUNDLE_PREPARED_SCHEMA_UNSUPPORTED",
            "$.schema",
            format!("expected {PREPARED_PLAY_BUNDLE_IDENTITY}@{PLAY_BUNDLE_ARTIFACT_MAJOR}"),
        ));
    }
    validate_identity(
        &prepared.play_bundle_identity.id,
        &prepared.play_bundle_identity.version,
        "$.playBundleIdentity",
        &mut diagnostics,
    );
    validate_ruleset(prepared, &mut diagnostics);
    validate_sources_and_lock(prepared, &mut diagnostics);
    validate_requirements(prepared, &mut diagnostics);
    validate_definitions(prepared, &mut diagnostics);
    let definition_commitments = validate_definition_commitments(prepared, &mut diagnostics);
    validate_materialization_provenance(prepared, &definition_commitments, &mut diagnostics);
    diagnostics
}

fn validate_sources_and_lock(prepared: &PreparedPlayBundle, diagnostics: &mut Vec<RpgDiagnostic>) {
    let mut sources = BTreeMap::new();
    let mut previous = None::<(&str, &str)>;
    for (index, source) in prepared.content_packs.iter().enumerate() {
        validate_identity(
            &source.id,
            &source.version,
            &format!("$.contentPacks[{index}]"),
            diagnostics,
        );
        if !valid_fingerprint(&source.source_fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_SOURCE_FINGERPRINT_INVALID",
                format!("$.contentPacks[{index}].sourceFingerprint"),
                "source fingerprint must be fnv1a64 with sixteen lowercase hex digits",
            ));
        }
        if let Some(previous_identity) = previous {
            if previous_identity >= (source.id.as_str(), source.version.as_str()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_SOURCE_PACKAGES_NOT_CANONICAL",
                    format!("$.contentPacks[{index}]"),
                    "source packages must be strictly identity-sorted",
                ));
            }
        }
        previous = Some((&source.id, &source.version));
        let identity = format!("{}@{}", source.id, source.version);
        if sources
            .insert(identity.clone(), &source.source_fingerprint)
            .is_some()
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DUPLICATE_SOURCE_PACKAGE",
                format!("$.contentPacks[{index}]"),
                format!("duplicate source package {identity}"),
            ));
        }
    }

    let mut lock_identities = BTreeSet::new();
    let mut previous_lock = None::<String>;
    for (index, entry) in prepared.dependency_lock.iter().enumerate() {
        if !exact_version(&entry.resolved_version) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_LOCK_VERSION_NOT_EXACT",
                format!("$.dependencyLock[{index}].resolvedVersion"),
                "resolved dependency versions must be exact semver",
            ));
        }
        let source_identity = format!("{}@{}", entry.package_id, entry.resolved_version);
        if sources.get(&source_identity).copied() != Some(&entry.source_fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_LOCK_SOURCE_MISMATCH",
                format!("$.dependencyLock[{index}]"),
                format!("lock entry does not match source package {source_identity}"),
            ));
        }
        let identity = format!(
            "{}\u{0}{}\u{0}{}",
            entry.requester, entry.package_id, entry.import_as
        );
        if let Some(previous_identity) = &previous_lock {
            if previous_identity >= &identity {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_LOCK_NOT_CANONICAL",
                    format!("$.dependencyLock[{index}]"),
                    "dependency lock entries must be strictly sorted",
                ));
            }
        }
        previous_lock = Some(identity.clone());
        if !lock_identities.insert(identity) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DUPLICATE_LOCK_ENTRY",
                format!("$.dependencyLock[{index}]"),
                "duplicate dependency lock entry",
            ));
        }
    }
}

fn validate_requirements(prepared: &PreparedPlayBundle, diagnostics: &mut Vec<RpgDiagnostic>) {
    validate_sorted_requirements(
        &prepared.content_requirements.operations,
        "$.contentRequirements.operations",
        diagnostics,
    );
    validate_sorted_requirements(
        &prepared.content_requirements.capabilities,
        "$.contentRequirements.capabilities",
        diagnostics,
    );
    let mut previous_value = None::<(RulesetValueKind, &str)>;
    for (index, requirement) in prepared.content_requirements.values.iter().enumerate() {
        let identity = (requirement.kind, requirement.id.as_str());
        if previous_value.is_some_and(|previous| previous >= identity) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL",
                format!("$.contentRequirements.values[{index}]"),
                "value requirements must be strictly identity-sorted",
            ));
        }
        previous_value = Some(identity);
    }
    let mut previous_domain = None::<&str>;
    for (index, requirement) in prepared
        .content_requirements
        .numeric_domains
        .iter()
        .enumerate()
    {
        if previous_domain.is_some_and(|previous| previous >= requirement.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL",
                format!("$.contentRequirements.numericDomains[{index}]"),
                "numeric domain requirements must be strictly identity-sorted",
            ));
        }
        previous_domain = Some(requirement.as_str());
    }

    for (index, requirement) in prepared.content_requirements.operations.iter().enumerate() {
        let provided = prepared
            .ruleset
            .provides
            .operations
            .iter()
            .any(|provision| {
                provision.id == requirement.id && provision.version == requirement.version
            });
        if !provided {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "PLAY_BUNDLE_OPERATION_REQUIREMENT_MISSING",
                format!("$.contentRequirements.operations[{index}]"),
                format!(
                    "content requires operation {}@{}, which the ruleset does not provide",
                    requirement.id, requirement.version
                ),
            ));
        }
    }
    for (index, requirement) in prepared
        .content_requirements
        .capabilities
        .iter()
        .enumerate()
    {
        let provided = prepared
            .ruleset
            .provides
            .capabilities
            .iter()
            .any(|provision| {
                provision.id == requirement.id && provision.version == requirement.version
            });
        if !provided {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "PLAY_BUNDLE_CAPABILITY_REQUIREMENT_MISSING",
                format!("$.contentRequirements.capabilities[{index}]"),
                format!(
                    "content requires capability {}@{}, which the ruleset does not provide",
                    requirement.id, requirement.version
                ),
            ));
        }
    }
    let provided_values = prepared
        .ruleset
        .provides
        .values
        .iter()
        .map(|value| (value.kind, value.id.as_str()))
        .collect::<BTreeSet<_>>();
    for (index, requirement) in prepared.content_requirements.values.iter().enumerate() {
        if provided_values.contains(&(requirement.kind, requirement.id.as_str())) {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "PLAY_BUNDLE_VALUE_REQUIREMENT_MISSING",
            format!("$.contentRequirements.values[{index}]"),
            format!(
                "content requires {:?} {}, which the ruleset does not provide",
                requirement.kind, requirement.id
            ),
        ));
    }
    let provided_domains = prepared
        .ruleset
        .provides
        .numeric_domains
        .iter()
        .map(|domain| domain.id.as_str())
        .collect::<BTreeSet<_>>();
    for (index, requirement) in prepared
        .content_requirements
        .numeric_domains
        .iter()
        .enumerate()
    {
        if provided_domains.contains(requirement.as_str()) {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "PLAY_BUNDLE_NUMERIC_DOMAIN_REQUIREMENT_MISSING",
            format!("$.contentRequirements.numericDomains[{index}]"),
            format!(
                "content requires numeric domain {requirement}, which the ruleset does not provide"
            ),
        ));
    }
}

fn validate_ruleset(prepared: &PreparedPlayBundle, diagnostics: &mut Vec<RpgDiagnostic>) {
    let ruleset = &prepared.ruleset;
    if ruleset.schema.identity != "asha.rpg.ruleset" || ruleset.schema.major != 1 {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "RULESET_SCHEMA_UNSUPPORTED",
            "$.ruleset.schema",
            "expected asha.rpg.ruleset@1",
        ));
    }
    validate_identity(
        &ruleset.identity.id,
        &ruleset.identity.version,
        "$.ruleset.identity",
        diagnostics,
    );
    if ruleset.language.id != "asha-rpg" || ruleset.language.version != "1.0.0" {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "RULESET_LANGUAGE_UNSUPPORTED",
            "$.ruleset.language",
            "supported language is asha-rpg@1.0.0",
        ));
    }
    for (path, binding, expected_id) in [
        (
            "$.ruleset.models.checks",
            &ruleset.models.checks,
            "check.d20-roll-over",
        ),
        (
            "$.ruleset.models.turns",
            &ruleset.models.turns,
            "turn.ordered-one-action",
        ),
        (
            "$.ruleset.models.initiative",
            &ruleset.models.initiative,
            "initiative.scenario-ordered",
        ),
        (
            "$.ruleset.models.reactions",
            &ruleset.models.reactions,
            "reaction.before-damage-choice",
        ),
        (
            "$.ruleset.models.actionEconomy",
            &ruleset.models.action_economy,
            "action-economy.one-action-plus-reaction",
        ),
    ] {
        if binding.id == expected_id && binding.version == 1 {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "RULESET_MODEL_UNSUPPORTED",
            path,
            format!(
                "ruleset model {}@{} is not bound by Rust authority",
                binding.id, binding.version
            ),
        ));
    }
    validate_sorted_requirements(
        &ruleset.provides.operations,
        "$.ruleset.provides.operations",
        diagnostics,
    );
    validate_sorted_requirements(
        &ruleset.provides.capabilities,
        "$.ruleset.provides.capabilities",
        diagnostics,
    );
    for (index, provision) in ruleset.provides.operations.iter().enumerate() {
        let registered = operation_registrations().iter().any(|registration| {
            registration.id == provision.id && registration.version == provision.version
        });
        if !registered {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_OPERATION_PROVISION_UNSUPPORTED",
                format!("$.ruleset.provides.operations[{index}]"),
                format!(
                    "ruleset provides operation {}@{}, which Rust authority does not bind",
                    provision.id, provision.version
                ),
            ));
        }
    }
    for (index, provision) in ruleset.provides.capabilities.iter().enumerate() {
        let registered = capability_registrations().iter().any(|registration| {
            registration.id.as_str() == provision.id && registration.version == provision.version
        });
        if !registered {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_CAPABILITY_PROVISION_UNSUPPORTED",
                format!("$.ruleset.provides.capabilities[{index}]"),
                format!(
                    "ruleset provides capability {}@{}, which Rust authority does not bind",
                    provision.id, provision.version
                ),
            ));
        }
    }
    let mut previous_value = None::<(RulesetValueKind, &str)>;
    let declared_domains = ruleset
        .provides
        .numeric_domains
        .iter()
        .map(|domain| domain.id.as_str())
        .collect::<BTreeSet<_>>();
    for (index, value) in ruleset.provides.values.iter().enumerate() {
        let identity = (value.kind, value.id.as_str());
        if previous_value.is_some_and(|previous| previous >= identity)
            || value.label.trim().is_empty()
            || !declared_domains.contains(value.numeric_domain_id.as_str())
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_VALUE_PROVISIONS_NOT_CANONICAL",
                format!("$.ruleset.provides.values[{index}]"),
                "ruleset value provisions must be unique, sorted, labelled, and use a declared numeric domain",
            ));
        }
        previous_value = Some(identity);
    }
    let mut previous_domain = None::<&str>;
    for (index, domain) in ruleset.provides.numeric_domains.iter().enumerate() {
        if previous_domain.is_some_and(|previous| previous >= domain.id.as_str())
            || domain.minimum > domain.maximum
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_NUMERIC_DOMAINS_NOT_CANONICAL",
                format!("$.ruleset.provides.numericDomains[{index}]"),
                "ruleset numeric domains must be unique, sorted, and ordered",
            ));
        }
        previous_domain = Some(domain.id.as_str());
    }
}

fn compile_ruleset_value_plan(
    ruleset: &Ruleset,
) -> Result<CompiledRulesetValuePlan, RpgCompileFailure> {
    let domains = ruleset
        .provides
        .numeric_domains
        .iter()
        .map(|domain| (domain.id.as_str(), (domain.minimum, domain.maximum)))
        .collect::<BTreeMap<_, _>>();
    let contracts = ruleset
        .provides
        .values
        .iter()
        .map(|value| {
            (
                RulesetValueKey {
                    kind: value.kind,
                    id: value.id.clone(),
                },
                value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut derivations = BTreeMap::new();
    let mut dependencies = BTreeMap::<RulesetValueKey, BTreeSet<RulesetValueKey>>::new();

    for (index, value) in ruleset.provides.values.iter().enumerate() {
        let RulesetValueSource::Derived { formula } = &value.source else {
            continue;
        };
        let value_path = format!("$.ruleset.provides.values[{index}].source.formula");
        if formula.schema.identity != RULESET_VALUE_FORMULA_IDENTITY
            || formula.schema.version != RULESET_VALUE_FORMULA_VERSION
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_VALUE_FORMULA_SCHEMA_UNSUPPORTED",
                format!("{value_path}.schema"),
                format!(
                    "expected {RULESET_VALUE_FORMULA_IDENTITY}@{RULESET_VALUE_FORMULA_VERSION}"
                ),
            ));
        }

        let target = RulesetValueKey {
            kind: value.kind,
            id: value.id.clone(),
        };
        let mut reads = BTreeSet::new();
        let mut node_count = 0;
        validate_ruleset_value_expression(
            &formula.expression,
            ruleset,
            &contracts,
            &format!("{value_path}.expression"),
            1,
            &mut node_count,
            &mut reads,
            &mut diagnostics,
        );
        if node_count > MAX_RULESET_VALUE_FORMULA_NODES {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "RULESET_VALUE_FORMULA_TOO_LARGE",
                format!("{value_path}.expression"),
                format!(
                    "a ruleset value formula may contain at most {MAX_RULESET_VALUE_FORMULA_NODES} nodes"
                ),
            ));
        }
        let Some((minimum, maximum)) = domains.get(value.numeric_domain_id.as_str()).copied()
        else {
            continue;
        };
        derivations.insert(
            target.clone(),
            CompiledRulesetValueDerivation {
                expression: formula.expression.clone(),
                minimum,
                maximum,
            },
        );
        dependencies.insert(target, reads);
    }

    if !diagnostics.is_empty() {
        return Err(RpgCompileFailure { diagnostics });
    }

    let derived_keys = derivations.keys().cloned().collect::<BTreeSet<_>>();
    for reads in dependencies.values_mut() {
        reads.retain(|read| derived_keys.contains(read));
    }
    let mut remaining_dependency_count = dependencies
        .iter()
        .map(|(target, reads)| (target.clone(), reads.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<RulesetValueKey, BTreeSet<RulesetValueKey>>::new();
    for (target, reads) in &dependencies {
        for read in reads {
            dependents
                .entry(read.clone())
                .or_default()
                .insert(target.clone());
        }
    }
    let mut ready = remaining_dependency_count
        .iter()
        .filter_map(|(target, count)| (*count == 0).then_some(target.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(derivations.len());
    while let Some(target) = ready.pop_first() {
        order.push(target.clone());
        for dependent in dependents.get(&target).into_iter().flatten() {
            let count = remaining_dependency_count
                .get_mut(dependent)
                .expect("compiled dependency target exists");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    if order.len() != derivations.len() {
        let cycle = remaining_dependency_count
            .iter()
            .filter_map(|(target, count)| (*count > 0).then_some(target.id.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_VALUE_DERIVATION_CYCLE",
                "$.ruleset.provides.values",
                format!("derived ruleset values contain a dependency cycle: {cycle}"),
            )],
        });
    }

    Ok(CompiledRulesetValuePlan { derivations, order })
}

#[allow(clippy::too_many_arguments)]
fn validate_ruleset_value_expression(
    expression: &RulesetValueExpression,
    ruleset: &Ruleset,
    contracts: &BTreeMap<RulesetValueKey, &rpg_ir::RulesetValueContract>,
    path: &str,
    depth: usize,
    node_count: &mut usize,
    reads: &mut BTreeSet<RulesetValueKey>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    *node_count += 1;
    if depth > MAX_RULESET_VALUE_FORMULA_DEPTH {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "RULESET_VALUE_FORMULA_TOO_DEEP",
            path,
            format!(
                "a ruleset value formula may be nested at most {MAX_RULESET_VALUE_FORMULA_DEPTH} levels"
            ),
        ));
        return;
    }
    match expression {
        RulesetValueExpression::Constant { .. } => {}
        RulesetValueExpression::ReadValue {
            ruleset_id,
            value_kind,
            value_id,
        } => {
            if ruleset_id != &ruleset.identity.id {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "RULESET_VALUE_FORMULA_OWNER_MISMATCH",
                    format!("{path}.rulesetId"),
                    format!(
                        "formula references ruleset {ruleset_id}, but the selected ruleset is {}",
                        ruleset.identity.id
                    ),
                ));
                return;
            }
            let key = RulesetValueKey {
                kind: *value_kind,
                id: value_id.clone(),
            };
            if !contracts.contains_key(&key) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "RULESET_VALUE_FORMULA_REFERENCE_MISSING",
                    path,
                    format!("formula references undeclared ruleset value {value_id}"),
                ));
                return;
            }
            reads.insert(key);
        }
        RulesetValueExpression::Subtract {
            minuend,
            subtrahend,
        } => {
            validate_ruleset_value_expression(
                minuend,
                ruleset,
                contracts,
                &format!("{path}.minuend"),
                depth + 1,
                node_count,
                reads,
                diagnostics,
            );
            validate_ruleset_value_expression(
                subtrahend,
                ruleset,
                contracts,
                &format!("{path}.subtrahend"),
                depth + 1,
                node_count,
                reads,
                diagnostics,
            );
        }
        RulesetValueExpression::FloorDivide { dividend, divisor } => {
            validate_ruleset_value_expression(
                dividend,
                ruleset,
                contracts,
                &format!("{path}.dividend"),
                depth + 1,
                node_count,
                reads,
                diagnostics,
            );
            validate_ruleset_value_expression(
                divisor,
                ruleset,
                contracts,
                &format!("{path}.divisor"),
                depth + 1,
                node_count,
                reads,
                diagnostics,
            );
        }
    }
}

fn evaluate_ruleset_value_expression(
    expression: &RulesetValueExpression,
    values: &BTreeMap<RulesetValueKey, i64>,
) -> Result<i64, String> {
    match expression {
        RulesetValueExpression::Constant { value } => Ok(*value),
        RulesetValueExpression::ReadValue {
            value_kind,
            value_id,
            ..
        } => values
            .get(&RulesetValueKey {
                kind: *value_kind,
                id: value_id.clone(),
            })
            .copied()
            .ok_or_else(|| format!("required input value {value_id} was not supplied")),
        RulesetValueExpression::Subtract {
            minuend,
            subtrahend,
        } => {
            let minuend = evaluate_ruleset_value_expression(minuend, values)?;
            let subtrahend = evaluate_ruleset_value_expression(subtrahend, values)?;
            minuend
                .checked_sub(subtrahend)
                .ok_or_else(|| "integer subtraction overflowed".to_owned())
        }
        RulesetValueExpression::FloorDivide { dividend, divisor } => {
            let dividend = evaluate_ruleset_value_expression(dividend, values)?;
            let divisor = evaluate_ruleset_value_expression(divisor, values)?;
            if divisor == 0 {
                return Err("integer floor division used a zero divisor".to_owned());
            }
            let quotient = dividend
                .checked_div(divisor)
                .ok_or_else(|| "integer floor division overflowed".to_owned())?;
            let remainder = dividend
                .checked_rem(divisor)
                .ok_or_else(|| "integer floor division overflowed".to_owned())?;
            if remainder != 0 && ((remainder > 0) != (divisor > 0)) {
                quotient
                    .checked_sub(1)
                    .ok_or_else(|| "integer floor division overflowed".to_owned())
            } else {
                Ok(quotient)
            }
        }
    }
}

fn compile_participant_profiles(
    prepared: &PreparedPlayBundle,
) -> Result<Vec<CompiledParticipantProfile>, RpgCompileFailure> {
    let definitions = prepared
        .materialized_definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect::<BTreeMap<_, _>>();
    let values = prepared
        .ruleset
        .provides
        .values
        .iter()
        .map(|value| {
            (
                RulesetValueKey {
                    kind: value.kind,
                    id: value.id.clone(),
                },
                value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let domains = prepared
        .ruleset
        .provides
        .numeric_domains
        .iter()
        .map(|domain| (domain.id.as_str(), domain))
        .collect::<BTreeMap<_, _>>();
    let required_capabilities = prepared
        .content_requirements
        .capabilities
        .iter()
        .map(|requirement| (requirement.id.as_str(), requirement.version))
        .collect::<BTreeMap<_, _>>();
    let required_values = prepared
        .content_requirements
        .values
        .iter()
        .map(|requirement| RulesetValueKey {
            kind: requirement.kind,
            id: requirement.id.clone(),
        })
        .collect::<BTreeSet<_>>();
    let required_domains = prepared
        .content_requirements
        .numeric_domains
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    let mut profiles = Vec::new();
    let mut profile_ids = BTreeSet::new();

    for (definition_index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if definition.kind != MaterializedContentDefinitionKind::Support
            || definition.semantic.get("catalog").and_then(Value::as_str)
                != Some("participantProfile")
        {
            continue;
        }
        let path = format!("$.materializedDefinitions[{definition_index}].semantic");
        let Some(profile_id) = definition.semantic.get("id").and_then(Value::as_str) else {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "PARTICIPANT_PROFILE_ID_MISSING",
                format!("{path}.id"),
                "participant profile semantic identity is required",
            ));
            continue;
        };
        if !valid_identifier(profile_id) || !profile_ids.insert(profile_id.to_owned()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "PARTICIPANT_PROFILE_ID_INVALID",
                format!("{path}.id"),
                "participant profile identities must be unique portable identifiers",
            ));
        }
        let Some(data_value) = definition.semantic.get("data") else {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "PARTICIPANT_PROFILE_DATA_MISSING",
                format!("{path}.data"),
                "participant profile data is required",
            ));
            continue;
        };
        let data = match serde_json::from_value::<MaterializedParticipantProfileData>(
            data_value.clone(),
        ) {
            Ok(data) => data,
            Err(error) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "PARTICIPANT_PROFILE_DATA_INVALID",
                    format!("{path}.data"),
                    error.to_string(),
                ));
                continue;
            }
        };
        if data.schema.identity != PARTICIPANT_PROFILE_IDENTITY
            || data.schema.version != PARTICIPANT_PROFILE_VERSION
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "PARTICIPANT_PROFILE_SCHEMA_UNSUPPORTED",
                format!("{path}.data.schema"),
                format!("expected {PARTICIPANT_PROFILE_IDENTITY}@{PARTICIPANT_PROFILE_VERSION}"),
            ));
        }

        let mut previous_definition = None::<&str>;
        let mut has_action = false;
        for (reference_index, definition_id) in data.definition_ids.iter().enumerate() {
            if previous_definition.is_some_and(|previous| previous >= definition_id.as_str()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "PARTICIPANT_PROFILE_DEFINITIONS_NOT_CANONICAL",
                    format!("{path}.data.definitionIds[{reference_index}]"),
                    "participant profile definition identities must be unique and sorted",
                ));
            }
            previous_definition = Some(definition_id);
            let Some(target) = definitions.get(definition_id.as_str()) else {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "PARTICIPANT_PROFILE_DEFINITION_MISSING",
                    format!("{path}.data.definitionIds[{reference_index}]"),
                    format!("profile definition {definition_id} is not materialized"),
                ));
                continue;
            };
            if target.visibility != MaterializedContentVisibility::Exported
                || !definition.references.contains(definition_id)
            {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "PARTICIPANT_PROFILE_DEFINITION_NOT_EXPORTED",
                    format!("{path}.data.definitionIds[{reference_index}]"),
                    format!("profile definition {definition_id} is not an exported graph edge"),
                ));
            }
            has_action |= target.kind == MaterializedContentDefinitionKind::Action;
        }
        if !has_action {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Requirements,
                "PARTICIPANT_PROFILE_ACTION_REQUIRED",
                format!("{path}.data.definitionIds"),
                "participant profiles require at least one exported action",
            ));
        }

        let mut vitality_count = 0;
        let mut capability_identities = BTreeSet::new();
        for (capability_index, capability) in data.capabilities.iter().enumerate() {
            let capability_path = format!("{path}.data.capabilities[{capability_index}]");
            let (identity, capability_requirement) = match capability {
                ParticipantProfileInitialCapability::Vitality { value } => {
                    vitality_count += 1;
                    validate_profile_bounded_value(
                        value.current,
                        value.max,
                        &capability_path,
                        &mut diagnostics,
                    );
                    ("vitality".to_owned(), "capability.vitality")
                }
                ParticipantProfileInitialCapability::Stat { id, value } => {
                    validate_profile_ruleset_value(
                        RulesetValueKind::Stat,
                        id,
                        *value,
                        &capability_path,
                        &values,
                        &domains,
                        &required_values,
                        &required_domains,
                        &mut diagnostics,
                    );
                    (format!("stat:{id}"), "capability.stats")
                }
                ParticipantProfileInitialCapability::Defense { id, value } => {
                    validate_profile_ruleset_value(
                        RulesetValueKind::Defense,
                        id,
                        *value,
                        &capability_path,
                        &values,
                        &domains,
                        &required_values,
                        &required_domains,
                        &mut diagnostics,
                    );
                    (format!("defense:{id}"), "capability.defenses")
                }
                ParticipantProfileInitialCapability::Resource { id, value } => {
                    validate_profile_content_value(
                        "resource",
                        id,
                        &capability_path,
                        definition,
                        &definitions,
                        &mut diagnostics,
                    );
                    validate_profile_bounded_value(
                        value.current,
                        value.max,
                        &capability_path,
                        &mut diagnostics,
                    );
                    (format!("resource:{id}"), "capability.resources")
                }
                ParticipantProfileInitialCapability::Modifier {
                    stacking_group,
                    id,
                    remaining_turns,
                    ..
                } => {
                    validate_profile_content_value(
                        "modifier",
                        id,
                        &capability_path,
                        definition,
                        &definitions,
                        &mut diagnostics,
                    );
                    if stacking_group.is_empty() || !(1..=1_000).contains(remaining_turns) {
                        diagnostics.push(RpgDiagnostic::error(
                            RpgDiagnosticStage::Semantics,
                            "PARTICIPANT_PROFILE_MODIFIER_INVALID",
                            &capability_path,
                            "profile modifiers require a stacking group and remainingTurns within 1..=1000",
                        ));
                    }
                    (format!("modifier:{stacking_group}"), "capability.modifiers")
                }
            };
            if !capability_identities.insert(identity.clone()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "PARTICIPANT_PROFILE_CAPABILITY_DUPLICATE",
                    &capability_path,
                    format!("participant profile repeats capability fact {identity}"),
                ));
            }
            if required_capabilities.get(capability_requirement).copied() != Some(1) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Requirements,
                    "PARTICIPANT_PROFILE_CAPABILITY_REQUIREMENT_MISSING",
                    &capability_path,
                    format!("profile capability owner requires {capability_requirement}@1"),
                ));
            }
        }
        if vitality_count != 1 {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Requirements,
                "PARTICIPANT_PROFILE_VITALITY_REQUIRED",
                format!("{path}.data.capabilities"),
                "participant profiles require exactly one vitality base fact",
            ));
        }

        let label = definition
            .presentation
            .get("label")
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty());
        let Some(label) = label else {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "PARTICIPANT_PROFILE_LABEL_REQUIRED",
                format!("$.materializedDefinitions[{definition_index}].presentation.label"),
                "exported participant profiles require a presentation label",
            ));
            continue;
        };
        let description = definition
            .presentation
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if definition.visibility == MaterializedContentVisibility::Exported {
            profiles.push(CompiledParticipantProfile {
                definition_id: definition.id.clone(),
                profile_id: profile_id.to_owned(),
                label: label.to_owned(),
                description,
                role: data.role,
                definition_ids: data.definition_ids,
                capabilities: data.capabilities,
            });
        }
    }

    if diagnostics.is_empty() {
        profiles.sort_by(|left, right| left.definition_id.cmp(&right.definition_id));
        Ok(profiles)
    } else {
        Err(RpgCompileFailure { diagnostics })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_profile_ruleset_value(
    kind: RulesetValueKind,
    id: &str,
    value: i32,
    path: &str,
    values: &BTreeMap<RulesetValueKey, &rpg_ir::RulesetValueContract>,
    domains: &BTreeMap<&str, &rpg_ir::RulesetNumericDomain>,
    required_values: &BTreeSet<RulesetValueKey>,
    required_domains: &BTreeSet<&str>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let key = RulesetValueKey {
        kind,
        id: id.to_owned(),
    };
    let Some(contract) = values.get(&key) else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "PARTICIPANT_PROFILE_RULESET_VALUE_MISSING",
            format!("{path}.id"),
            format!("profile references undeclared Ruleset value {id}"),
        ));
        return;
    };
    if matches!(contract.source, RulesetValueSource::Derived { .. }) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "PARTICIPANT_PROFILE_DERIVED_VALUE_FORBIDDEN",
            path,
            format!("profile must not supply derived Ruleset value {id}"),
        ));
    }
    if !required_values.contains(&key)
        || !required_domains.contains(contract.numeric_domain_id.as_str())
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Requirements,
            "PARTICIPANT_PROFILE_VALUE_REQUIREMENT_MISSING",
            path,
            format!("profile value {id} and its numeric domain must be declared requirements"),
        ));
    }
    if let Some(domain) = domains.get(contract.numeric_domain_id.as_str()) {
        let value = i64::from(value);
        if value < domain.minimum || value > domain.maximum {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "PARTICIPANT_PROFILE_VALUE_OUT_OF_DOMAIN",
                format!("{path}.value"),
                format!(
                    "profile value must be within {}..={}",
                    domain.minimum, domain.maximum
                ),
            ));
        }
    }
}

fn validate_profile_content_value(
    catalog: &str,
    id: &str,
    path: &str,
    profile_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let target = definitions.values().find(|definition| {
        definition.kind == MaterializedContentDefinitionKind::Support
            && definition.semantic.get("catalog").and_then(Value::as_str) == Some(catalog)
            && definition.semantic.get("id").and_then(Value::as_str) == Some(id)
    });
    let Some(target) = target else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "PARTICIPANT_PROFILE_CONTENT_VALUE_MISSING",
            format!("{path}.id"),
            format!("profile references missing {catalog} {id}"),
        ));
        return;
    };
    if target.visibility != MaterializedContentVisibility::Exported
        || !profile_definition.references.contains(&target.id)
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "PARTICIPANT_PROFILE_CONTENT_VALUE_NOT_EXPORTED",
            format!("{path}.id"),
            format!("profile {catalog} {id} is not an exported graph edge"),
        ));
    }
}

fn validate_profile_bounded_value(
    current: i32,
    max: i32,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    if current < 0 || max < 0 || current > max {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "PARTICIPANT_PROFILE_BOUNDED_VALUE_INVALID",
            path,
            "profile bounded values require 0 <= current <= max",
        ));
    }
}

fn validate_sorted_requirements(
    requirements: &[VersionedRpgRequirement],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let diagnostic_code = if path.starts_with("$.ruleset") {
        "RULESET_PROVISIONS_NOT_CANONICAL"
    } else {
        "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL"
    };
    let mut previous = None::<(&str, u32)>;
    for (index, requirement) in requirements.iter().enumerate() {
        let identity = (requirement.id.as_str(), requirement.version);
        if previous.is_some_and(|value| value >= identity) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                diagnostic_code,
                format!("{path}[{index}]"),
                "requirements must be strictly identity-sorted",
            ));
        }
        previous = Some(identity);
    }
}

fn validate_definition_commitments<'a>(
    prepared: &'a PreparedPlayBundle,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> BTreeMap<String, &'a ContentDefinitionCommitment> {
    let source_fingerprints = prepared
        .content_packs
        .iter()
        .map(|source| {
            (
                format!("{}@{}", source.id, source.version),
                source.source_fingerprint.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut commitments = BTreeMap::new();
    let mut previous_identity = None::<String>;
    for (index, commitment) in prepared.definition_commitments.iter().enumerate() {
        let (package_id, package_version, package_source_fingerprint, definition_id, fingerprint) =
            definition_commitment_header(commitment);
        let path = format!("$.definitionCommitments[{index}]");
        validate_identity(package_id, package_version, &path, diagnostics);
        if !valid_identifier(definition_id) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITION_COMMITMENT_ID_INVALID",
                format!("{path}.definitionId"),
                format!("invalid committed definition identity {definition_id}"),
            ));
        }
        if !valid_fingerprint(fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITION_COMMITMENT_FINGERPRINT_INVALID",
                format!("{path}.fingerprint"),
                "definition commitment fingerprints must be fnv1a64 with sixteen lowercase hex digits",
            ));
        }
        let package_identity = format!("{package_id}@{package_version}");
        if source_fingerprints.get(&package_identity).copied() != Some(package_source_fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DEFINITION_COMMITMENT_SOURCE_MISMATCH",
                format!("{path}.packageSourceFingerprint"),
                format!("definition commitment does not match source package {package_identity}"),
            ));
        }
        let identity = definition_commitment_identity(package_id, package_version, definition_id);
        if previous_identity
            .as_ref()
            .is_some_and(|previous| previous >= &identity)
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITION_COMMITMENTS_NOT_CANONICAL",
                path.clone(),
                "definition commitments must be strictly identity-sorted",
            ));
        }
        previous_identity = Some(identity.clone());
        if commitments.insert(identity, commitment).is_some() {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DUPLICATE_DEFINITION_COMMITMENT",
                path.clone(),
                "definition commitments must have unique package-qualified identities",
            ));
        }
        match commitment {
            ContentDefinitionCommitment::Concrete {
                definition_id,
                fingerprint,
                stage,
                ..
            } => {
                if stage.id != *definition_id {
                    diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Artifact,
                        "CONTENT_PACK_CONCRETE_COMMITMENT_STAGE_MISMATCH",
                        format!("{path}.stage.id"),
                        "a concrete commitment stage must retain its named definition identity",
                    ));
                }
                match canonical_fingerprint(stage) {
                    Ok(actual) if actual != *fingerprint => diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Artifact,
                        "CONTENT_PACK_CONCRETE_COMMITMENT_FINGERPRINT_MISMATCH",
                        format!("{path}.fingerprint"),
                        format!("the committed concrete stage fingerprints as {actual}"),
                    )),
                    Err(failure) => diagnostics.extend(failure.diagnostics),
                    _ => {}
                }
            }
            ContentDefinitionCommitment::Mixin {
                fingerprint, value, ..
            } => {
                validate_mixin_parameter_commitments(&value.parameters, &path, diagnostics);
                match canonical_fingerprint(value) {
                    Ok(actual) if actual != *fingerprint => diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Artifact,
                        "CONTENT_PACK_MIXIN_COMMITMENT_FINGERPRINT_MISMATCH",
                        format!("{path}.fingerprint"),
                        format!("the committed mixin definition fingerprints as {actual}"),
                    )),
                    Err(failure) => diagnostics.extend(failure.diagnostics),
                    _ => {}
                }
            }
        }
    }

    let mut expected = prepared
        .derivation_provenance
        .iter()
        .flat_map(|provenance| {
            let mut identities = vec![
                definition_commitment_identity(
                    &provenance.package_id,
                    &provenance.package_version,
                    &provenance.definition_id,
                ),
                definition_commitment_identity(
                    &provenance.base_package_id,
                    &provenance.base_package_version,
                    &provenance.base_definition_id,
                ),
            ];
            identities.extend(provenance.mixins.iter().map(|mixin| {
                definition_commitment_identity(
                    &mixin.package_id,
                    &mixin.package_version,
                    &mixin.definition_id,
                )
            }));
            identities
        })
        .collect::<BTreeSet<_>>();
    expected.extend(prepared.overlay_provenance.iter().map(|provenance| {
        definition_commitment_identity(
            &provenance.target_package_id,
            &provenance.target_package_version,
            &provenance.target_definition_id,
        )
    }));
    let actual = commitments.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "CONTENT_PACK_DEFINITION_COMMITMENT_COVERAGE_MISMATCH",
            "$.definitionCommitments",
            "derivation targets, bases, named mixins, and overlay targets require exactly one source commitment",
        ));
    }
    commitments
}

fn definition_commitment_header(
    commitment: &ContentDefinitionCommitment,
) -> (&str, &str, &str, &str, &str) {
    match commitment {
        ContentDefinitionCommitment::Concrete {
            package_id,
            package_version,
            package_source_fingerprint,
            definition_id,
            fingerprint,
            ..
        }
        | ContentDefinitionCommitment::Mixin {
            package_id,
            package_version,
            package_source_fingerprint,
            definition_id,
            fingerprint,
            ..
        } => (
            package_id,
            package_version,
            package_source_fingerprint,
            definition_id,
            fingerprint,
        ),
    }
}

fn validate_mixin_parameter_commitments(
    parameters: &[ContentMixinParameterCommitment],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let mut previous = None::<&str>;
    for (index, parameter) in parameters.iter().enumerate() {
        if !valid_identifier(&parameter.id) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_MIXIN_COMMITMENT_PARAMETER_ID_INVALID",
                format!("{path}.value.parameters[{index}].id"),
                format!("invalid mixin parameter identity {}", parameter.id),
            ));
        }
        if previous.is_some_and(|value| value >= parameter.id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_MIXIN_COMMITMENT_PARAMETERS_NOT_CANONICAL",
                format!("{path}.value.parameters[{index}]"),
                "committed mixin parameters must be strictly identity-sorted",
            ));
        }
        previous = Some(&parameter.id);
        if let Some(default) = &parameter.default {
            if !mixin_parameter_value_matches(default, parameter.value_type) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_MIXIN_COMMITMENT_PARAMETER_DEFAULT_INVALID",
                    format!("{path}.value.parameters[{index}].default"),
                    "a committed mixin parameter default must match its declared type",
                ));
            }
        }
    }
}

fn validate_applied_mixin_parameters(
    definitions: &[ContentMixinParameterCommitment],
    supplied: &BTreeMap<String, Value>,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let declared = definitions
        .iter()
        .map(|parameter| (parameter.id.as_str(), parameter))
        .collect::<BTreeMap<_, _>>();
    if supplied
        .keys()
        .any(|id| !declared.contains_key(id.as_str()))
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_MIXIN_PARAMETER_COMMITMENT_MISMATCH",
            path,
            "applied mixin parameters contain an undeclared parameter",
        ));
        return;
    }
    for parameter in definitions {
        let resolved = supplied.get(&parameter.id);
        if !resolved.is_some_and(|value| mixin_parameter_value_matches(value, parameter.value_type))
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_MIXIN_PARAMETER_COMMITMENT_MISMATCH",
                path,
                format!(
                    "applied mixin parameter {} is not explicitly resolved or has the wrong committed type",
                    parameter.id
                ),
            ));
        }
    }
}

fn mixin_parameter_value_matches(value: &Value, value_type: ContentMixinParameterType) -> bool {
    match value_type {
        ContentMixinParameterType::String => value.is_string(),
        ContentMixinParameterType::Number => value.is_number(),
        ContentMixinParameterType::Boolean => value.is_boolean(),
    }
}

fn definition_commitment_identity(
    package_id: &str,
    package_version: &str,
    definition_id: &str,
) -> String {
    format!("{package_id}@{package_version}#{definition_id}")
}

fn validate_definitions(prepared: &PreparedPlayBundle, diagnostics: &mut Vec<RpgDiagnostic>) {
    let mut definitions = BTreeMap::<&str, &MaterializedContentDefinition>::new();
    let mut previous = None::<&str>;
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if previous.is_some_and(|value| value >= definition.id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITIONS_NOT_CANONICAL",
                format!("$.materializedDefinitions[{index}]"),
                "materialized definitions must be strictly identity-sorted",
            ));
        }
        previous = Some(&definition.id);
        if definitions
            .insert(definition.id.as_str(), definition)
            .is_some()
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DUPLICATE_MATERIALIZED_DEFINITION",
                format!("$.materializedDefinitions[{index}].id"),
                format!("duplicate definition {}", definition.id),
            ));
        }
        if definition.provenance.definition_id != definition.id {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITION_PROVENANCE_MISMATCH",
                format!("$.materializedDefinitions[{index}].provenance"),
                "definition provenance must name its materialized definition",
            ));
        }
        match materialized_definition_fingerprint(definition) {
            Ok(expected) if expected != definition.fingerprint => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DEFINITION_FINGERPRINT_MISMATCH",
                    format!("$.materializedDefinitions[{index}].fingerprint"),
                    format!(
                        "definition {} fingerprint does not match its canonical materialized value",
                        definition.id
                    ),
                ));
            }
            Err(failure) => diagnostics.extend(failure.diagnostics),
            Ok(_) => {}
        }
    }
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        for reference in &definition.references {
            if !definitions.contains_key(reference.as_str()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "CONTENT_PACK_ARTIFACT_REFERENCE_MISSING",
                    format!("$.materializedDefinitions[{index}].references"),
                    format!("materialized reference {reference} is missing"),
                ));
            }
        }
    }
    let mut previous_root = None::<&str>;
    let roots = prepared
        .exported_roots
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for (index, root) in prepared.exported_roots.iter().enumerate() {
        if previous_root.is_some_and(|value| value >= root.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_EXPORTED_ROOTS_NOT_CANONICAL",
                format!("$.exportedRoots[{index}]"),
                "exported roots must be strictly identity-sorted",
            ));
        }
        previous_root = Some(root);
        if !definitions.contains_key(root.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_EXPORTED_ROOT_MISSING",
                format!("$.exportedRoots[{index}]"),
                format!("exported root {root} is not materialized"),
            ));
        } else if definitions[root.as_str()].visibility != MaterializedContentVisibility::Exported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_EXPORTED_ROOT_VISIBILITY_MISMATCH",
                format!("$.exportedRoots[{index}]"),
                format!("exported root {root} must have exported visibility"),
            ));
        }
    }
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        let is_root = roots.contains(definition.id.as_str());
        let is_exported = definition.visibility == MaterializedContentVisibility::Exported;
        if is_root != is_exported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DEFINITION_VISIBILITY_MISMATCH",
                format!("$.materializedDefinitions[{index}].visibility"),
                "only exported roots may have exported visibility",
            ));
        }
    }

    let expected_provenance = prepared
        .materialized_definitions
        .iter()
        .map(|definition| definition.provenance.clone())
        .collect::<Vec<_>>();
    if prepared.definition_provenance != expected_provenance {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DEFINITION_PROVENANCE_NOT_CANONICAL",
            "$.definitionProvenance",
            "definition provenance must exactly match canonical materialized definition provenance",
        ));
    }

    let mut reachable = BTreeSet::new();
    let mut visiting = Vec::new();
    for root in &prepared.exported_roots {
        visit_materialized_definition(
            root,
            &definitions,
            &mut visiting,
            &mut reachable,
            diagnostics,
        );
    }
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if !reachable.contains(definition.id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_MATERIALIZED_DEFINITION_UNREACHABLE",
                format!("$.materializedDefinitions[{index}]"),
                format!(
                    "materialized definition {} is not reachable from an exported root",
                    definition.id
                ),
            ));
        }
    }
}

fn visit_materialized_definition<'a>(
    definition_id: &str,
    definitions: &BTreeMap<&'a str, &'a MaterializedContentDefinition>,
    visiting: &mut Vec<String>,
    reachable: &mut BTreeSet<String>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    if reachable.contains(definition_id) {
        return;
    }
    if let Some(cycle_start) = visiting.iter().position(|entry| entry == definition_id) {
        let mut cycle = visiting[cycle_start..].to_vec();
        cycle.push(definition_id.to_owned());
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "CONTENT_PACK_ARTIFACT_DEFINITION_CYCLE",
            "$.materializedDefinitions",
            format!("definition cycle: {}", cycle.join(" -> ")),
        ));
        return;
    }
    let Some(definition) = definitions.get(definition_id) else {
        return;
    };
    visiting.push(definition_id.to_owned());
    for reference in &definition.references {
        visit_materialized_definition(reference, definitions, visiting, reachable, diagnostics);
    }
    visiting.pop();
    reachable.insert(definition_id.to_owned());
}

#[derive(Default)]
struct DerivedCatalogs {
    stats: BTreeSet<String>,
    defenses: BTreeSet<String>,
    resources: BTreeSet<String>,
    modifiers: BTreeSet<String>,
}

#[derive(Default)]
struct RulesetCatalogs {
    stats: BTreeSet<String>,
    defenses: BTreeSet<String>,
}

impl RulesetCatalogs {
    fn contains(&self, catalog: &str, value: &str) -> bool {
        match catalog {
            "stat" => self.stats.contains(value),
            "defense" => self.defenses.contains(value),
            _ => false,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDefinitionSemantic {
    catalog: String,
    id: String,
}

#[derive(Clone, Copy)]
struct CatalogReferenceKind {
    catalog: &'static str,
    diagnostic: &'static str,
}

impl CatalogReferenceKind {
    const DAMAGE_TYPE: Self = Self {
        catalog: "damageType",
        diagnostic: "DAMAGE_TYPE",
    };
    const DEFENSE: Self = Self {
        catalog: "defense",
        diagnostic: "DEFENSE",
    };
    const MODIFIER: Self = Self {
        catalog: "modifier",
        diagnostic: "MODIFIER",
    };
    const RESOURCE: Self = Self {
        catalog: "resource",
        diagnostic: "RESOURCE",
    };
    const STAT: Self = Self {
        catalog: "stat",
        diagnostic: "STAT",
    };
}

fn normalized_ir_from_materialized(
    prepared: &PreparedPlayBundle,
) -> Result<NormalizedRpgIr, RpgCompileFailure> {
    let definitions = prepared
        .materialized_definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut catalogs = DerivedCatalogs::default();
    let ruleset_catalogs = RulesetCatalogs {
        stats: prepared
            .ruleset
            .provides
            .values
            .iter()
            .filter(|value| value.kind == RulesetValueKind::Stat)
            .map(|value| value.id.clone())
            .collect(),
        defenses: prepared
            .ruleset
            .provides
            .values
            .iter()
            .filter(|value| value.kind == RulesetValueKind::Defense)
            .map(|value| value.id.clone())
            .collect(),
    };
    let mut actions = Vec::new();

    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if definition.kind != MaterializedContentDefinitionKind::ActionProcedure {
            continue;
        }
        let path = format!("$.materializedDefinitions[{index}].semantic");
        validate_action_procedure_definition(
            definition,
            &definitions,
            &prepared.ruleset,
            &path,
            &mut diagnostics,
        );
    }

    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if definition.kind != MaterializedContentDefinitionKind::Action {
            continue;
        }
        let path = format!("$.materializedDefinitions[{index}].semantic");
        let semantic =
            match serde_json::from_value::<MaterializedActionSemantic>(definition.semantic.clone())
            {
                Ok(semantic) => semantic,
                Err(error) => {
                    diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Semantics,
                        "CONTENT_PACK_ACTION_SEMANTIC_DECODE_FAILED",
                        &path,
                        error.to_string(),
                    ));
                    continue;
                }
            };
        let (mut action, effective_references) = match semantic {
            MaterializedActionSemantic::Inline { schema, action } => {
                if !validate_action_semantic_schema(
                    &schema,
                    ACTION_DEFINITION_IDENTITY,
                    ACTION_DEFINITION_VERSION,
                    &format!("{path}.schema"),
                    &mut diagnostics,
                ) {
                    continue;
                }
                (
                    action,
                    definition
                        .references
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                )
            }
            MaterializedActionSemantic::Invocation {
                schema,
                procedure_id,
                procedure_owner_package_id,
                arguments,
            } => {
                if !validate_action_semantic_schema(
                    &schema,
                    ACTION_DEFINITION_IDENTITY,
                    ACTION_DEFINITION_VERSION,
                    &format!("{path}.schema"),
                    &mut diagnostics,
                ) {
                    continue;
                }
                let mut effective_references = definition
                    .references
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();
                let mut visiting = Vec::new();
                let Some(body) = expand_action_procedure(
                    &procedure_id,
                    &procedure_owner_package_id,
                    &arguments,
                    &definitions,
                    &prepared.ruleset,
                    &mut effective_references,
                    &mut visiting,
                    false,
                    &format!("{path}.invocation"),
                    &mut diagnostics,
                ) else {
                    continue;
                };
                let name = definition
                    .presentation
                    .get("label")
                    .and_then(Value::as_str)
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or(&definition.id)
                    .to_owned();
                (
                    RpgIrAction {
                        id: definition.id.clone(),
                        name,
                        source_path: definition.provenance.source.module.clone(),
                        targets: body.targets,
                        check: body.check,
                        roll_scope: body.roll_scope,
                        costs: body.costs,
                        program: body.program,
                    },
                    effective_references,
                )
            }
        };
        if action.id != definition.id {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_ACTION_SEMANTIC_ID_MISMATCH",
                format!("{path}.id"),
                format!(
                    "materialized action {} carries semantic identity {}",
                    definition.id, action.id
                ),
            ));
        }
        let mut effective_definition = definition.clone();
        effective_definition.references = effective_references.into_iter().collect();
        resolve_action_catalogs(
            &mut action,
            &effective_definition,
            &definitions,
            &ruleset_catalogs,
            &path,
            &mut diagnostics,
        );
        collect_action_catalogs(&action, &mut catalogs);
        actions.push(action);
    }
    if !diagnostics.is_empty() {
        return Err(RpgCompileFailure { diagnostics });
    }
    actions.sort_by(|left, right| left.id.cmp(&right.id));

    let requirements = prepared
        .content_requirements
        .operations
        .iter()
        .map(|requirement| RpgIrRequirement {
            kind: RpgIrRequirementKind::Operation,
            id: requirement.id.clone(),
            version: requirement.version,
        })
        .chain(
            prepared
                .content_requirements
                .capabilities
                .iter()
                .map(|requirement| RpgIrRequirement {
                    kind: RpgIrRequirementKind::Capability,
                    id: requirement.id.clone(),
                    version: requirement.version,
                }),
        )
        .collect();
    Ok(NormalizedRpgIr {
        schema: RpgIrSchema {
            identity: RPG_IR_IDENTITY.to_owned(),
            major: RPG_IR_MAJOR,
        },
        package: RpgIrPackage {
            id: prepared.play_bundle_identity.id.clone(),
            version: prepared.play_bundle_identity.version.clone(),
        },
        catalogs: RpgIrCatalogs {
            stats: catalogs.stats.into_iter().collect(),
            defenses: catalogs.defenses.into_iter().collect(),
            resources: catalogs.resources.into_iter().collect(),
            modifiers: catalogs.modifiers.into_iter().collect(),
            capabilities: prepared
                .content_requirements
                .capabilities
                .iter()
                .map(|requirement| requirement.id.clone())
                .collect(),
        },
        requirements,
        actions,
    })
}

fn validate_action_semantic_schema(
    schema: &rpg_ir::ActionSemanticSchema,
    identity: &str,
    version: u32,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> bool {
    if schema.identity == identity && schema.version == version {
        return true;
    }
    diagnostics.push(RpgDiagnostic::error(
        RpgDiagnosticStage::Compatibility,
        "ACTION_SEMANTIC_SCHEMA_UNSUPPORTED",
        path,
        format!("expected {identity}@{version}"),
    ));
    false
}

fn validate_action_procedure_definition(
    definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset: &Ruleset,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let initial_diagnostic_count = diagnostics.len();
    let procedure = match serde_json::from_value::<MaterializedActionProcedureSemantic>(
        definition.semantic.clone(),
    ) {
        Ok(procedure) => procedure,
        Err(error) => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_SEMANTIC_DECODE_FAILED",
                path,
                error.to_string(),
            ));
            return;
        }
    };
    if !validate_action_semantic_schema(
        &procedure.schema,
        ACTION_PROCEDURE_IDENTITY,
        ACTION_PROCEDURE_VERSION,
        &format!("{path}.schema"),
        diagnostics,
    ) {
        return;
    }
    if procedure.owner_package_id != definition.provenance.package_id {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_OWNER_MISMATCH",
            format!("{path}.ownerPackageId"),
            format!(
                "procedure owner {} does not match declaring package {}",
                procedure.owner_package_id, definition.provenance.package_id
            ),
        ));
    }
    let mut parameters = BTreeMap::new();
    let mut previous = None::<&str>;
    for (index, parameter) in procedure.parameters.iter().enumerate() {
        let parameter_path = format!("{path}.parameters[{index}]");
        if previous.is_some_and(|previous| previous >= parameter.id()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "ACTION_PROCEDURE_PARAMETERS_NOT_CANONICAL",
                &parameter_path,
                "procedure parameters must be strictly identity-sorted",
            ));
        }
        previous = Some(parameter.id());
        if !valid_identifier(parameter.id())
            || parameters.insert(parameter.id(), parameter).is_some()
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_PARAMETER_INVALID",
                &parameter_path,
                "procedure parameter identities must be unique portable identifiers",
            ));
        }
        if let ActionProcedureParameter::BoundedInteger {
            minimum, maximum, ..
        } = parameter
        {
            if minimum > maximum {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "ACTION_PROCEDURE_INTEGER_BOUNDS_INVALID",
                    &parameter_path,
                    "bounded integer parameter minimum must not exceed maximum",
                ));
            }
        }
    }
    let implementation_value = serde_json::to_value(&procedure.implementation)
        .expect("procedure implementation serializes");
    let mut used_parameters = BTreeSet::new();
    validate_parameter_markers(
        &implementation_value,
        &parameters,
        &format!("{path}.implementation"),
        &mut used_parameters,
        diagnostics,
    );
    for parameter_id in parameters.keys() {
        if used_parameters.contains(*parameter_id) {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "ACTION_PROCEDURE_PARAMETER_UNUSED",
            format!("{path}.parameters.{parameter_id}"),
            format!("procedure parameter {parameter_id} is declared but never consumed"),
        ));
    }
    if let ActionProcedureImplementation::Invocation {
        procedure_id,
        procedure_owner_package_id,
        arguments,
    } = &procedure.implementation
    {
        validate_composed_action_procedure_contract(
            definition,
            procedure_id,
            procedure_owner_package_id,
            arguments,
            &parameters,
            definitions,
            ruleset,
            &format!("{path}.implementation"),
            diagnostics,
        );
    }
    if diagnostics.len() != initial_diagnostic_count {
        return;
    }

    validate_action_procedure_callable(
        definition,
        &procedure,
        definitions,
        ruleset,
        path,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_composed_action_procedure_contract(
    definition: &MaterializedContentDefinition,
    procedure_id: &str,
    procedure_owner_package_id: &str,
    arguments: &BTreeMap<String, Value>,
    outer_parameters: &BTreeMap<&str, &ActionProcedureParameter>,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset: &Ruleset,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    if !definition
        .references
        .iter()
        .any(|reference| reference == procedure_id)
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_REFERENCE_UNDECLARED",
            format!("{path}.procedureId"),
            format!("procedure {procedure_id} must be a direct definition reference"),
        ));
        return;
    }
    let Some(target_definition) = definitions.get(procedure_id) else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_DEFINITION_MISSING",
            format!("{path}.procedureId"),
            format!("procedure definition {procedure_id} is absent"),
        ));
        return;
    };
    if target_definition.kind != MaterializedContentDefinitionKind::ActionProcedure {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_DEFINITION_KIND_INVALID",
            format!("{path}.procedureId"),
            format!("definition {procedure_id} is not an action procedure"),
        ));
        return;
    }
    if target_definition.provenance.package_id != procedure_owner_package_id {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_REFERENCE_OWNER_MISMATCH",
            format!("{path}.procedureOwnerPackageId"),
            format!(
                "procedure {procedure_id} belongs to {}, not {procedure_owner_package_id}",
                target_definition.provenance.package_id
            ),
        ));
        return;
    }
    let target = match serde_json::from_value::<MaterializedActionProcedureSemantic>(
        target_definition.semantic.clone(),
    ) {
        Ok(target) => target,
        Err(error) => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_SEMANTIC_DECODE_FAILED",
                format!("{path}.procedure"),
                error.to_string(),
            ));
            return;
        }
    };
    if !validate_action_semantic_schema(
        &target.schema,
        ACTION_PROCEDURE_IDENTITY,
        ACTION_PROCEDURE_VERSION,
        &format!("{path}.procedure.schema"),
        diagnostics,
    ) {
        return;
    }
    let target_parameters = target
        .parameters
        .iter()
        .map(|parameter| (parameter.id(), parameter))
        .collect::<BTreeMap<_, _>>();
    let effective_references = definition
        .references
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for (argument_id, argument) in arguments {
        let argument_path = format!("{path}.arguments.{argument_id}");
        let Some(target_parameter) = target_parameters.get(argument_id.as_str()) else {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_ARGUMENT_EXTRA",
                &argument_path,
                format!("procedure {procedure_id} has no parameter {argument_id}"),
            ));
            continue;
        };
        if let Some((outer_parameter_id, outer_parameter_type)) =
            action_procedure_parameter_marker(argument)
        {
            let compatible =
                outer_parameters
                    .get(outer_parameter_id)
                    .is_some_and(|outer_parameter| {
                        outer_parameter.value_type() == outer_parameter_type
                            && outer_parameter.value_type() == target_parameter.value_type()
                            && bounded_parameter_domain_is_compatible(
                                outer_parameter,
                                target_parameter,
                            )
                    });
            if !compatible {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH",
                    &argument_path,
                    format!(
                        "forwarded argument does not satisfy {}",
                        target_parameter.value_type()
                    ),
                ));
            }
            continue;
        }
        let _ = normalize_procedure_argument(
            argument,
            target_parameter,
            definitions,
            ruleset,
            &effective_references,
            false,
            &argument_path,
            diagnostics,
        );
    }
    for parameter_id in target_parameters.keys() {
        if arguments.contains_key(*parameter_id) {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "ACTION_PROCEDURE_ARGUMENT_MISSING",
            format!("{path}.arguments.{parameter_id}"),
            format!("procedure {procedure_id} requires argument {parameter_id}"),
        ));
    }
}

fn action_procedure_parameter_marker(value: &Value) -> Option<(&str, &str)> {
    let object = value.as_object()?;
    if object.get("kind").and_then(Value::as_str) != Some("parameter") {
        return None;
    }
    Some((
        object.get("parameterId")?.as_str()?,
        object.get("parameterType")?.as_str()?,
    ))
}

fn bounded_parameter_domain_is_compatible(
    source: &ActionProcedureParameter,
    target: &ActionProcedureParameter,
) -> bool {
    match (source, target) {
        (
            ActionProcedureParameter::BoundedInteger {
                minimum: source_minimum,
                maximum: source_maximum,
                ..
            },
            ActionProcedureParameter::BoundedInteger {
                minimum: target_minimum,
                maximum: target_maximum,
                ..
            },
        ) => source_minimum >= target_minimum && source_maximum <= target_maximum,
        _ => true,
    }
}

fn validate_parameter_markers(
    value: &Value,
    parameters: &BTreeMap<&str, &ActionProcedureParameter>,
    path: &str,
    used_parameters: &mut BTreeSet<String>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_parameter_markers(
                    value,
                    parameters,
                    &format!("{path}[{index}]"),
                    used_parameters,
                    diagnostics,
                );
            }
        }
        Value::Object(object)
            if object.get("kind").and_then(Value::as_str) == Some("parameter") =>
        {
            let parameter_id = object.get("parameterId").and_then(Value::as_str);
            let parameter_type = object.get("parameterType").and_then(Value::as_str);
            if parameter_id.is_some_and(|id| parameters.contains_key(id)) {
                used_parameters.insert(parameter_id.expect("checked parameter id").to_owned());
            }
            let valid = object.len() == 3
                && parameter_id
                    .and_then(|id| parameters.get(id))
                    .is_some_and(|parameter| Some(parameter.value_type()) == parameter_type);
            if !valid {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "ACTION_PROCEDURE_PARAMETER_REFERENCE_INVALID",
                    path,
                    "parameter marker must exactly name a declared parameter with the same type",
                ));
            }
        }
        Value::Object(object) => {
            for (field, value) in object {
                validate_parameter_markers(
                    value,
                    parameters,
                    &format!("{path}.{field}"),
                    used_parameters,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn validate_action_procedure_callable(
    definition: &MaterializedContentDefinition,
    procedure: &MaterializedActionProcedureSemantic,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset: &Ruleset,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let mut base_references = definition
        .references
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    base_references.insert(definition.id.clone());
    let base_arguments = procedure_validation_arguments(&procedure.parameters, ruleset);
    let mut variants = vec![(base_arguments.clone(), base_references.clone())];
    let mut maximum_arguments = base_arguments.clone();
    let mut varying_bounded_parameter_count = 0_usize;
    for parameter in &procedure.parameters {
        let ActionProcedureParameter::BoundedInteger {
            minimum, maximum, ..
        } = parameter
        else {
            continue;
        };
        maximum_arguments.insert(parameter.id().to_owned(), Value::from(*maximum));
        if minimum != maximum {
            varying_bounded_parameter_count += 1;
        }
        let mut arguments = base_arguments.clone();
        arguments.insert(parameter.id().to_owned(), Value::from(*maximum));
        variants.push((arguments, base_references.clone()));
    }
    // Minimums expose lower-bound failures, and each independent maximum
    // against the remaining minimums exposes relational failures. Semantic
    // budgets such as expanded program nodes are cumulative and monotone, so
    // they additionally require every varying bound at its maximum together.
    if varying_bounded_parameter_count > 1 {
        variants.push((maximum_arguments, base_references.clone()));
    }

    for (arguments, mut effective_references) in variants {
        let mut visiting = Vec::new();
        let expanded = expand_action_procedure(
            &definition.id,
            &procedure.owner_package_id,
            &arguments,
            definitions,
            ruleset,
            &mut effective_references,
            &mut visiting,
            true,
            &format!("{path}.callable"),
            diagnostics,
        );
        if let Some(body) = expanded {
            validate_expanded_action_procedure_body(definition, body, path, diagnostics);
        }
    }
}

fn validate_expanded_action_procedure_body(
    definition: &MaterializedContentDefinition,
    body: RpgIrActionBody,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let action = RpgIrAction {
        id: definition.id.clone(),
        name: definition.id.clone(),
        source_path: definition.provenance.source.module.clone(),
        targets: body.targets,
        check: body.check,
        roll_scope: body.roll_scope,
        costs: body.costs,
        program: body.program,
    };
    let mut catalogs = DerivedCatalogs::default();
    collect_action_catalogs(&action, &mut catalogs);
    let capabilities = capability_registrations()
        .iter()
        .map(|registration| registration.id.as_str().to_owned())
        .collect::<Vec<_>>();
    let requirements = operation_registrations()
        .iter()
        .map(|registration| RpgIrRequirement {
            kind: RpgIrRequirementKind::Operation,
            id: registration.id.to_owned(),
            version: registration.version,
        })
        .chain(
            capability_registrations()
                .iter()
                .map(|registration| RpgIrRequirement {
                    kind: RpgIrRequirementKind::Capability,
                    id: registration.id.as_str().to_owned(),
                    version: registration.version,
                }),
        )
        .collect();
    let source = NormalizedRpgIr {
        schema: RpgIrSchema {
            identity: RPG_IR_IDENTITY.to_owned(),
            major: RPG_IR_MAJOR,
        },
        package: RpgIrPackage {
            id: "procedure.validation".to_owned(),
            version: "1.0.0".to_owned(),
        },
        catalogs: RpgIrCatalogs {
            stats: catalogs.stats.into_iter().collect(),
            defenses: catalogs.defenses.into_iter().collect(),
            resources: catalogs.resources.into_iter().collect(),
            modifiers: catalogs.modifiers.into_iter().collect(),
            capabilities,
        },
        requirements,
        actions: vec![action],
    };
    let Err(failure) = compile_normalized_rpg_ir(source) else {
        return;
    };
    diagnostics.extend(failure.diagnostics.into_iter().map(|mut diagnostic| {
        diagnostic.path = diagnostic.path.strip_prefix("$.actions[0]").map_or_else(
            || format!("{path}.callable.validation.{}", diagnostic.path),
            |suffix| format!("{path}.callable{suffix}"),
        );
        diagnostic
    }));
}

fn procedure_validation_arguments(
    parameters: &[ActionProcedureParameter],
    ruleset: &Ruleset,
) -> BTreeMap<String, Value> {
    parameters
        .iter()
        .map(|parameter| {
            (
                parameter.id().to_owned(),
                procedure_parameter_validation_sample(parameter, ruleset),
            )
        })
        .collect()
}

fn procedure_parameter_validation_sample(
    parameter: &ActionProcedureParameter,
    ruleset: &Ruleset,
) -> Value {
    match parameter {
        ActionProcedureParameter::BoundedInteger { minimum, .. } => Value::from(*minimum),
        ActionProcedureParameter::Identifier { .. } => Value::from("procedure.parameter"),
        ActionProcedureParameter::Boolean { .. } => Value::Bool(false),
        ActionProcedureParameter::Formula { .. } => json!({"kind": "constant", "value": 0}),
        ActionProcedureParameter::RulesetValueReference { .. } => {
            json!({
                "kind": RulesetValueKind::Stat,
                "id": "procedure.parameter",
                "rulesetId": ruleset.identity.id,
            })
        }
        ActionProcedureParameter::CatalogReference { .. } => json!({
            "definitionId": "procedure.parameter",
            "category": "damageType",
            "packageId": "procedure.parameter",
        }),
        ActionProcedureParameter::Targeting { .. } => json!({
            "kind": "participant",
            "team": "any",
            "maximumRange": 1,
            "maximumTargets": 1,
        }),
        ActionProcedureParameter::Check { .. } => json!({"kind": "noRoll"}),
        ActionProcedureParameter::Costs { .. } => json!([]),
        ActionProcedureParameter::Program { .. } => json!({
            "kind": "atomic",
            "body": {
                "kind": "onCheck",
                "noRoll": {
                    "kind": "operation",
                    "operation": {
                        "kind": "heal",
                        "amount": {"kind": "constant", "value": 1},
                    },
                },
            },
        }),
        ActionProcedureParameter::SemanticBranches { .. } => json!({
            "kind": "onCheck",
            "noRoll": {
                "kind": "operation",
                "operation": {
                    "kind": "heal",
                    "amount": {"kind": "constant", "value": 1},
                },
            },
        }),
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcedureRulesetValueReference {
    kind: RulesetValueKind,
    id: String,
    ruleset_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProcedureCatalogReference {
    definition_id: String,
    category: String,
    package_id: String,
}

#[allow(clippy::too_many_arguments)]
fn expand_action_procedure(
    procedure_id: &str,
    procedure_owner_package_id: &str,
    arguments: &BTreeMap<String, Value>,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset: &Ruleset,
    effective_references: &mut BTreeSet<String>,
    visiting: &mut Vec<String>,
    allow_symbolic_references: bool,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> Option<RpgIrActionBody> {
    if !effective_references.contains(procedure_id) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_REFERENCE_UNDECLARED",
            format!("{path}.procedureId"),
            format!("procedure {procedure_id} must be a direct definition reference"),
        ));
        return None;
    }
    if let Some(cycle_start) = visiting.iter().position(|entry| entry == procedure_id) {
        let mut cycle = visiting[cycle_start..].to_vec();
        cycle.push(procedure_id.to_owned());
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_CYCLE",
            path,
            format!("procedure cycle: {}", cycle.join(" -> ")),
        ));
        return None;
    }
    let Some(definition) = definitions.get(procedure_id) else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_DEFINITION_MISSING",
            format!("{path}.procedureId"),
            format!("procedure definition {procedure_id} is absent"),
        ));
        return None;
    };
    if definition.kind != MaterializedContentDefinitionKind::ActionProcedure {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_DEFINITION_KIND_INVALID",
            format!("{path}.procedureId"),
            format!("definition {procedure_id} is not an action procedure"),
        ));
        return None;
    }
    if definition.provenance.package_id != procedure_owner_package_id {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "ACTION_PROCEDURE_REFERENCE_OWNER_MISMATCH",
            format!("{path}.procedureOwnerPackageId"),
            format!(
                "procedure {procedure_id} belongs to {}, not {procedure_owner_package_id}",
                definition.provenance.package_id
            ),
        ));
        return None;
    }
    let procedure = match serde_json::from_value::<MaterializedActionProcedureSemantic>(
        definition.semantic.clone(),
    ) {
        Ok(procedure) => procedure,
        Err(error) => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_SEMANTIC_DECODE_FAILED",
                format!("{path}.procedure"),
                error.to_string(),
            ));
            return None;
        }
    };
    if !validate_action_semantic_schema(
        &procedure.schema,
        ACTION_PROCEDURE_IDENTITY,
        ACTION_PROCEDURE_VERSION,
        &format!("{path}.procedure.schema"),
        diagnostics,
    ) {
        return None;
    }
    effective_references.extend(definition.references.iter().cloned());
    let parameters = procedure
        .parameters
        .iter()
        .map(|parameter| (parameter.id(), parameter))
        .collect::<BTreeMap<_, _>>();
    let mut substitutions = BTreeMap::new();
    for (argument_id, argument) in arguments {
        let Some(parameter) = parameters.get(argument_id.as_str()) else {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Semantics,
                "ACTION_PROCEDURE_ARGUMENT_EXTRA",
                format!("{path}.arguments.{argument_id}"),
                format!("procedure {procedure_id} has no parameter {argument_id}"),
            ));
            continue;
        };
        if let Some(value) = normalize_procedure_argument(
            argument,
            parameter,
            definitions,
            ruleset,
            effective_references,
            allow_symbolic_references,
            &format!("{path}.arguments.{argument_id}"),
            diagnostics,
        ) {
            substitutions.insert(argument_id.as_str(), value);
        }
    }
    for parameter_id in parameters.keys() {
        if arguments.contains_key(*parameter_id) {
            continue;
        }
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "ACTION_PROCEDURE_ARGUMENT_MISSING",
            format!("{path}.arguments.{parameter_id}"),
            format!("procedure {procedure_id} requires argument {parameter_id}"),
        ));
    }
    if substitutions.len() != parameters.len() {
        return None;
    }
    visiting.push(procedure_id.to_owned());
    let expanded = match procedure.implementation {
        ActionProcedureImplementation::Inline { template } => {
            let substituted = substitute_action_procedure_parameters(
                &template,
                &substitutions,
                true,
                &format!("{path}.template"),
                diagnostics,
            )?;
            match serde_json::from_value::<RpgIrActionBody>(substituted) {
                Ok(body) => Some(body),
                Err(error) => {
                    diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Semantics,
                        "ACTION_PROCEDURE_TEMPLATE_INVALID",
                        format!("{path}.template"),
                        error.to_string(),
                    ));
                    None
                }
            }
        }
        ActionProcedureImplementation::Invocation {
            procedure_id: nested_procedure_id,
            procedure_owner_package_id: nested_procedure_owner_package_id,
            arguments: nested_arguments,
        } => {
            let arguments_value = Value::Object(
                nested_arguments
                    .into_iter()
                    .collect::<serde_json::Map<String, Value>>(),
            );
            let substituted = substitute_action_procedure_parameters(
                &arguments_value,
                &substitutions,
                false,
                &format!("{path}.arguments"),
                diagnostics,
            )?;
            let Value::Object(arguments_object) = substituted else {
                unreachable!("procedure invocation arguments remain an object");
            };
            let nested_arguments = arguments_object.into_iter().collect::<BTreeMap<_, _>>();
            expand_action_procedure(
                &nested_procedure_id,
                &nested_procedure_owner_package_id,
                &nested_arguments,
                definitions,
                ruleset,
                effective_references,
                visiting,
                allow_symbolic_references,
                &format!("{path}.procedure"),
                diagnostics,
            )
        }
    };
    visiting.pop();
    expanded
}

#[allow(clippy::too_many_arguments)]
fn normalize_procedure_argument(
    value: &Value,
    parameter: &ActionProcedureParameter,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset: &Ruleset,
    effective_references: &BTreeSet<String>,
    allow_symbolic_references: bool,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> Option<Value> {
    let normalized = match parameter {
        ActionProcedureParameter::BoundedInteger {
            minimum, maximum, ..
        } => value
            .as_i64()
            .filter(|value| value >= minimum && value <= maximum)
            .map(Value::from),
        ActionProcedureParameter::Identifier { .. } => value
            .as_str()
            .filter(|value| valid_identifier(value))
            .map(|value| Value::String(value.to_owned())),
        ActionProcedureParameter::Boolean { .. } => value.as_bool().map(Value::Bool),
        ActionProcedureParameter::Formula { .. } => strict_argument::<RpgIrFormula>(value),
        ActionProcedureParameter::Targeting { .. } => strict_argument::<RpgIrTargetSelector>(value),
        ActionProcedureParameter::Check { .. } => strict_argument::<RpgIrCheck>(value),
        ActionProcedureParameter::Costs { .. } => strict_argument::<Vec<RpgIrResourceCost>>(value),
        ActionProcedureParameter::Program { .. } => strict_argument::<RpgIrProgram>(value),
        ActionProcedureParameter::SemanticBranches { .. } => {
            let program = serde_json::from_value::<RpgIrProgram>(value.clone()).ok();
            program
                .filter(|program| matches!(program, RpgIrProgram::OnCheck { .. }))
                .and_then(|program| serde_json::to_value(program).ok())
        }
        ActionProcedureParameter::RulesetValueReference { .. } => {
            let reference =
                serde_json::from_value::<ProcedureRulesetValueReference>(value.clone()).ok();
            reference
                .filter(|reference| {
                    allow_symbolic_references
                        || (reference.ruleset_id == ruleset.identity.id
                            && ruleset.provides.values.iter().any(|candidate| {
                                candidate.kind == reference.kind && candidate.id == reference.id
                            }))
                })
                .and_then(|reference| serde_json::to_value(reference).ok())
        }
        ActionProcedureParameter::CatalogReference { .. } => {
            let reference = serde_json::from_value::<ProcedureCatalogReference>(value.clone()).ok();
            reference
                .filter(|reference| {
                    allow_symbolic_references
                        || (effective_references.contains(&reference.definition_id)
                            && definitions
                                .get(reference.definition_id.as_str())
                                .is_some_and(|definition| {
                                    definition.provenance.package_id == reference.package_id
                                        && definition.kind
                                            == MaterializedContentDefinitionKind::Support
                                        && definition
                                            .semantic
                                            .get("catalog")
                                            .and_then(Value::as_str)
                                            == Some(reference.category.as_str())
                                }))
                })
                .and_then(|reference| serde_json::to_value(reference).ok())
        }
    };
    if normalized.is_none() {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Semantics,
            "ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH",
            path,
            format!("argument does not satisfy {}", parameter.value_type()),
        ));
    }
    normalized
}

fn strict_argument<T>(value: &Value) -> Option<Value>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    serde_json::from_value::<T>(value.clone())
        .ok()
        .and_then(|value| serde_json::to_value(value).ok())
}

fn substitute_action_procedure_parameters(
    value: &Value,
    substitutions: &BTreeMap<&str, Value>,
    materialize_references: bool,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> Option<Value> {
    match value {
        Value::Array(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                substitute_action_procedure_parameters(
                    value,
                    substitutions,
                    materialize_references,
                    &format!("{path}[{index}]"),
                    diagnostics,
                )
            })
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        Value::Object(object)
            if object.get("kind").and_then(Value::as_str) == Some("parameter") =>
        {
            let parameter_id = object.get("parameterId").and_then(Value::as_str);
            if object.len() != 3 || parameter_id.is_none() {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "ACTION_PROCEDURE_PARAMETER_REFERENCE_INVALID",
                    path,
                    "parameter marker is malformed",
                ));
                return None;
            }
            let Some(value) = parameter_id.and_then(|id| substitutions.get(id)) else {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "ACTION_PROCEDURE_PARAMETER_UNRESOLVED",
                    path,
                    "parameter marker has no supplied argument",
                ));
                return None;
            };
            if materialize_references {
                match object.get("parameterType").and_then(Value::as_str) {
                    Some("catalogReference") => {
                        return value.get("definitionId").cloned();
                    }
                    Some("rulesetValueReference") => {
                        return value.get("id").cloned();
                    }
                    _ => {}
                }
            }
            Some(value.clone())
        }
        Value::Object(object) => object
            .iter()
            .map(|(field, value)| {
                substitute_action_procedure_parameters(
                    value,
                    substitutions,
                    materialize_references,
                    &format!("{path}.{field}"),
                    diagnostics,
                )
                .map(|value| (field.clone(), value))
            })
            .collect::<Option<serde_json::Map<_, _>>>()
            .map(Value::Object),
        _ => Some(value.clone()),
    }
}

fn resolve_action_catalogs(
    action: &mut RpgIrAction,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for (index, cost) in action.costs.iter_mut().enumerate() {
        resolve_catalog_reference(
            &mut cost.resource_id,
            CatalogReferenceKind::RESOURCE,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.costs[{index}].resourceId"),
            diagnostics,
        );
    }
    match &mut action.check {
        RpgIrCheck::NoRoll => {}
        RpgIrCheck::Attack {
            modifier,
            defense_id,
        } => {
            resolve_catalog_reference(
                defense_id,
                CatalogReferenceKind::DEFENSE,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.check.defenseId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                modifier,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.check.modifier"),
                diagnostics,
            );
        }
        RpgIrCheck::SavingThrow {
            difficulty,
            defense_id,
        } => {
            resolve_catalog_reference(
                defense_id,
                CatalogReferenceKind::DEFENSE,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.check.defenseId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                difficulty,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.check.difficulty"),
                diagnostics,
            );
        }
    }
    resolve_program_catalogs(
        &mut action.program,
        action_definition,
        definitions,
        ruleset_catalogs,
        &format!("{path}.program"),
        diagnostics,
    );
}

fn resolve_program_catalogs(
    program: &mut RpgIrProgram,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match program {
        RpgIrProgram::Operation { operation } => {
            resolve_operation_catalogs(
                operation,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.operation"),
                diagnostics,
            );
        }
        RpgIrProgram::Sequence { steps } => {
            for (index, step) in steps.iter_mut().enumerate() {
                resolve_program_catalogs(
                    step,
                    action_definition,
                    definitions,
                    ruleset_catalogs,
                    &format!("{path}.steps[{index}]"),
                    diagnostics,
                );
            }
        }
        RpgIrProgram::When {
            predicate,
            then,
            otherwise,
        } => {
            resolve_predicate_catalogs(
                predicate,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.predicate"),
                diagnostics,
            );
            resolve_program_catalogs(
                then,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.then"),
                diagnostics,
            );
            if let Some(otherwise) = otherwise {
                resolve_program_catalogs(
                    otherwise,
                    action_definition,
                    definitions,
                    ruleset_catalogs,
                    &format!("{path}.otherwise"),
                    diagnostics,
                );
            }
        }
        RpgIrProgram::Repeat { body, .. }
        | RpgIrProgram::ForEachTarget { body, .. }
        | RpgIrProgram::Atomic { body } => resolve_program_catalogs(
            body,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.body"),
            diagnostics,
        ),
        RpgIrProgram::OnCheck {
            hit,
            miss,
            saved,
            failed,
            no_roll,
        } => {
            for (branch_name, branch) in [
                ("hit", hit),
                ("miss", miss),
                ("saved", saved),
                ("failed", failed),
                ("noRoll", no_roll),
            ] {
                if let Some(branch) = branch {
                    resolve_program_catalogs(
                        branch,
                        action_definition,
                        definitions,
                        ruleset_catalogs,
                        &format!("{path}.{branch_name}"),
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn resolve_operation_catalogs(
    operation: &mut RpgIrOperation,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match operation {
        RpgIrOperation::Damage {
            amount,
            damage_type,
        } => {
            resolve_catalog_reference(
                damage_type,
                CatalogReferenceKind::DAMAGE_TYPE,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.damageType"),
                diagnostics,
            );
            resolve_formula_catalogs(
                amount,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.amount"),
                diagnostics,
            );
        }
        RpgIrOperation::Heal { amount } => resolve_formula_catalogs(
            amount,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.amount"),
            diagnostics,
        ),
        RpgIrOperation::ChangeResource {
            resource_id, delta, ..
        } => {
            resolve_catalog_reference(
                resource_id,
                CatalogReferenceKind::RESOURCE,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.resourceId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                delta,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.delta"),
                diagnostics,
            );
        }
        RpgIrOperation::ApplyModifier {
            modifier_id, value, ..
        } => {
            resolve_catalog_reference(
                modifier_id,
                CatalogReferenceKind::MODIFIER,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.modifierId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                value,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.value"),
                diagnostics,
            );
        }
        RpgIrOperation::Move {
            delta_x, delta_y, ..
        } => {
            resolve_formula_catalogs(
                delta_x,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.deltaX"),
                diagnostics,
            );
            resolve_formula_catalogs(
                delta_y,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.deltaY"),
                diagnostics,
            );
        }
        RpgIrOperation::MoveToCell { .. } => {}
        RpgIrOperation::OpenReaction { .. } => {}
    }
}

fn resolve_formula_catalogs(
    formula: &mut RpgIrFormula,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match formula {
        RpgIrFormula::ReadStat { stat_id, .. } => resolve_catalog_reference(
            stat_id,
            CatalogReferenceKind::STAT,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.statId"),
            diagnostics,
        ),
        RpgIrFormula::Add { terms } => {
            for (index, term) in terms.iter_mut().enumerate() {
                resolve_formula_catalogs(
                    term,
                    action_definition,
                    definitions,
                    ruleset_catalogs,
                    &format!("{path}.terms[{index}]"),
                    diagnostics,
                );
            }
        }
        RpgIrFormula::Half { value } => resolve_formula_catalogs(
            value,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.value"),
            diagnostics,
        ),
        RpgIrFormula::Constant { .. } | RpgIrFormula::Dice { .. } => {}
    }
}

fn resolve_predicate_catalogs(
    predicate: &mut RpgIrPredicate,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match predicate {
        RpgIrPredicate::Always => {}
        RpgIrPredicate::Compare { left, right, .. } => {
            resolve_formula_catalogs(
                left,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.left"),
                diagnostics,
            );
            resolve_formula_catalogs(
                right,
                action_definition,
                definitions,
                ruleset_catalogs,
                &format!("{path}.right"),
                diagnostics,
            );
        }
        RpgIrPredicate::Not { predicate } => resolve_predicate_catalogs(
            predicate,
            action_definition,
            definitions,
            ruleset_catalogs,
            &format!("{path}.predicate"),
            diagnostics,
        ),
        RpgIrPredicate::All { predicates } | RpgIrPredicate::Any { predicates } => {
            for (index, predicate) in predicates.iter_mut().enumerate() {
                resolve_predicate_catalogs(
                    predicate,
                    action_definition,
                    definitions,
                    ruleset_catalogs,
                    &format!("{path}.predicates[{index}]"),
                    diagnostics,
                );
            }
        }
    }
}

fn resolve_catalog_reference(
    value: &mut String,
    kind: CatalogReferenceKind,
    action_definition: &MaterializedContentDefinition,
    definitions: &BTreeMap<&str, &MaterializedContentDefinition>,
    ruleset_catalogs: &RulesetCatalogs,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let references_content_definition = action_definition
        .references
        .iter()
        .any(|reference| reference == value);
    if !references_content_definition && ruleset_catalogs.contains(kind.catalog, value) {
        return;
    }
    if !references_content_definition {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(kind.diagnostic, CatalogDiagnostic::ReferenceUndeclared),
            path,
            format!(
                "{} {value} must be a direct definition reference from {}",
                kind.catalog, action_definition.id
            ),
        ));
        return;
    }
    let Some(definition) = definitions.get(value.as_str()) else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(kind.diagnostic, CatalogDiagnostic::DefinitionMissing),
            path,
            format!("{} definition {value} is absent", kind.catalog),
        ));
        return;
    };
    if definition.kind != MaterializedContentDefinitionKind::Support {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(kind.diagnostic, CatalogDiagnostic::DefinitionKindInvalid),
            path,
            format!("{} definition {value} must be support data", kind.catalog),
        ));
        return;
    }
    let semantic =
        match serde_json::from_value::<CatalogDefinitionSemantic>(definition.semantic.clone()) {
            Ok(semantic) => semantic,
            Err(error) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    catalog_diagnostic_code(kind.diagnostic, CatalogDiagnostic::SemanticInvalid),
                    path,
                    error.to_string(),
                ));
                return;
            }
        };
    if semantic.catalog != kind.catalog {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(kind.diagnostic, CatalogDiagnostic::CatalogMismatch),
            path,
            format!(
                "definition {} belongs to catalog {}, not {}",
                definition.id, semantic.catalog, kind.catalog,
            ),
        ));
        return;
    }
    *value = semantic.id;
}

#[derive(Clone, Copy)]
enum CatalogDiagnostic {
    ReferenceUndeclared,
    DefinitionMissing,
    DefinitionKindInvalid,
    SemanticInvalid,
    CatalogMismatch,
}

fn catalog_diagnostic_code(kind: &str, diagnostic: CatalogDiagnostic) -> &'static str {
    match (kind, diagnostic) {
        ("DAMAGE_TYPE", CatalogDiagnostic::ReferenceUndeclared) => {
            "CONTENT_PACK_DAMAGE_TYPE_REFERENCE_UNDECLARED"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::DefinitionMissing) => {
            "CONTENT_PACK_DAMAGE_TYPE_DEFINITION_MISSING"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::DefinitionKindInvalid) => {
            "CONTENT_PACK_DAMAGE_TYPE_DEFINITION_KIND_INVALID"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::SemanticInvalid) => {
            "CONTENT_PACK_DAMAGE_TYPE_SEMANTIC_INVALID"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::CatalogMismatch) => {
            "CONTENT_PACK_DAMAGE_TYPE_CATALOG_MISMATCH"
        }
        (_, CatalogDiagnostic::ReferenceUndeclared) => "CONTENT_PACK_CATALOG_REFERENCE_UNDECLARED",
        (_, CatalogDiagnostic::DefinitionMissing) => "CONTENT_PACK_CATALOG_DEFINITION_MISSING",
        (_, CatalogDiagnostic::DefinitionKindInvalid) => {
            "CONTENT_PACK_CATALOG_DEFINITION_KIND_INVALID"
        }
        (_, CatalogDiagnostic::SemanticInvalid) => "CONTENT_PACK_CATALOG_SEMANTIC_INVALID",
        (_, CatalogDiagnostic::CatalogMismatch) => "CONTENT_PACK_CATALOG_MISMATCH",
    }
}

fn collect_action_catalogs(action: &RpgIrAction, catalogs: &mut DerivedCatalogs) {
    for cost in &action.costs {
        catalogs.resources.insert(cost.resource_id.clone());
    }
    match &action.check {
        RpgIrCheck::NoRoll => {}
        RpgIrCheck::Attack {
            modifier,
            defense_id,
        } => {
            catalogs.defenses.insert(defense_id.clone());
            collect_formula_catalogs(modifier, catalogs);
        }
        RpgIrCheck::SavingThrow {
            difficulty,
            defense_id,
        } => {
            catalogs.defenses.insert(defense_id.clone());
            collect_formula_catalogs(difficulty, catalogs);
        }
    }
    collect_program_catalogs(&action.program, catalogs);
}

fn collect_program_catalogs(program: &RpgIrProgram, catalogs: &mut DerivedCatalogs) {
    match program {
        RpgIrProgram::Operation { operation } => collect_operation_catalogs(operation, catalogs),
        RpgIrProgram::Sequence { steps } => {
            for step in steps {
                collect_program_catalogs(step, catalogs);
            }
        }
        RpgIrProgram::When {
            predicate,
            then,
            otherwise,
        } => {
            collect_predicate_catalogs(predicate, catalogs);
            collect_program_catalogs(then, catalogs);
            if let Some(otherwise) = otherwise {
                collect_program_catalogs(otherwise, catalogs);
            }
        }
        RpgIrProgram::Repeat { body, .. }
        | RpgIrProgram::ForEachTarget { body, .. }
        | RpgIrProgram::Atomic { body } => collect_program_catalogs(body, catalogs),
        RpgIrProgram::OnCheck {
            hit,
            miss,
            saved,
            failed,
            no_roll,
        } => {
            for branch in [hit, miss, saved, failed, no_roll].into_iter().flatten() {
                collect_program_catalogs(branch, catalogs);
            }
        }
    }
}

fn collect_operation_catalogs(operation: &RpgIrOperation, catalogs: &mut DerivedCatalogs) {
    match operation {
        RpgIrOperation::Damage { amount, .. } | RpgIrOperation::Heal { amount } => {
            collect_formula_catalogs(amount, catalogs);
        }
        RpgIrOperation::ChangeResource {
            resource_id, delta, ..
        } => {
            catalogs.resources.insert(resource_id.clone());
            collect_formula_catalogs(delta, catalogs);
        }
        RpgIrOperation::ApplyModifier {
            modifier_id, value, ..
        } => {
            catalogs.modifiers.insert(modifier_id.clone());
            collect_formula_catalogs(value, catalogs);
        }
        RpgIrOperation::Move {
            delta_x, delta_y, ..
        } => {
            collect_formula_catalogs(delta_x, catalogs);
            collect_formula_catalogs(delta_y, catalogs);
        }
        RpgIrOperation::MoveToCell { .. } => {}
        RpgIrOperation::OpenReaction { .. } => {}
    }
}

fn collect_formula_catalogs(formula: &RpgIrFormula, catalogs: &mut DerivedCatalogs) {
    match formula {
        RpgIrFormula::Constant { .. } | RpgIrFormula::Dice { .. } => {}
        RpgIrFormula::ReadStat { stat_id, .. } => {
            catalogs.stats.insert(stat_id.clone());
        }
        RpgIrFormula::Add { terms } => {
            for term in terms {
                collect_formula_catalogs(term, catalogs);
            }
        }
        RpgIrFormula::Half { value } => collect_formula_catalogs(value, catalogs),
    }
}

fn collect_predicate_catalogs(predicate: &RpgIrPredicate, catalogs: &mut DerivedCatalogs) {
    match predicate {
        RpgIrPredicate::Always => {}
        RpgIrPredicate::Compare { left, right, .. } => {
            collect_formula_catalogs(left, catalogs);
            collect_formula_catalogs(right, catalogs);
        }
        RpgIrPredicate::Not { predicate } => collect_predicate_catalogs(predicate, catalogs),
        RpgIrPredicate::All { predicates } | RpgIrPredicate::Any { predicates } => {
            for predicate in predicates {
                collect_predicate_catalogs(predicate, catalogs);
            }
        }
    }
}

fn validate_materialization_provenance(
    prepared: &PreparedPlayBundle,
    definition_commitments: &BTreeMap<String, &ContentDefinitionCommitment>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let definitions = prepared
        .materialized_definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect::<BTreeMap<_, _>>();
    let mut validated_derivation_stages = BTreeMap::new();
    let mut previous_derivation = None::<&str>;
    for (index, provenance) in prepared.derivation_provenance.iter().enumerate() {
        if previous_derivation.is_some_and(|previous| previous >= provenance.definition_id.as_str())
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_PROVENANCE_NOT_CANONICAL",
                format!("$.derivationProvenance[{index}]"),
                "derivation provenance must be strictly definition-sorted",
            ));
        }
        previous_derivation = Some(&provenance.definition_id);
        if !definitions.contains_key(provenance.definition_id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DERIVATION_TARGET_MISSING",
                format!("$.derivationProvenance[{index}].definitionId"),
                format!(
                    "derived definition {} is not materialized",
                    provenance.definition_id
                ),
            ));
        }
        for (field, value) in [
            ("baseFingerprint", provenance.base_fingerprint.as_str()),
            (
                "localPatchFingerprint",
                provenance.local_patch_fingerprint.as_str(),
            ),
            (
                "materializedFingerprint",
                provenance.materialized_fingerprint.as_str(),
            ),
        ] {
            if !valid_fingerprint(value) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_FINGERPRINT_INVALID",
                    format!("$.derivationProvenance[{index}].{field}"),
                    "derivation fingerprints must be fnv1a64 with sixteen lowercase hex digits",
                ));
            }
        }
        for (mixin_index, mixin) in provenance.mixins.iter().enumerate() {
            if mixin.order != mixin_index || !valid_fingerprint(&mixin.fingerprint) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_MIXIN_NOT_CANONICAL",
                    format!("$.derivationProvenance[{index}].mixins[{mixin_index}]"),
                    "mixin provenance must preserve contiguous order and exact fingerprints",
                ));
            }
            if mixin
                .parameters
                .values()
                .any(|value| !value.is_string() && !value.is_number() && !value.is_boolean())
            {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_MIXIN_PARAMETER_INVALID",
                    format!("$.derivationProvenance[{index}].mixins[{mixin_index}].parameters"),
                    "mixin parameters must be scalar immutable values",
                ));
            }
        }
        validate_patch_changes(
            &provenance.changes,
            &format!("$.derivationProvenance[{index}].changes"),
            diagnostics,
        );
        if let Some(stage) =
            validate_derivation_semantics(provenance, index, definition_commitments, diagnostics)
        {
            validated_derivation_stages.insert(provenance.definition_id.as_str(), stage);
        }
    }

    let mut overlays_by_definition = BTreeMap::<&str, Vec<_>>::new();
    let mut previous_overlay_order = None::<usize>;
    for (index, provenance) in prepared.overlay_provenance.iter().enumerate() {
        if previous_overlay_order.is_some_and(|previous| previous >= provenance.order) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_PROVENANCE_NOT_CANONICAL",
                format!("$.overlayProvenance[{index}].order"),
                "overlay provenance order must be strictly increasing",
            ));
        }
        previous_overlay_order = Some(provenance.order);
        overlays_by_definition
            .entry(provenance.target_definition_id.as_str())
            .or_default()
            .push(provenance);
        if !definitions.contains_key(provenance.target_definition_id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_OVERLAY_TARGET_MISSING",
                format!("$.overlayProvenance[{index}].targetDefinitionId"),
                format!(
                    "overlay target {} is not materialized",
                    provenance.target_definition_id
                ),
            ));
        }
        for (field, value) in [
            (
                "expectedFingerprint",
                provenance.expected_fingerprint.as_str(),
            ),
            ("beforeFingerprint", provenance.before_fingerprint.as_str()),
            ("afterFingerprint", provenance.after_fingerprint.as_str()),
            ("patchFingerprint", provenance.patch_fingerprint.as_str()),
        ] {
            if !valid_fingerprint(value) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_OVERLAY_FINGERPRINT_INVALID",
                    format!("$.overlayProvenance[{index}].{field}"),
                    "overlay fingerprints must be fnv1a64 with sixteen lowercase hex digits",
                ));
            }
        }
        if provenance.expected_fingerprint != provenance.before_fingerprint {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_EXPECTED_FINGERPRINT_MISMATCH",
                format!("$.overlayProvenance[{index}].expectedFingerprint"),
                "the pinned expected fingerprint must equal the observed pre-overlay fingerprint",
            ));
        }
        validate_patch_changes(
            &provenance.changes,
            &format!("$.overlayProvenance[{index}].changes"),
            diagnostics,
        );
    }

    for overlays in overlays_by_definition.values_mut() {
        overlays.sort_by_key(|provenance| provenance.order);
    }
    for (definition_id, definition) in &definitions {
        let derivation_stage = validated_derivation_stages.get(definition_id).cloned();
        let commitment_identity = definition_commitment_identity(
            &definition.provenance.package_id,
            &definition.provenance.package_version,
            definition_id,
        );
        let committed_stage = match definition_commitments.get(&commitment_identity) {
            Some(ContentDefinitionCommitment::Concrete { stage, .. }) => Some(stage.clone()),
            _ => None,
        };
        if let (Some(replayed), Some(committed)) = (&derivation_stage, &committed_stage) {
            if replayed != committed {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_COMMITMENT_REPLAY_MISMATCH",
                    "$.definitionCommitments",
                    format!(
                        "the replayed derivation stage for {definition_id} does not equal its committed pre-overlay stage"
                    ),
                ));
            }
        }
        let initial_stage = committed_stage.or(derivation_stage);
        if let Some(entries) = overlays_by_definition.get(definition_id) {
            match initial_stage {
                Some(stage) => {
                    validate_overlay_fingerprint_chain(definition, stage, entries, diagnostics);
                }
                None => diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "CONTENT_PACK_OVERLAY_INITIAL_COMMITMENT_MISSING",
                    "$.definitionCommitments",
                    format!(
                        "overlay target {definition_id} requires a committed pre-overlay stage"
                    ),
                )),
            }
        } else if let Some(stage) = initial_stage {
            let final_stage = materialization_stage(definition);
            if stage != final_stage {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_MATERIALIZED_STAGE_MISMATCH",
                    "$.derivationProvenance",
                    format!(
                        "the replayed derivation stage for {definition_id} does not equal the materialized definition"
                    ),
                ));
            }
        }
    }

    let mut declared_derivations = BTreeMap::new();
    for relationship in prepared
        .relationships
        .iter()
        .filter(|relationship| matches!(relationship.kind, ContentRelationshipKind::DerivesFrom))
    {
        *declared_derivations
            .entry((
                relationship.source.clone(),
                relationship.target.clone(),
                relationship.order,
            ))
            .or_insert(0_usize) += 1;
    }
    let mut proven_derivations = BTreeMap::new();
    for provenance in &prepared.derivation_provenance {
        let source = materialization_relationship_identity(
            &provenance.package_id,
            &provenance.package_version,
            &provenance.definition_id,
        );
        let base = materialization_relationship_identity(
            &provenance.base_package_id,
            &provenance.base_package_version,
            &provenance.base_definition_id,
        );
        *proven_derivations
            .entry((source.clone(), base, 0_usize))
            .or_insert(0_usize) += 1;
        for mixin in &provenance.mixins {
            let target = materialization_relationship_identity(
                &mixin.package_id,
                &mixin.package_version,
                &mixin.definition_id,
            );
            *proven_derivations
                .entry((source.clone(), target, mixin.order))
                .or_insert(0_usize) += 1;
        }
    }
    if declared_derivations != proven_derivations {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_PROVENANCE_COVERAGE_MISMATCH",
            "$.relationships",
            "each derivation base and mixin relationship requires one matching provenance record",
        ));
    }

    let mut declared_overlays = BTreeMap::new();
    for relationship in prepared.relationships.iter().filter(|relationship| {
        matches!(relationship.kind, ContentRelationshipKind::Patches)
            && relationship.target.contains('#')
    }) {
        *declared_overlays
            .entry((
                relationship.source.clone(),
                relationship.target.clone(),
                relationship.order,
            ))
            .or_insert(0_usize) += 1;
    }
    let mut proven_overlays = BTreeMap::new();
    for provenance in &prepared.overlay_provenance {
        let source = format!(
            "{}@{}",
            provenance.overlay_package_id, provenance.overlay_package_version
        );
        let target = materialization_relationship_identity(
            &provenance.target_package_id,
            &provenance.target_package_version,
            &provenance.target_definition_id,
        );
        *proven_overlays
            .entry((source, target, provenance.order))
            .or_insert(0_usize) += 1;
    }
    if declared_overlays != proven_overlays {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_OVERLAY_PROVENANCE_COVERAGE_MISMATCH",
            "$.relationships",
            "each overlay relationship requires one matching provenance record",
        ));
    }
}

fn materialization_relationship_identity(
    package_id: &str,
    package_version: &str,
    definition_id: &str,
) -> String {
    format!("{package_id}@{package_version}#{definition_id}")
}

fn materialization_stage(
    definition: &MaterializedContentDefinition,
) -> ContentMaterializationStage {
    let semantic = if definition.kind == MaterializedContentDefinitionKind::Action
        && definition.semantic.get("kind").and_then(Value::as_str) == Some("inline")
    {
        definition
            .semantic
            .get("action")
            .cloned()
            .unwrap_or_else(|| definition.semantic.clone())
    } else {
        definition.semantic.clone()
    };
    ContentMaterializationStage {
        id: definition.id.clone(),
        kind: definition.kind,
        extension_policy: definition.extension_policy,
        value: ContentMaterializationValue {
            semantic,
            presentation: definition.presentation.clone(),
        },
        references: definition.references.clone(),
    }
}

fn canonical_fingerprint(value: &impl Serialize) -> Result<String, RpgCompileFailure> {
    let canonical = serde_json::to_value(value).map_err(fingerprint_error)?;
    fingerprint(&canonical)
}

fn validate_derivation_semantics(
    provenance: &rpg_ir::ContentDerivationProvenance,
    provenance_index: usize,
    definition_commitments: &BTreeMap<String, &ContentDefinitionCommitment>,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> Option<ContentMaterializationStage> {
    let path = format!("$.derivationProvenance[{provenance_index}]");
    if provenance.base.id != provenance.base_definition_id {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_BASE_STAGE_MISMATCH",
            format!("{path}.base.id"),
            "the base stage identity must match baseDefinitionId",
        ));
    }
    let base_commitment_identity = definition_commitment_identity(
        &provenance.base_package_id,
        &provenance.base_package_version,
        &provenance.base_definition_id,
    );
    match definition_commitments.get(&base_commitment_identity) {
        Some(ContentDefinitionCommitment::Concrete {
            fingerprint, stage, ..
        }) => {
            if stage != &provenance.base || fingerprint != &provenance.base_fingerprint {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_BASE_COMMITMENT_MISMATCH",
                    format!("{path}.base"),
                    "the replay base must equal the independently committed named definition",
                ));
            }
        }
        Some(ContentDefinitionCommitment::Mixin { .. }) => diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_BASE_COMMITMENT_KIND_MISMATCH",
            format!("{path}.baseDefinitionId"),
            "a derivation base must resolve to a concrete definition commitment",
        )),
        None => diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "CONTENT_PACK_DERIVATION_BASE_COMMITMENT_MISSING",
            format!("{path}.baseDefinitionId"),
            format!("missing source commitment {base_commitment_identity}"),
        )),
    }
    match canonical_fingerprint(&provenance.base) {
        Ok(actual) if actual != provenance.base_fingerprint => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_BASE_FINGERPRINT_MISMATCH",
                format!("{path}.baseFingerprint"),
                format!("the base stage fingerprints as {actual}"),
            ))
        }
        Err(failure) => diagnostics.extend(failure.diagnostics),
        _ => {}
    }

    let mut stage = provenance.base.clone();
    let mut computed_changes = Vec::new();
    for (mixin_index, mixin) in provenance.mixins.iter().enumerate() {
        let mixin_commitment_identity = definition_commitment_identity(
            &mixin.package_id,
            &mixin.package_version,
            &mixin.definition_id,
        );
        match definition_commitments.get(&mixin_commitment_identity) {
            Some(ContentDefinitionCommitment::Mixin { value, .. }) => {
                if value.patch != mixin.patch {
                    diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Artifact,
                        "CONTENT_PACK_DERIVATION_MIXIN_COMMITMENT_MISMATCH",
                        format!("{path}.mixins[{mixin_index}].patch"),
                        "the replay patch must equal the independently committed named mixin definition",
                    ));
                }
                validate_applied_mixin_parameters(
                    &value.parameters,
                    &mixin.parameters,
                    &format!("{path}.mixins[{mixin_index}].parameters"),
                    diagnostics,
                );
            }
            Some(ContentDefinitionCommitment::Concrete { .. }) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_MIXIN_COMMITMENT_KIND_MISMATCH",
                    format!("{path}.mixins[{mixin_index}].definitionId"),
                    "a derivation mixin must resolve to a mixin definition commitment",
                ))
            }
            None => diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "CONTENT_PACK_DERIVATION_MIXIN_COMMITMENT_MISSING",
                format!("{path}.mixins[{mixin_index}].definitionId"),
                format!("missing source commitment {mixin_commitment_identity}"),
            )),
        }
        match canonical_fingerprint(&mixin.patch) {
            Ok(actual) if actual != mixin.fingerprint => diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_MIXIN_FINGERPRINT_MISMATCH",
                format!("{path}.mixins[{mixin_index}].fingerprint"),
                format!("the mixin patch fingerprints as {actual}"),
            )),
            Err(failure) => diagnostics.extend(failure.diagnostics),
            _ => {}
        }
        match apply_ruleset_patch(&stage, &mixin.patch, &mixin.parameters) {
            Ok((next, changes)) => {
                stage = next;
                computed_changes.extend(changes);
            }
            Err(message) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_PATCH_REPLAY_FAILED",
                    format!("{path}.mixins[{mixin_index}].patch"),
                    message,
                ));
                return None;
            }
        }
    }

    match canonical_fingerprint(&provenance.local_patch) {
        Ok(actual) if actual != provenance.local_patch_fingerprint => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_LOCAL_PATCH_FINGERPRINT_MISMATCH",
                format!("{path}.localPatchFingerprint"),
                format!("the local patch fingerprints as {actual}"),
            ))
        }
        Err(failure) => diagnostics.extend(failure.diagnostics),
        _ => {}
    }
    match apply_ruleset_patch(&stage, &provenance.local_patch, &BTreeMap::new()) {
        Ok((next, changes)) => {
            stage = next;
            computed_changes.extend(changes);
        }
        Err(message) => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_PATCH_REPLAY_FAILED",
                format!("{path}.localPatch"),
                message,
            ));
            return None;
        }
    }

    materialize_derived_identity(&mut stage, &provenance.materialized);
    if stage != provenance.materialized {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_MATERIALIZED_STAGE_MISMATCH",
            format!("{path}.materialized"),
            "replaying the base, mixins, and local patch does not produce the claimed materialized stage",
        ));
    }
    let target_commitment_identity = definition_commitment_identity(
        &provenance.package_id,
        &provenance.package_version,
        &provenance.definition_id,
    );
    match definition_commitments.get(&target_commitment_identity) {
        Some(ContentDefinitionCommitment::Concrete {
            fingerprint,
            stage: committed_stage,
            ..
        }) => {
            if committed_stage != &provenance.materialized
                || fingerprint != &provenance.materialized_fingerprint
            {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_DERIVATION_TARGET_COMMITMENT_MISMATCH",
                    format!("{path}.materialized"),
                    "the derived stage must equal its independent pre-overlay commitment",
                ));
            }
        }
        Some(ContentDefinitionCommitment::Mixin { .. }) => diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_TARGET_COMMITMENT_KIND_MISMATCH",
            format!("{path}.definitionId"),
            "a derived target must resolve to a concrete definition commitment",
        )),
        None => diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            "CONTENT_PACK_DERIVATION_TARGET_COMMITMENT_MISSING",
            format!("{path}.definitionId"),
            format!("missing source commitment {target_commitment_identity}"),
        )),
    }
    if computed_changes != provenance.changes {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_DERIVATION_CHANGE_COVERAGE_MISMATCH",
            format!("{path}.changes"),
            "the submitted derivation changes do not exactly match authoritative patch replay",
        ));
    }
    match canonical_fingerprint(&provenance.materialized) {
        Ok(actual) if actual != provenance.materialized_fingerprint => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_DERIVATION_MATERIALIZED_FINGERPRINT_MISMATCH",
                format!("{path}.materializedFingerprint"),
                format!("the materialized derivation stage fingerprints as {actual}"),
            ))
        }
        Err(failure) => diagnostics.extend(failure.diagnostics),
        _ => {}
    }
    Some(stage)
}

fn materialize_derived_identity(
    stage: &mut ContentMaterializationStage,
    materialized: &ContentMaterializationStage,
) {
    stage.id.clone_from(&materialized.id);
    stage.kind = materialized.kind;
    stage.extension_policy = materialized.extension_policy;
    stage.references.clone_from(&materialized.references);
    if materialized.kind == MaterializedContentDefinitionKind::Action {
        let materialized_semantic = materialized.value.semantic.as_object();
        if let (Some(stage_semantic), Some(materialized_semantic)) =
            (stage.value.semantic.as_object_mut(), materialized_semantic)
        {
            for field in ["id", "sourcePath"] {
                if let Some(value) = materialized_semantic.get(field) {
                    stage_semantic.insert(field.to_owned(), value.clone());
                }
            }
        }
    }
}

fn validate_overlay_fingerprint_chain(
    final_definition: &MaterializedContentDefinition,
    mut stage: ContentMaterializationStage,
    overlays: &[&rpg_ir::ContentOverlayProvenance],
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for provenance in overlays {
        let path = format!("$.overlayProvenance[order={}]", provenance.order);
        if stage != provenance.before {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_BEFORE_STAGE_MISMATCH",
                format!("{path}.before"),
                "the submitted pre-overlay stage does not equal the preceding materialization stage",
            ));
        }
        let before_fingerprint = match canonical_fingerprint(&stage) {
            Ok(fingerprint) => fingerprint,
            Err(failure) => {
                diagnostics.extend(failure.diagnostics);
                return;
            }
        };
        for (field, expected) in [
            ("expectedFingerprint", &provenance.expected_fingerprint),
            ("beforeFingerprint", &provenance.before_fingerprint),
        ] {
            if expected != &before_fingerprint {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_OVERLAY_BEFORE_FINGERPRINT_MISMATCH",
                    format!("{path}.{field}"),
                    format!(
                        "the authoritative pre-overlay stage fingerprints as {before_fingerprint}"
                    ),
                ));
            }
        }
        match canonical_fingerprint(&provenance.patch) {
            Ok(actual) if actual != provenance.patch_fingerprint => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "CONTENT_PACK_OVERLAY_PATCH_FINGERPRINT_MISMATCH",
                    format!("{path}.patchFingerprint"),
                    format!("the overlay patch fingerprints as {actual}"),
                ))
            }
            Err(failure) => diagnostics.extend(failure.diagnostics),
            _ => {}
        }
        if !patch_matches_plane(&provenance.patch, provenance.plane) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_IMPACT_PLANE_MISMATCH",
                format!("{path}.patch"),
                "overlay patch operations exceed the declared impact plane",
            ));
        }
        let (next, computed_changes) =
            match apply_ruleset_patch(&stage, &provenance.patch, &BTreeMap::new()) {
                Ok(result) => result,
                Err(message) => {
                    diagnostics.push(RpgDiagnostic::error(
                        RpgDiagnosticStage::Artifact,
                        "CONTENT_PACK_OVERLAY_PATCH_REPLAY_FAILED",
                        format!("{path}.patch"),
                        message,
                    ));
                    return;
                }
            };
        if computed_changes != provenance.changes {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_CHANGE_COVERAGE_MISMATCH",
                format!("{path}.changes"),
                "the submitted overlay changes do not exactly match authoritative patch replay",
            ));
        }
        let after_fingerprint = match canonical_fingerprint(&next) {
            Ok(fingerprint) => fingerprint,
            Err(failure) => {
                diagnostics.extend(failure.diagnostics);
                return;
            }
        };
        if after_fingerprint != provenance.after_fingerprint {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_OVERLAY_AFTER_FINGERPRINT_MISMATCH",
                format!("{path}.afterFingerprint"),
                format!("the authoritative post-overlay stage fingerprints as {after_fingerprint}"),
            ));
        }
        stage = next;
    }
    if stage != materialization_stage(final_definition) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_OVERLAY_FINAL_STAGE_MISMATCH",
            "$.overlayProvenance",
            format!(
                "replaying overlays for {} does not produce the materialized definition",
                final_definition.id
            ),
        ));
    }
}

fn apply_ruleset_patch(
    stage: &ContentMaterializationStage,
    patch: &ContentPatch,
    parameters: &BTreeMap<String, Value>,
) -> Result<
    (
        ContentMaterializationStage,
        Vec<ContentPatchChangeProvenance>,
    ),
    String,
> {
    if patch.version != 1 {
        return Err(format!("patch version {} is unsupported", patch.version));
    }
    let mut next = stage.clone();
    if next.value.presentation.is_null() {
        next.value.presentation = json!({});
    }
    let mut changes = Vec::new();
    for operation in &patch.operations {
        let plane = patch_operation_plane(operation);
        if plane == ContentImpactPlane::Both {
            return Err(
                "an individual patch operation must name semantic or presentation".to_owned(),
            );
        }
        let root = patch_plane_root_mut(&mut next, plane)?;
        let path = patch_operation_path(operation);
        if matches!(operation, ContentPatchOperation::UpsertScalar { .. })
            && !supported_patch_upsert(plane, path)
        {
            return Err(format!(
                "upsertScalar is not supported at {:?}.{}",
                plane,
                patch_change_path(path)
            ));
        }
        let before = match operation {
            ContentPatchOperation::UpsertScalar { .. } => read_upsert_patch_path(root, path)?
                .cloned()
                .unwrap_or(Value::Null),
            _ => read_patch_path(root, path)?.clone(),
        };
        match operation {
            ContentPatchOperation::SetScalar { value, .. } => {
                let replacement = resolve_patch_scalar(value, parameters)?;
                write_patch_path(root, path, replacement)?;
            }
            ContentPatchOperation::UpsertScalar { value, .. } => {
                let replacement = resolve_patch_scalar(value, parameters)?;
                write_upsert_patch_path(root, path, replacement)?;
            }
            ContentPatchOperation::AdjustNumber { multiply, add, .. } => {
                let current = read_patch_path(root, path)?
                    .as_f64()
                    .ok_or_else(|| "adjustNumber requires a numeric target".to_owned())?;
                let multiplier = resolve_patch_number(multiply, parameters)?;
                let addend = resolve_patch_number(add, parameters)?;
                write_patch_path(root, path, json_number(current * multiplier + addend)?)?;
            }
            ContentPatchOperation::AppendMember {
                identity,
                value,
                position,
                ..
            } => append_patch_member(root, path, identity, value, position)?,
            ContentPatchOperation::RemoveMember { identity, .. } => {
                remove_patch_member(root, path, identity)?;
            }
        }
        let after = read_patch_path(root, path)?.clone();
        changes.push(ContentPatchChangeProvenance {
            plane,
            path: patch_change_path(path),
            path_segments: path.to_vec(),
            effective: before != after,
            before,
            after,
        });
    }
    if next
        .value
        .presentation
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
    {
        next.value.presentation = Value::Null;
    }
    Ok((next, changes))
}

fn patch_operation_plane(operation: &ContentPatchOperation) -> ContentImpactPlane {
    match operation {
        ContentPatchOperation::SetScalar { plane, .. }
        | ContentPatchOperation::UpsertScalar { plane, .. }
        | ContentPatchOperation::AdjustNumber { plane, .. }
        | ContentPatchOperation::AppendMember { plane, .. }
        | ContentPatchOperation::RemoveMember { plane, .. } => *plane,
    }
}

fn patch_operation_path(operation: &ContentPatchOperation) -> &[ContentPatchPathSegment] {
    match operation {
        ContentPatchOperation::SetScalar { path, .. }
        | ContentPatchOperation::UpsertScalar { path, .. }
        | ContentPatchOperation::AdjustNumber { path, .. }
        | ContentPatchOperation::AppendMember { path, .. }
        | ContentPatchOperation::RemoveMember { path, .. } => path,
    }
}

fn patch_matches_plane(patch: &ContentPatch, plane: ContentImpactPlane) -> bool {
    patch.operations.iter().all(|operation| {
        plane == ContentImpactPlane::Both || patch_operation_plane(operation) == plane
    })
}

fn supported_patch_upsert(plane: ContentImpactPlane, path: &[ContentPatchPathSegment]) -> bool {
    plane == ContentImpactPlane::Presentation
        && matches!(
            path,
            [ContentPatchPathSegment::Field { name }] if name == "description"
        )
}

fn patch_plane_root_mut(
    stage: &mut ContentMaterializationStage,
    plane: ContentImpactPlane,
) -> Result<&mut Value, String> {
    match plane {
        ContentImpactPlane::Semantic => Ok(&mut stage.value.semantic),
        ContentImpactPlane::Presentation => Ok(&mut stage.value.presentation),
        ContentImpactPlane::Both => {
            Err("an individual patch operation cannot target both planes".to_owned())
        }
    }
}

fn read_patch_path<'a>(
    root: &'a Value,
    path: &[ContentPatchPathSegment],
) -> Result<&'a Value, String> {
    let mut current = root;
    for segment in path {
        current = match segment {
            ContentPatchPathSegment::Field { name } => current
                .as_object()
                .and_then(|object| object.get(name))
                .ok_or_else(|| format!("field {name} is missing at {}", patch_change_path(path)))?,
            ContentPatchPathSegment::Member { key, value } => {
                resolve_patch_member(current, *key, value)?
            }
        };
    }
    Ok(current)
}

fn read_upsert_patch_path<'a>(
    root: &'a Value,
    path: &[ContentPatchPathSegment],
) -> Result<Option<&'a Value>, String> {
    let Some((leaf, parent_path)) = path.split_last() else {
        return Err("patch operations must not write the root".to_owned());
    };
    let ContentPatchPathSegment::Field { name } = leaf else {
        return Err("patch upserts must end in a writable field".to_owned());
    };
    let parent = read_patch_path(root, parent_path)?;
    let object = parent
        .as_object()
        .ok_or_else(|| format!("upsert parent for {name} must be an object"))?;
    Ok(object.get(name))
}

fn read_patch_path_mut<'a>(
    root: &'a mut Value,
    path: &[ContentPatchPathSegment],
) -> Result<&'a mut Value, String> {
    let mut current = root;
    for segment in path {
        current = match segment {
            ContentPatchPathSegment::Field { name } => current
                .as_object_mut()
                .and_then(|object| object.get_mut(name))
                .ok_or_else(|| format!("field {name} is missing at {}", patch_change_path(path)))?,
            ContentPatchPathSegment::Member { key, value } => {
                resolve_patch_member_mut(current, *key, value)?
            }
        };
    }
    Ok(current)
}

fn write_patch_path(
    root: &mut Value,
    path: &[ContentPatchPathSegment],
    replacement: Value,
) -> Result<(), String> {
    let Some((leaf, parent_path)) = path.split_last() else {
        return Err("patch operations must not write the root".to_owned());
    };
    let ContentPatchPathSegment::Field { name } = leaf else {
        return Err("patch writes must end in a writable field".to_owned());
    };
    let parent = read_patch_path_mut(root, parent_path)?;
    let current = parent
        .as_object_mut()
        .and_then(|object| object.get_mut(name))
        .ok_or_else(|| format!("writable field {name} is missing"))?;
    *current = replacement;
    Ok(())
}

fn write_upsert_patch_path(
    root: &mut Value,
    path: &[ContentPatchPathSegment],
    replacement: Value,
) -> Result<(), String> {
    let Some((leaf, parent_path)) = path.split_last() else {
        return Err("patch operations must not write the root".to_owned());
    };
    let ContentPatchPathSegment::Field { name } = leaf else {
        return Err("patch upserts must end in a writable field".to_owned());
    };
    let parent = read_patch_path_mut(root, parent_path)?;
    let object = parent
        .as_object_mut()
        .ok_or_else(|| format!("upsert parent for {name} must be an object"))?;
    object.insert(name.clone(), replacement);
    Ok(())
}

fn resolve_patch_member<'a>(
    value: &'a Value,
    key: ContentPatchMemberKey,
    expected: &str,
) -> Result<&'a Value, String> {
    let list = value.as_array().ok_or_else(|| {
        format!(
            "member selector [{}={expected}] requires a list",
            patch_member_key(key)
        )
    })?;
    let matches = list
        .iter()
        .filter(|entry| patch_member_matches(entry, key, expected))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "member selector [{}={expected}] resolved {} entries",
            patch_member_key(key),
            matches.len()
        ));
    }
    Ok(matches[0])
}

fn resolve_patch_member_mut<'a>(
    value: &'a mut Value,
    key: ContentPatchMemberKey,
    expected: &str,
) -> Result<&'a mut Value, String> {
    let list = value.as_array_mut().ok_or_else(|| {
        format!(
            "member selector [{}={expected}] requires a list",
            patch_member_key(key)
        )
    })?;
    let indexes = list
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| patch_member_matches(entry, key, expected).then_some(index))
        .collect::<Vec<_>>();
    if indexes.len() != 1 {
        return Err(format!(
            "member selector [{}={expected}] resolved {} entries",
            patch_member_key(key),
            indexes.len()
        ));
    }
    Ok(&mut list[indexes[0]])
}

fn patch_member_matches(value: &Value, key: ContentPatchMemberKey, expected: &str) -> bool {
    value
        .as_object()
        .and_then(|object| object.get(patch_member_key(key)))
        .and_then(Value::as_str)
        .is_some_and(|actual| actual == expected)
}

fn resolve_patch_scalar(
    value: &Value,
    parameters: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    if let Some(parameter) = patch_parameter(value) {
        return parameters
            .get(parameter)
            .filter(|value| value.is_string() || value.is_number() || value.is_boolean())
            .cloned()
            .ok_or_else(|| format!("scalar parameter {parameter} is not supplied"));
    }
    if value.is_null() || value.is_string() || value.is_number() || value.is_boolean() {
        return Ok(value.clone());
    }
    Err("setScalar requires a scalar value or parameter reference".to_owned())
}

fn resolve_patch_number(
    value: &Value,
    parameters: &BTreeMap<String, Value>,
) -> Result<f64, String> {
    if let Some(parameter) = patch_parameter(value) {
        return parameters
            .get(parameter)
            .and_then(Value::as_f64)
            .ok_or_else(|| format!("numeric parameter {parameter} is not supplied"));
    }
    value
        .as_f64()
        .ok_or_else(|| "adjustNumber operands must be numeric".to_owned())
}

fn patch_parameter(value: &Value) -> Option<&str> {
    value
        .as_object()
        .and_then(|object| object.get("parameter"))
        .and_then(Value::as_str)
}

fn json_number(value: f64) -> Result<Value, String> {
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        return Ok(Value::from(value as i64));
    }
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| "adjustNumber produced a non-finite value".to_owned())
}

fn append_patch_member(
    root: &mut Value,
    path: &[ContentPatchPathSegment],
    identity: &ContentPatchMemberSelector,
    value: &BTreeMap<String, Value>,
    position: &ContentPatchPosition,
) -> Result<(), String> {
    let target = read_patch_path_mut(root, path)?;
    let list = target
        .as_array_mut()
        .ok_or_else(|| "appendMember requires a list".to_owned())?;
    if list
        .iter()
        .any(|entry| patch_member_matches(entry, identity.key, &identity.value))
    {
        return Err(format!(
            "member {}={} already exists",
            patch_member_key(identity.key),
            identity.value
        ));
    }
    if value.values().any(|value| {
        !value.is_null() && !value.is_string() && !value.is_number() && !value.is_boolean()
    }) {
        return Err("appendMember values must be scalar".to_owned());
    }
    let mut member = value
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    member.insert(
        patch_member_key(identity.key).to_owned(),
        Value::String(identity.value.clone()),
    );
    let member = Value::Object(member);
    let index = match position {
        ContentPatchPosition::Start => 0,
        ContentPatchPosition::End => list.len(),
        ContentPatchPosition::Before { anchor } | ContentPatchPosition::After { anchor } => {
            let matches = list
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    patch_member_matches(entry, anchor.key, &anchor.value).then_some(index)
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(format!(
                    "anchor {}={} resolved {} entries",
                    patch_member_key(anchor.key),
                    anchor.value,
                    matches.len()
                ));
            }
            matches[0] + usize::from(matches!(position, ContentPatchPosition::After { .. }))
        }
    };
    list.insert(index, member);
    Ok(())
}

fn remove_patch_member(
    root: &mut Value,
    path: &[ContentPatchPathSegment],
    identity: &ContentPatchMemberSelector,
) -> Result<(), String> {
    let target = read_patch_path_mut(root, path)?;
    let list = target
        .as_array_mut()
        .ok_or_else(|| "removeMember requires a list".to_owned())?;
    let indexes = list
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            patch_member_matches(entry, identity.key, &identity.value).then_some(index)
        })
        .collect::<Vec<_>>();
    if indexes.len() != 1 {
        return Err(format!(
            "member {}={} resolved {} entries",
            patch_member_key(identity.key),
            identity.value,
            indexes.len()
        ));
    }
    list.remove(indexes[0]);
    Ok(())
}

fn patch_member_key(key: ContentPatchMemberKey) -> &'static str {
    match key {
        ContentPatchMemberKey::Id => "id",
        ContentPatchMemberKey::ResourceId => "resourceId",
        ContentPatchMemberKey::StatId => "statId",
        ContentPatchMemberKey::DefenseId => "defenseId",
        ContentPatchMemberKey::ModifierId => "modifierId",
        ContentPatchMemberKey::DamageType => "damageType",
        ContentPatchMemberKey::Kind => "kind",
    }
}

fn patch_change_path(path: &[ContentPatchPathSegment]) -> String {
    path.iter()
        .map(|segment| match segment {
            ContentPatchPathSegment::Field { name } => name.clone(),
            ContentPatchPathSegment::Member { key, value } => {
                format!("[{}={value}]", patch_member_key(*key))
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn validate_patch_changes(
    changes: &[rpg_ir::ContentPatchChangeProvenance],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for (index, change) in changes.iter().enumerate() {
        if change.path_segments.is_empty() {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_PATCH_CHANGE_PATH_SEGMENTS_MISSING",
                format!("{path}[{index}].pathSegments"),
                "patch changes must carry a typed path for authoritative reconstruction",
            ));
        }
        let rendered_path = patch_change_path(&change.path_segments);
        if rendered_path != change.path {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_PATCH_CHANGE_PATH_MISMATCH",
                format!("{path}[{index}].path"),
                "the display path must match the canonical typed path",
            ));
        }
        if change.plane == ContentImpactPlane::Both {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_PATCH_CHANGE_PLANE_INVALID",
                format!("{path}[{index}].plane"),
                "an individual patch change must name semantic or presentation",
            ));
        }
        let effective = change.before != change.after;
        if effective != change.effective {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "CONTENT_PACK_PATCH_CHANGE_EFFECT_MISMATCH",
                format!("{path}[{index}].effective"),
                "patch change effectiveness must match canonical before/after values",
            ));
        }
    }
}

pub fn materialized_definition_fingerprint(
    definition: &MaterializedContentDefinition,
) -> Result<String, RpgCompileFailure> {
    fingerprint(&json!({
        "id": definition.id,
        "kind": definition.kind,
        "visibility": definition.visibility,
        "extensionPolicy": definition.extension_policy,
        "semantic": definition.semantic,
        "presentation": definition.presentation,
        "references": definition.references,
        "provenance": definition.provenance,
    }))
}

fn fingerprints(
    prepared: &PreparedPlayBundle,
) -> Result<PlayBundleFingerprints, RpgCompileFailure> {
    let source = fingerprint(&(
        &prepared.play_bundle_identity,
        &prepared.ruleset.identity,
        &prepared.content_packs,
        &prepared.dependency_lock,
        &prepared.definition_provenance,
        &prepared.definition_commitments,
        &prepared.relationships,
        &prepared.derivation_provenance,
        &prepared.overlay_provenance,
        prepared
            .materialized_definitions
            .iter()
            .map(|definition| {
                json!({
                    "id": definition.id,
                    "kind": definition.kind,
                    "visibility": definition.visibility,
                    "extensionPolicy": definition.extension_policy,
                    "references": definition.references,
                    "actionSourcePath": action_semantic_field(definition, "sourcePath"),
                })
            })
            .collect::<Vec<_>>(),
    ))?;

    let semantic = fingerprint(&json!({
        "ruleset": prepared.ruleset,
        "definitions": prepared.materialized_definitions.iter().map(|definition| json!({
            "id": definition.id,
            "kind": definition.kind,
            "visibility": definition.visibility,
            "extensionPolicy": definition.extension_policy,
            "semantic": semantic_definition_value(definition),
            "references": definition.references,
        })).collect::<Vec<_>>(),
        "contentRequirements": prepared.content_requirements,
        "policyBindings": prepared.compiled_policy_bindings.iter().map(|binding| json!({
            "id": binding.id,
            "policyId": binding.policy_id,
            "policyVersion": binding.policy_version,
            "viewKind": binding.view_kind,
            "viewVersion": binding.view_version,
            "intentKinds": binding.intent_kinds,
            "decisionMoments": binding.decision_moments,
        })).collect::<Vec<_>>(),
    }))?;
    let presentation = fingerprint(&json!({
        "definitions": prepared.materialized_definitions.iter().map(|definition| json!({
            "id": definition.id,
            "presentation": definition.presentation,
            "actionName": action_semantic_field(definition, "name"),
        })).collect::<Vec<_>>(),
        "policyLabels": prepared.compiled_policy_bindings.iter().map(|binding| json!({
            "id": binding.id,
            "label": binding.label,
        })).collect::<Vec<_>>(),
    }))?;
    Ok(PlayBundleFingerprints {
        source,
        semantic,
        presentation,
    })
}

fn semantic_definition_value(definition: &MaterializedContentDefinition) -> Value {
    let mut semantic = definition.semantic.clone();
    if definition.kind == MaterializedContentDefinitionKind::Action {
        if let Some(object) = semantic.get_mut("action").and_then(Value::as_object_mut) {
            object.remove("name");
            object.remove("sourcePath");
        }
    }
    semantic
}

fn action_semantic_field(definition: &MaterializedContentDefinition, field: &str) -> Value {
    if definition.kind != MaterializedContentDefinitionKind::Action {
        return Value::Null;
    }
    definition
        .semantic
        .get("action")
        .and_then(|action| action.get(field))
        .cloned()
        .unwrap_or(Value::Null)
}

fn fingerprint(value: &impl Serialize) -> Result<String, RpgCompileFailure> {
    let bytes = serde_json::to_vec(value).map_err(fingerprint_error)?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

fn fingerprint_error(error: serde_json::Error) -> RpgCompileFailure {
    RpgCompileFailure {
        diagnostics: vec![RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_FINGERPRINT_ENCODING_FAILED",
            "$",
            error.to_string(),
        )],
    }
}

fn validate_identity(id: &str, version: &str, path: &str, diagnostics: &mut Vec<RpgDiagnostic>) {
    if !valid_identifier(id) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_IDENTITY_INVALID",
            format!("{path}.id"),
            format!("invalid ruleset identity {id}"),
        ));
    }
    if !exact_version(version) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "CONTENT_PACK_VERSION_INVALID",
            format!("{path}.version"),
            format!("version {version} is not exact semver"),
        ));
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    if !characters
        .next()
        .is_some_and(|value| value.is_ascii_lowercase())
    {
        return false;
    }
    characters.all(|value| {
        value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, '.' | '_' | '-')
    })
}

fn exact_version(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|value| value.is_ascii_digit())
                && (segment == &"0" || !segment.starts_with('0'))
        })
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("fnv1a64:").is_some_and(|hash| {
        hash.len() == 16
            && hash
                .chars()
                .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
    })
}
