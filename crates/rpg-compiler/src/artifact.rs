use std::collections::{BTreeMap, BTreeSet};

use rpg_ir::{
    CompiledRulesetArtifact, MaterializedRulesetDefinition, MaterializedRulesetDefinitionKind,
    MaterializedRulesetVisibility, PreparedRulesetCompilation, RulesetArtifactFingerprints,
    RulesetArtifactSchema, RulesetRelationshipKind, VersionedRulesetRequirement,
    COMPILED_RULESET_IDENTITY, PREPARED_RULESET_IDENTITY, RPG_IR_IDENTITY, RULESET_ARTIFACT_MAJOR,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    compile_normalized_rpg_ir, CompiledRpgRuleset, RpgCompileFailure, RpgDiagnostic,
    RpgDiagnosticStage,
};

#[derive(Debug, Clone)]
pub struct CompiledRulesetBundle {
    artifact: CompiledRulesetArtifact,
    ruleset: CompiledRpgRuleset,
}

impl CompiledRulesetBundle {
    pub fn artifact(&self) -> &CompiledRulesetArtifact {
        &self.artifact
    }

    pub fn ruleset(&self) -> &CompiledRpgRuleset {
        &self.ruleset
    }

    pub fn into_artifact(self) -> CompiledRulesetArtifact {
        self.artifact
    }
}

pub fn compile_prepared_ruleset_json(
    source: &[u8],
) -> Result<CompiledRulesetBundle, RpgCompileFailure> {
    let prepared =
        serde_json::from_slice::<PreparedRulesetCompilation>(source).map_err(|error| {
            RpgCompileFailure {
                diagnostics: vec![RpgDiagnostic::error(
                    RpgDiagnosticStage::Decode,
                    "RULESET_PREPARED_DECODE_FAILED",
                    "$",
                    error.to_string(),
                )],
            }
        })?;
    compile_prepared_ruleset(prepared)
}

pub fn compile_prepared_ruleset(
    prepared: PreparedRulesetCompilation,
) -> Result<CompiledRulesetBundle, RpgCompileFailure> {
    let diagnostics = validate_prepared(&prepared);
    if !diagnostics.is_empty() {
        return Err(RpgCompileFailure { diagnostics });
    }

    let ruleset = compile_normalized_rpg_ir(prepared.normalized_ir.clone())?;
    let fingerprints = fingerprints(&prepared)?;
    let artifact_id = fingerprint(&(
        &prepared.composition_identity,
        &fingerprints.source,
        &fingerprints.semantic,
        &fingerprints.presentation,
    ))?;
    let artifact = CompiledRulesetArtifact {
        artifact_schema: RulesetArtifactSchema {
            identity: COMPILED_RULESET_IDENTITY.to_owned(),
            major: RULESET_ARTIFACT_MAJOR,
        },
        artifact_id: format!(
            "{}@{}:{artifact_id}",
            prepared.composition_identity.id, prepared.composition_identity.version
        ),
        composition_identity: prepared.composition_identity,
        language_identity: prepared.language_identity,
        source_packages: prepared.source_packages,
        dependency_lock: prepared.dependency_lock,
        required_operations: prepared.required_operations,
        required_capabilities: prepared.required_capabilities,
        exported_roots: prepared.exported_roots,
        materialized_definitions: prepared.materialized_definitions,
        compiled_policy_bindings: prepared.compiled_policy_bindings,
        definition_provenance: prepared.definition_provenance,
        relationships: prepared.relationships,
        derivation_provenance: prepared.derivation_provenance,
        overlay_provenance: prepared.overlay_provenance,
        normalized_ir: prepared.normalized_ir,
        fingerprints,
    };
    Ok(CompiledRulesetBundle { artifact, ruleset })
}

pub fn load_compiled_ruleset_artifact_json(
    source: &[u8],
) -> Result<CompiledRulesetBundle, RpgCompileFailure> {
    let artifact = serde_json::from_slice::<CompiledRulesetArtifact>(source).map_err(|error| {
        RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Decode,
                "RULESET_ARTIFACT_DECODE_FAILED",
                "$",
                error.to_string(),
            )],
        }
    })?;
    load_compiled_ruleset_artifact(artifact)
}

pub fn load_compiled_ruleset_artifact(
    artifact: CompiledRulesetArtifact,
) -> Result<CompiledRulesetBundle, RpgCompileFailure> {
    if artifact.artifact_schema.identity != COMPILED_RULESET_IDENTITY
        || artifact.artifact_schema.major != RULESET_ARTIFACT_MAJOR
    {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_ARTIFACT_SCHEMA_UNSUPPORTED",
                "$.artifactSchema",
                format!("expected {COMPILED_RULESET_IDENTITY}@{RULESET_ARTIFACT_MAJOR}"),
            )],
        });
    }

    let prepared = prepared_from_artifact(&artifact);
    let recompiled = compile_prepared_ruleset(prepared)?;
    if recompiled.artifact != artifact {
        return Err(RpgCompileFailure {
            diagnostics: vec![RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_ARTIFACT_FINGERPRINT_MISMATCH",
                "$.fingerprints",
                "artifact identity or fingerprint planes do not match its closed contents",
            )],
        });
    }
    Ok(recompiled)
}

fn prepared_from_artifact(artifact: &CompiledRulesetArtifact) -> PreparedRulesetCompilation {
    PreparedRulesetCompilation {
        schema: RulesetArtifactSchema {
            identity: PREPARED_RULESET_IDENTITY.to_owned(),
            major: RULESET_ARTIFACT_MAJOR,
        },
        composition_identity: artifact.composition_identity.clone(),
        language_identity: artifact.language_identity.clone(),
        source_packages: artifact.source_packages.clone(),
        dependency_lock: artifact.dependency_lock.clone(),
        required_operations: artifact.required_operations.clone(),
        required_capabilities: artifact.required_capabilities.clone(),
        exported_roots: artifact.exported_roots.clone(),
        materialized_definitions: artifact.materialized_definitions.clone(),
        compiled_policy_bindings: artifact.compiled_policy_bindings.clone(),
        definition_provenance: artifact.definition_provenance.clone(),
        relationships: artifact.relationships.clone(),
        derivation_provenance: artifact.derivation_provenance.clone(),
        overlay_provenance: artifact.overlay_provenance.clone(),
        normalized_ir: artifact.normalized_ir.clone(),
    }
}

fn validate_prepared(prepared: &PreparedRulesetCompilation) -> Vec<RpgDiagnostic> {
    let mut diagnostics = Vec::new();
    if prepared.schema.identity != PREPARED_RULESET_IDENTITY
        || prepared.schema.major != RULESET_ARTIFACT_MAJOR
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "RULESET_PREPARED_SCHEMA_UNSUPPORTED",
            "$.schema",
            format!("expected {PREPARED_RULESET_IDENTITY}@{RULESET_ARTIFACT_MAJOR}"),
        ));
    }
    validate_identity(
        &prepared.composition_identity.id,
        &prepared.composition_identity.version,
        "$.compositionIdentity",
        &mut diagnostics,
    );
    if prepared.language_identity.id != "asha-rpg" || prepared.language_identity.version != "1.0.0"
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Compatibility,
            "RULESET_LANGUAGE_UNSUPPORTED",
            "$.languageIdentity",
            "supported language is asha-rpg@1.0.0",
        ));
    }
    if prepared.normalized_ir.schema.identity != RPG_IR_IDENTITY
        || prepared.normalized_ir.package.id != prepared.composition_identity.id
        || prepared.normalized_ir.package.version != prepared.composition_identity.version
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_NORMALIZED_IDENTITY_MISMATCH",
            "$.normalizedIr.package",
            "normalized IR must carry the resolved composition identity",
        ));
    }

    validate_sources_and_lock(prepared, &mut diagnostics);
    validate_requirements(prepared, &mut diagnostics);
    validate_definitions(prepared, &mut diagnostics);
    validate_deferred_relationships(prepared, &mut diagnostics);
    diagnostics
}

fn validate_sources_and_lock(
    prepared: &PreparedRulesetCompilation,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let mut sources = BTreeMap::new();
    let mut previous = None::<(&str, &str)>;
    for (index, source) in prepared.source_packages.iter().enumerate() {
        validate_identity(
            &source.id,
            &source.version,
            &format!("$.sourcePackages[{index}]"),
            diagnostics,
        );
        if !valid_fingerprint(&source.source_fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_SOURCE_FINGERPRINT_INVALID",
                format!("$.sourcePackages[{index}].sourceFingerprint"),
                "source fingerprint must be fnv1a64 with sixteen lowercase hex digits",
            ));
        }
        if let Some(previous_identity) = previous {
            if previous_identity >= (source.id.as_str(), source.version.as_str()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "RULESET_SOURCE_PACKAGES_NOT_CANONICAL",
                    format!("$.sourcePackages[{index}]"),
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
                "RULESET_DUPLICATE_SOURCE_PACKAGE",
                format!("$.sourcePackages[{index}]"),
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
                "RULESET_LOCK_VERSION_NOT_EXACT",
                format!("$.dependencyLock[{index}].resolvedVersion"),
                "resolved dependency versions must be exact semver",
            ));
        }
        let source_identity = format!("{}@{}", entry.package_id, entry.resolved_version);
        if sources.get(&source_identity).copied() != Some(&entry.source_fingerprint) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_LOCK_SOURCE_MISMATCH",
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
                    "RULESET_LOCK_NOT_CANONICAL",
                    format!("$.dependencyLock[{index}]"),
                    "dependency lock entries must be strictly sorted",
                ));
            }
        }
        previous_lock = Some(identity.clone());
        if !lock_identities.insert(identity) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_DUPLICATE_LOCK_ENTRY",
                format!("$.dependencyLock[{index}]"),
                "duplicate dependency lock entry",
            ));
        }
    }
}

fn validate_requirements(
    prepared: &PreparedRulesetCompilation,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    validate_sorted_requirements(
        &prepared.required_operations,
        "$.requiredOperations",
        diagnostics,
    );
    validate_sorted_requirements(
        &prepared.required_capabilities,
        "$.requiredCapabilities",
        diagnostics,
    );

    let operations = prepared
        .required_operations
        .iter()
        .map(|entry| (entry.id.as_str(), entry.version))
        .collect::<BTreeSet<_>>();
    let capabilities = prepared
        .required_capabilities
        .iter()
        .map(|entry| (entry.id.as_str(), entry.version))
        .collect::<BTreeSet<_>>();
    for (index, requirement) in prepared.normalized_ir.requirements.iter().enumerate() {
        let present = match requirement.kind {
            rpg_ir::RpgIrRequirementKind::Operation => {
                operations.contains(&(requirement.id.as_str(), requirement.version))
            }
            rpg_ir::RpgIrRequirementKind::Capability => {
                capabilities.contains(&(requirement.id.as_str(), requirement.version))
            }
        };
        if !present {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Requirements,
                "RULESET_NORMALIZED_REQUIREMENT_UNDECLARED",
                format!("$.normalizedIr.requirements[{index}]"),
                format!(
                    "normalized requirement {}@{} is absent from the closed artifact requirements",
                    requirement.id, requirement.version
                ),
            ));
        }
    }
}

fn validate_sorted_requirements(
    requirements: &[VersionedRulesetRequirement],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let mut previous = None::<(&str, u32)>;
    for (index, requirement) in requirements.iter().enumerate() {
        let identity = (requirement.id.as_str(), requirement.version);
        if previous.is_some_and(|value| value >= identity) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_REQUIREMENTS_NOT_CANONICAL",
                format!("{path}[{index}]"),
                "requirements must be strictly identity-sorted",
            ));
        }
        previous = Some(identity);
    }
}

fn validate_definitions(
    prepared: &PreparedRulesetCompilation,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let mut definitions = BTreeMap::<&str, &MaterializedRulesetDefinition>::new();
    let mut previous = None::<&str>;
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if previous.is_some_and(|value| value >= definition.id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DEFINITIONS_NOT_CANONICAL",
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
                "RULESET_DUPLICATE_MATERIALIZED_DEFINITION",
                format!("$.materializedDefinitions[{index}].id"),
                format!("duplicate definition {}", definition.id),
            ));
        }
        if definition.provenance.definition_id != definition.id {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DEFINITION_PROVENANCE_MISMATCH",
                format!("$.materializedDefinitions[{index}].provenance"),
                "definition provenance must name its materialized definition",
            ));
        }
    }
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        for reference in &definition.references {
            if !definitions.contains_key(reference.as_str()) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::References,
                    "RULESET_ARTIFACT_REFERENCE_MISSING",
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
                "RULESET_EXPORTED_ROOTS_NOT_CANONICAL",
                format!("$.exportedRoots[{index}]"),
                "exported roots must be strictly identity-sorted",
            ));
        }
        previous_root = Some(root);
        if !definitions.contains_key(root.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_EXPORTED_ROOT_MISSING",
                format!("$.exportedRoots[{index}]"),
                format!("exported root {root} is not materialized"),
            ));
        } else if definitions[root.as_str()].visibility != MaterializedRulesetVisibility::Exported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_EXPORTED_ROOT_VISIBILITY_MISMATCH",
                format!("$.exportedRoots[{index}]"),
                format!("exported root {root} must have exported visibility"),
            ));
        }
    }
    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        let is_root = roots.contains(definition.id.as_str());
        let is_exported = definition.visibility == MaterializedRulesetVisibility::Exported;
        if is_root != is_exported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DEFINITION_VISIBILITY_MISMATCH",
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
            "RULESET_DEFINITION_PROVENANCE_NOT_CANONICAL",
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
                "RULESET_MATERIALIZED_DEFINITION_UNREACHABLE",
                format!("$.materializedDefinitions[{index}]"),
                format!(
                    "materialized definition {} is not reachable from an exported root",
                    definition.id
                ),
            ));
        }
    }

    let action_definitions = prepared
        .materialized_definitions
        .iter()
        .filter(|definition| definition.kind == MaterializedRulesetDefinitionKind::Action)
        .map(|definition| definition.id.as_str())
        .collect::<BTreeSet<_>>();
    let normalized_actions = prepared
        .normalized_ir
        .actions
        .iter()
        .map(|action| action.id.as_str())
        .collect::<BTreeSet<_>>();
    if action_definitions != normalized_actions {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_ACTION_MATERIALIZATION_MISMATCH",
            "$.normalizedIr.actions",
            "normalized actions must exactly match materialized action definitions",
        ));
    }
}

fn visit_materialized_definition<'a>(
    definition_id: &str,
    definitions: &BTreeMap<&'a str, &'a MaterializedRulesetDefinition>,
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
            "RULESET_ARTIFACT_DEFINITION_CYCLE",
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

fn validate_deferred_relationships(
    prepared: &PreparedRulesetCompilation,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    if !prepared.derivation_provenance.is_empty() {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_DERIVATION_EXECUTION_DEFERRED",
            "$.derivationProvenance",
            "derivation provenance cannot enter an artifact until its owned materializer exists",
        ));
    }
    if !prepared.overlay_provenance.is_empty() {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_OVERLAY_EXECUTION_DEFERRED",
            "$.overlayProvenance",
            "overlay provenance cannot enter an artifact until its owned materializer exists",
        ));
    }
    for (index, relationship) in prepared.relationships.iter().enumerate() {
        if matches!(
            relationship.kind,
            RulesetRelationshipKind::DerivesFrom
                | RulesetRelationshipKind::Patches
                | RulesetRelationshipKind::Configures
        ) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_RELATIONSHIP_EXECUTION_DEFERRED",
                format!("$.relationships[{index}]"),
                "derivation, patch, and configuration relationships require an owned materializer",
            ));
        }
    }
}

fn fingerprints(
    prepared: &PreparedRulesetCompilation,
) -> Result<RulesetArtifactFingerprints, RpgCompileFailure> {
    let source = fingerprint(&(
        &prepared.composition_identity,
        &prepared.source_packages,
        &prepared.dependency_lock,
        &prepared.definition_provenance,
        &prepared.relationships,
        &prepared.derivation_provenance,
        &prepared.overlay_provenance,
    ))?;

    let mut normalized =
        serde_json::to_value(&prepared.normalized_ir).map_err(fingerprint_error)?;
    if let Some(actions) = normalized.get_mut("actions").and_then(Value::as_array_mut) {
        for action in actions {
            if let Some(object) = action.as_object_mut() {
                object.remove("name");
                object.remove("sourcePath");
            }
        }
    }
    let semantic = fingerprint(&json!({
        "normalizedIr": normalized,
        "requiredOperations": prepared.required_operations,
        "requiredCapabilities": prepared.required_capabilities,
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
        })).collect::<Vec<_>>(),
        "policyLabels": prepared.compiled_policy_bindings.iter().map(|binding| json!({
            "id": binding.id,
            "label": binding.label,
        })).collect::<Vec<_>>(),
    }))?;
    Ok(RulesetArtifactFingerprints {
        source,
        semantic,
        presentation,
    })
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
            "RULESET_FINGERPRINT_ENCODING_FAILED",
            "$",
            error.to_string(),
        )],
    }
}

fn validate_identity(id: &str, version: &str, path: &str, diagnostics: &mut Vec<RpgDiagnostic>) {
    if !valid_identifier(id) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_IDENTITY_INVALID",
            format!("{path}.id"),
            format!("invalid ruleset identity {id}"),
        ));
    }
    if !exact_version(version) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_VERSION_INVALID",
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
