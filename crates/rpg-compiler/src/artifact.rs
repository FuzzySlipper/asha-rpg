use std::collections::{BTreeMap, BTreeSet};

use rpg_ir::{
    CompiledRulesetArtifact, MaterializedRulesetDefinition, MaterializedRulesetDefinitionKind,
    MaterializedRulesetVisibility, NormalizedRpgIr, PreparedRulesetCompilation, RpgIrAction,
    RpgIrCatalogs, RpgIrCheck, RpgIrFormula, RpgIrOperation, RpgIrPackage, RpgIrPredicate,
    RpgIrProgram, RpgIrRequirement, RpgIrRequirementKind, RpgIrSchema, RulesetArtifactFingerprints,
    RulesetArtifactSchema, RulesetImpactPlane, RulesetMaterializationStage,
    RulesetMaterializationValue, RulesetPatch, RulesetPatchChangeProvenance, RulesetPatchMemberKey,
    RulesetPatchMemberSelector, RulesetPatchOperation, RulesetPatchPathSegment,
    RulesetPatchPosition, RulesetRelationshipKind, VersionedRulesetRequirement,
    COMPILED_RULESET_IDENTITY, PREPARED_RULESET_IDENTITY, RPG_IR_IDENTITY, RPG_IR_MAJOR,
    RULESET_ARTIFACT_MAJOR,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    capability_registrations, compile_normalized_rpg_ir, operation_registrations,
    CompiledRpgRuleset, RpgCompileFailure, RpgDiagnostic, RpgDiagnosticStage,
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

    let normalized_ir = normalized_ir_from_materialized(&prepared)?;
    let ruleset = compile_normalized_rpg_ir(normalized_ir)?;
    let fingerprints = fingerprints(&prepared)?;
    let artifact_schema = RulesetArtifactSchema {
        identity: COMPILED_RULESET_IDENTITY.to_owned(),
        major: RULESET_ARTIFACT_MAJOR,
    };
    let artifact_id = fingerprint(&json!({
        "artifactSchema": &artifact_schema,
        "compositionIdentity": &prepared.composition_identity,
        "languageIdentity": &prepared.language_identity,
        "sourcePackages": &prepared.source_packages,
        "dependencyLock": &prepared.dependency_lock,
        "requiredOperations": &prepared.required_operations,
        "requiredCapabilities": &prepared.required_capabilities,
        "exportedRoots": &prepared.exported_roots,
        "materializedDefinitions": &prepared.materialized_definitions,
        "compiledPolicyBindings": &prepared.compiled_policy_bindings,
        "definitionProvenance": &prepared.definition_provenance,
        "relationships": &prepared.relationships,
        "derivationProvenance": &prepared.derivation_provenance,
        "overlayProvenance": &prepared.overlay_provenance,
        "fingerprints": &fingerprints,
    }))?;
    let artifact = CompiledRulesetArtifact {
        artifact_schema,
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
    validate_sources_and_lock(prepared, &mut diagnostics);
    validate_requirements(prepared, &mut diagnostics);
    validate_definitions(prepared, &mut diagnostics);
    validate_materialization_provenance(prepared, &mut diagnostics);
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

    for (index, requirement) in prepared.required_operations.iter().enumerate() {
        let supported = operation_registrations().iter().any(|registration| {
            registration.id == requirement.id && registration.version == requirement.version
        });
        if !supported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_OPERATION_REQUIREMENT_UNSUPPORTED",
                format!("$.requiredOperations[{index}]"),
                format!(
                    "operation {}@{} is not registered by Rust authority",
                    requirement.id, requirement.version
                ),
            ));
        }
    }
    for (index, requirement) in prepared.required_capabilities.iter().enumerate() {
        let supported = capability_registrations().iter().any(|registration| {
            registration.id.as_str() == requirement.id
                && registration.version == requirement.version
        });
        if !supported {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Compatibility,
                "RULESET_CAPABILITY_REQUIREMENT_UNSUPPORTED",
                format!("$.requiredCapabilities[{index}]"),
                format!(
                    "capability {}@{} is not registered by Rust authority",
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
        match materialized_definition_fingerprint(definition) {
            Ok(expected) if expected != definition.fingerprint => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "RULESET_DEFINITION_FINGERPRINT_MISMATCH",
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

#[derive(Default)]
struct DerivedCatalogs {
    stats: BTreeSet<String>,
    defenses: BTreeSet<String>,
    resources: BTreeSet<String>,
    modifiers: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDefinitionSemantic {
    catalog: String,
    id: String,
}

fn normalized_ir_from_materialized(
    prepared: &PreparedRulesetCompilation,
) -> Result<NormalizedRpgIr, RpgCompileFailure> {
    let definitions = prepared
        .materialized_definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    let mut catalogs = DerivedCatalogs::default();
    let mut actions = Vec::new();

    for (index, definition) in prepared.materialized_definitions.iter().enumerate() {
        if definition.kind != MaterializedRulesetDefinitionKind::Action {
            continue;
        }
        let path = format!("$.materializedDefinitions[{index}].semantic");
        let mut action = match serde_json::from_value::<RpgIrAction>(definition.semantic.clone()) {
            Ok(action) => action,
            Err(error) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    "RULESET_ACTION_SEMANTIC_DECODE_FAILED",
                    &path,
                    error.to_string(),
                ));
                continue;
            }
        };
        if action.id != definition.id {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_ACTION_SEMANTIC_ID_MISMATCH",
                format!("{path}.id"),
                format!(
                    "materialized action {} carries semantic identity {}",
                    definition.id, action.id
                ),
            ));
        }
        resolve_action_catalogs(
            &mut action,
            definition,
            &definitions,
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
        .required_operations
        .iter()
        .map(|requirement| RpgIrRequirement {
            kind: RpgIrRequirementKind::Operation,
            id: requirement.id.clone(),
            version: requirement.version,
        })
        .chain(
            prepared
                .required_capabilities
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
            id: prepared.composition_identity.id.clone(),
            version: prepared.composition_identity.version.clone(),
        },
        catalogs: RpgIrCatalogs {
            stats: catalogs.stats.into_iter().collect(),
            defenses: catalogs.defenses.into_iter().collect(),
            resources: catalogs.resources.into_iter().collect(),
            modifiers: catalogs.modifiers.into_iter().collect(),
            capabilities: prepared
                .required_capabilities
                .iter()
                .map(|requirement| requirement.id.clone())
                .collect(),
        },
        requirements,
        actions,
    })
}

fn resolve_action_catalogs(
    action: &mut RpgIrAction,
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for (index, cost) in action.costs.iter_mut().enumerate() {
        resolve_catalog_reference(
            &mut cost.resource_id,
            "resource",
            "RESOURCE",
            action_definition,
            definitions,
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
                "defense",
                "DEFENSE",
                action_definition,
                definitions,
                &format!("{path}.check.defenseId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                modifier,
                action_definition,
                definitions,
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
                "defense",
                "DEFENSE",
                action_definition,
                definitions,
                &format!("{path}.check.defenseId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                difficulty,
                action_definition,
                definitions,
                &format!("{path}.check.difficulty"),
                diagnostics,
            );
        }
    }
    resolve_program_catalogs(
        &mut action.program,
        action_definition,
        definitions,
        &format!("{path}.program"),
        diagnostics,
    );
}

fn resolve_program_catalogs(
    program: &mut RpgIrProgram,
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match program {
        RpgIrProgram::Operation { operation } => {
            resolve_operation_catalogs(
                operation,
                action_definition,
                definitions,
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
                &format!("{path}.predicate"),
                diagnostics,
            );
            resolve_program_catalogs(
                then,
                action_definition,
                definitions,
                &format!("{path}.then"),
                diagnostics,
            );
            if let Some(otherwise) = otherwise {
                resolve_program_catalogs(
                    otherwise,
                    action_definition,
                    definitions,
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
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
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
                "damageType",
                "DAMAGE_TYPE",
                action_definition,
                definitions,
                &format!("{path}.damageType"),
                diagnostics,
            );
            resolve_formula_catalogs(
                amount,
                action_definition,
                definitions,
                &format!("{path}.amount"),
                diagnostics,
            );
        }
        RpgIrOperation::Heal { amount } => resolve_formula_catalogs(
            amount,
            action_definition,
            definitions,
            &format!("{path}.amount"),
            diagnostics,
        ),
        RpgIrOperation::ChangeResource {
            resource_id, delta, ..
        } => {
            resolve_catalog_reference(
                resource_id,
                "resource",
                "RESOURCE",
                action_definition,
                definitions,
                &format!("{path}.resourceId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                delta,
                action_definition,
                definitions,
                &format!("{path}.delta"),
                diagnostics,
            );
        }
        RpgIrOperation::ApplyModifier {
            modifier_id, value, ..
        } => {
            resolve_catalog_reference(
                modifier_id,
                "modifier",
                "MODIFIER",
                action_definition,
                definitions,
                &format!("{path}.modifierId"),
                diagnostics,
            );
            resolve_formula_catalogs(
                value,
                action_definition,
                definitions,
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
                &format!("{path}.deltaX"),
                diagnostics,
            );
            resolve_formula_catalogs(
                delta_y,
                action_definition,
                definitions,
                &format!("{path}.deltaY"),
                diagnostics,
            );
        }
        RpgIrOperation::OpenReaction { .. } => {}
    }
}

fn resolve_formula_catalogs(
    formula: &mut RpgIrFormula,
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    match formula {
        RpgIrFormula::ReadStat { stat_id, .. } => resolve_catalog_reference(
            stat_id,
            "stat",
            "STAT",
            action_definition,
            definitions,
            &format!("{path}.statId"),
            diagnostics,
        ),
        RpgIrFormula::Add { terms } => {
            for (index, term) in terms.iter_mut().enumerate() {
                resolve_formula_catalogs(
                    term,
                    action_definition,
                    definitions,
                    &format!("{path}.terms[{index}]"),
                    diagnostics,
                );
            }
        }
        RpgIrFormula::Half { value } => resolve_formula_catalogs(
            value,
            action_definition,
            definitions,
            &format!("{path}.value"),
            diagnostics,
        ),
        RpgIrFormula::Constant { .. } | RpgIrFormula::Dice { .. } => {}
    }
}

fn resolve_predicate_catalogs(
    predicate: &mut RpgIrPredicate,
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
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
                &format!("{path}.left"),
                diagnostics,
            );
            resolve_formula_catalogs(
                right,
                action_definition,
                definitions,
                &format!("{path}.right"),
                diagnostics,
            );
        }
        RpgIrPredicate::Not { predicate } => resolve_predicate_catalogs(
            predicate,
            action_definition,
            definitions,
            &format!("{path}.predicate"),
            diagnostics,
        ),
        RpgIrPredicate::All { predicates } | RpgIrPredicate::Any { predicates } => {
            for (index, predicate) in predicates.iter_mut().enumerate() {
                resolve_predicate_catalogs(
                    predicate,
                    action_definition,
                    definitions,
                    &format!("{path}.predicates[{index}]"),
                    diagnostics,
                );
            }
        }
    }
}

fn resolve_catalog_reference(
    value: &mut String,
    expected_catalog: &str,
    diagnostic_kind: &str,
    action_definition: &MaterializedRulesetDefinition,
    definitions: &BTreeMap<&str, &MaterializedRulesetDefinition>,
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    if !action_definition
        .references
        .iter()
        .any(|reference| reference == value)
    {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(diagnostic_kind, CatalogDiagnostic::ReferenceUndeclared),
            path,
            format!(
                "{expected_catalog} {value} must be a direct definition reference from {}",
                action_definition.id
            ),
        ));
        return;
    }
    let Some(definition) = definitions.get(value.as_str()) else {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(diagnostic_kind, CatalogDiagnostic::DefinitionMissing),
            path,
            format!("{expected_catalog} definition {value} is absent"),
        ));
        return;
    };
    if definition.kind != MaterializedRulesetDefinitionKind::Support {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(diagnostic_kind, CatalogDiagnostic::DefinitionKindInvalid),
            path,
            format!("{expected_catalog} definition {value} must be support data"),
        ));
        return;
    }
    let semantic =
        match serde_json::from_value::<CatalogDefinitionSemantic>(definition.semantic.clone()) {
            Ok(semantic) => semantic,
            Err(error) => {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Semantics,
                    catalog_diagnostic_code(diagnostic_kind, CatalogDiagnostic::SemanticInvalid),
                    path,
                    error.to_string(),
                ));
                return;
            }
        };
    if semantic.catalog != expected_catalog {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::References,
            catalog_diagnostic_code(diagnostic_kind, CatalogDiagnostic::CatalogMismatch),
            path,
            format!(
                "definition {} belongs to catalog {}, not {expected_catalog}",
                definition.id, semantic.catalog,
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
            "RULESET_DAMAGE_TYPE_REFERENCE_UNDECLARED"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::DefinitionMissing) => {
            "RULESET_DAMAGE_TYPE_DEFINITION_MISSING"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::DefinitionKindInvalid) => {
            "RULESET_DAMAGE_TYPE_DEFINITION_KIND_INVALID"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::SemanticInvalid) => {
            "RULESET_DAMAGE_TYPE_SEMANTIC_INVALID"
        }
        ("DAMAGE_TYPE", CatalogDiagnostic::CatalogMismatch) => {
            "RULESET_DAMAGE_TYPE_CATALOG_MISMATCH"
        }
        (_, CatalogDiagnostic::ReferenceUndeclared) => "RULESET_CATALOG_REFERENCE_UNDECLARED",
        (_, CatalogDiagnostic::DefinitionMissing) => "RULESET_CATALOG_DEFINITION_MISSING",
        (_, CatalogDiagnostic::DefinitionKindInvalid) => "RULESET_CATALOG_DEFINITION_KIND_INVALID",
        (_, CatalogDiagnostic::SemanticInvalid) => "RULESET_CATALOG_SEMANTIC_INVALID",
        (_, CatalogDiagnostic::CatalogMismatch) => "RULESET_CATALOG_MISMATCH",
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
    prepared: &PreparedRulesetCompilation,
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
                "RULESET_DERIVATION_PROVENANCE_NOT_CANONICAL",
                format!("$.derivationProvenance[{index}]"),
                "derivation provenance must be strictly definition-sorted",
            ));
        }
        previous_derivation = Some(&provenance.definition_id);
        if !definitions.contains_key(provenance.definition_id.as_str()) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::References,
                "RULESET_DERIVATION_TARGET_MISSING",
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
                    "RULESET_DERIVATION_FINGERPRINT_INVALID",
                    format!("$.derivationProvenance[{index}].{field}"),
                    "derivation fingerprints must be fnv1a64 with sixteen lowercase hex digits",
                ));
            }
        }
        for (mixin_index, mixin) in provenance.mixins.iter().enumerate() {
            if mixin.order != mixin_index || !valid_fingerprint(&mixin.fingerprint) {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "RULESET_DERIVATION_MIXIN_NOT_CANONICAL",
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
                    "RULESET_DERIVATION_MIXIN_PARAMETER_INVALID",
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
        if let Some(stage) = validate_derivation_semantics(provenance, index, diagnostics) {
            validated_derivation_stages.insert(provenance.definition_id.as_str(), stage);
        }
    }

    let mut overlays_by_definition = BTreeMap::<&str, Vec<_>>::new();
    let mut previous_overlay_order = None::<usize>;
    for (index, provenance) in prepared.overlay_provenance.iter().enumerate() {
        if previous_overlay_order.is_some_and(|previous| previous >= provenance.order) {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_OVERLAY_PROVENANCE_NOT_CANONICAL",
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
                "RULESET_OVERLAY_TARGET_MISSING",
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
                    "RULESET_OVERLAY_FINGERPRINT_INVALID",
                    format!("$.overlayProvenance[{index}].{field}"),
                    "overlay fingerprints must be fnv1a64 with sixteen lowercase hex digits",
                ));
            }
        }
        if provenance.expected_fingerprint != provenance.before_fingerprint {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_OVERLAY_EXPECTED_FINGERPRINT_MISMATCH",
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
        if let Some(entries) = overlays_by_definition.get(definition_id) {
            validate_overlay_fingerprint_chain(definition, derivation_stage, entries, diagnostics);
        } else if let Some(stage) = derivation_stage {
            let final_stage = materialization_stage(definition);
            if stage != final_stage {
                diagnostics.push(RpgDiagnostic::error(
                    RpgDiagnosticStage::Artifact,
                    "RULESET_DERIVATION_MATERIALIZED_STAGE_MISMATCH",
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
        .filter(|relationship| matches!(relationship.kind, RulesetRelationshipKind::DerivesFrom))
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
            "RULESET_DERIVATION_PROVENANCE_COVERAGE_MISMATCH",
            "$.relationships",
            "each derivation base and mixin relationship requires one matching provenance record",
        ));
    }

    let mut declared_overlays = BTreeMap::new();
    for relationship in prepared.relationships.iter().filter(|relationship| {
        matches!(relationship.kind, RulesetRelationshipKind::Patches)
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
            "RULESET_OVERLAY_PROVENANCE_COVERAGE_MISMATCH",
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
    definition: &MaterializedRulesetDefinition,
) -> RulesetMaterializationStage {
    RulesetMaterializationStage {
        id: definition.id.clone(),
        kind: definition.kind,
        extension_policy: definition.extension_policy,
        value: RulesetMaterializationValue {
            semantic: definition.semantic.clone(),
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
    provenance: &rpg_ir::RulesetDerivationProvenance,
    provenance_index: usize,
    diagnostics: &mut Vec<RpgDiagnostic>,
) -> Option<RulesetMaterializationStage> {
    let path = format!("$.derivationProvenance[{provenance_index}]");
    if provenance.base.id != provenance.base_definition_id {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_DERIVATION_BASE_STAGE_MISMATCH",
            format!("{path}.base.id"),
            "the base stage identity must match baseDefinitionId",
        ));
    }
    match canonical_fingerprint(&provenance.base) {
        Ok(actual) if actual != provenance.base_fingerprint => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DERIVATION_BASE_FINGERPRINT_MISMATCH",
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
        match canonical_fingerprint(&mixin.patch) {
            Ok(actual) if actual != mixin.fingerprint => diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DERIVATION_MIXIN_FINGERPRINT_MISMATCH",
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
                    "RULESET_DERIVATION_PATCH_REPLAY_FAILED",
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
                "RULESET_DERIVATION_LOCAL_PATCH_FINGERPRINT_MISMATCH",
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
                "RULESET_DERIVATION_PATCH_REPLAY_FAILED",
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
            "RULESET_DERIVATION_MATERIALIZED_STAGE_MISMATCH",
            format!("{path}.materialized"),
            "replaying the base, mixins, and local patch does not produce the claimed materialized stage",
        ));
    }
    if computed_changes != provenance.changes {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_DERIVATION_CHANGE_COVERAGE_MISMATCH",
            format!("{path}.changes"),
            "the submitted derivation changes do not exactly match authoritative patch replay",
        ));
    }
    match canonical_fingerprint(&provenance.materialized) {
        Ok(actual) if actual != provenance.materialized_fingerprint => {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DERIVATION_MATERIALIZED_FINGERPRINT_MISMATCH",
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
    stage: &mut RulesetMaterializationStage,
    materialized: &RulesetMaterializationStage,
) {
    stage.id.clone_from(&materialized.id);
    stage.kind = materialized.kind;
    stage.extension_policy = materialized.extension_policy;
    stage.references.clone_from(&materialized.references);
    if materialized.kind == MaterializedRulesetDefinitionKind::Action {
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
    final_definition: &MaterializedRulesetDefinition,
    derivation_stage: Option<RulesetMaterializationStage>,
    overlays: &[&rpg_ir::RulesetOverlayProvenance],
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    let Some(first) = overlays.first() else {
        return;
    };
    let mut stage = derivation_stage.unwrap_or_else(|| first.before.clone());
    for provenance in overlays {
        let path = format!("$.overlayProvenance[order={}]", provenance.order);
        if stage != provenance.before {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_OVERLAY_BEFORE_STAGE_MISMATCH",
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
                    "RULESET_OVERLAY_BEFORE_FINGERPRINT_MISMATCH",
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
                    "RULESET_OVERLAY_PATCH_FINGERPRINT_MISMATCH",
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
                "RULESET_OVERLAY_IMPACT_PLANE_MISMATCH",
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
                        "RULESET_OVERLAY_PATCH_REPLAY_FAILED",
                        format!("{path}.patch"),
                        message,
                    ));
                    return;
                }
            };
        if computed_changes != provenance.changes {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_OVERLAY_CHANGE_COVERAGE_MISMATCH",
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
                "RULESET_OVERLAY_AFTER_FINGERPRINT_MISMATCH",
                format!("{path}.afterFingerprint"),
                format!("the authoritative post-overlay stage fingerprints as {after_fingerprint}"),
            ));
        }
        stage = next;
    }
    if stage != materialization_stage(final_definition) {
        diagnostics.push(RpgDiagnostic::error(
            RpgDiagnosticStage::Artifact,
            "RULESET_OVERLAY_FINAL_STAGE_MISMATCH",
            "$.overlayProvenance",
            format!(
                "replaying overlays for {} does not produce the materialized definition",
                final_definition.id
            ),
        ));
    }
}

fn apply_ruleset_patch(
    stage: &RulesetMaterializationStage,
    patch: &RulesetPatch,
    parameters: &BTreeMap<String, Value>,
) -> Result<
    (
        RulesetMaterializationStage,
        Vec<RulesetPatchChangeProvenance>,
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
        if plane == RulesetImpactPlane::Both {
            return Err(
                "an individual patch operation must name semantic or presentation".to_owned(),
            );
        }
        let root = patch_plane_root_mut(&mut next, plane)?;
        let path = patch_operation_path(operation);
        let before = read_patch_path(root, path)?.clone();
        match operation {
            RulesetPatchOperation::SetScalar { value, .. } => {
                let replacement = resolve_patch_scalar(value, parameters)?;
                write_patch_path(root, path, replacement)?;
            }
            RulesetPatchOperation::AdjustNumber { multiply, add, .. } => {
                let current = read_patch_path(root, path)?
                    .as_f64()
                    .ok_or_else(|| "adjustNumber requires a numeric target".to_owned())?;
                let multiplier = resolve_patch_number(multiply, parameters)?;
                let addend = resolve_patch_number(add, parameters)?;
                write_patch_path(root, path, json_number(current * multiplier + addend)?)?;
            }
            RulesetPatchOperation::AppendMember {
                identity,
                value,
                position,
                ..
            } => append_patch_member(root, path, identity, value, position)?,
            RulesetPatchOperation::RemoveMember { identity, .. } => {
                remove_patch_member(root, path, identity)?;
            }
        }
        let after = read_patch_path(root, path)?.clone();
        changes.push(RulesetPatchChangeProvenance {
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

fn patch_operation_plane(operation: &RulesetPatchOperation) -> RulesetImpactPlane {
    match operation {
        RulesetPatchOperation::SetScalar { plane, .. }
        | RulesetPatchOperation::AdjustNumber { plane, .. }
        | RulesetPatchOperation::AppendMember { plane, .. }
        | RulesetPatchOperation::RemoveMember { plane, .. } => *plane,
    }
}

fn patch_operation_path(operation: &RulesetPatchOperation) -> &[RulesetPatchPathSegment] {
    match operation {
        RulesetPatchOperation::SetScalar { path, .. }
        | RulesetPatchOperation::AdjustNumber { path, .. }
        | RulesetPatchOperation::AppendMember { path, .. }
        | RulesetPatchOperation::RemoveMember { path, .. } => path,
    }
}

fn patch_matches_plane(patch: &RulesetPatch, plane: RulesetImpactPlane) -> bool {
    patch.operations.iter().all(|operation| {
        plane == RulesetImpactPlane::Both || patch_operation_plane(operation) == plane
    })
}

fn patch_plane_root_mut(
    stage: &mut RulesetMaterializationStage,
    plane: RulesetImpactPlane,
) -> Result<&mut Value, String> {
    match plane {
        RulesetImpactPlane::Semantic => Ok(&mut stage.value.semantic),
        RulesetImpactPlane::Presentation => Ok(&mut stage.value.presentation),
        RulesetImpactPlane::Both => {
            Err("an individual patch operation cannot target both planes".to_owned())
        }
    }
}

fn read_patch_path<'a>(
    root: &'a Value,
    path: &[RulesetPatchPathSegment],
) -> Result<&'a Value, String> {
    let mut current = root;
    for segment in path {
        current = match segment {
            RulesetPatchPathSegment::Field { name } => current
                .as_object()
                .and_then(|object| object.get(name))
                .ok_or_else(|| format!("field {name} is missing at {}", patch_change_path(path)))?,
            RulesetPatchPathSegment::Member { key, value } => {
                resolve_patch_member(current, *key, value)?
            }
        };
    }
    Ok(current)
}

fn read_patch_path_mut<'a>(
    root: &'a mut Value,
    path: &[RulesetPatchPathSegment],
) -> Result<&'a mut Value, String> {
    let mut current = root;
    for segment in path {
        current = match segment {
            RulesetPatchPathSegment::Field { name } => current
                .as_object_mut()
                .and_then(|object| object.get_mut(name))
                .ok_or_else(|| format!("field {name} is missing at {}", patch_change_path(path)))?,
            RulesetPatchPathSegment::Member { key, value } => {
                resolve_patch_member_mut(current, *key, value)?
            }
        };
    }
    Ok(current)
}

fn write_patch_path(
    root: &mut Value,
    path: &[RulesetPatchPathSegment],
    replacement: Value,
) -> Result<(), String> {
    let Some((leaf, parent_path)) = path.split_last() else {
        return Err("patch operations must not write the root".to_owned());
    };
    let RulesetPatchPathSegment::Field { name } = leaf else {
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

fn resolve_patch_member<'a>(
    value: &'a Value,
    key: RulesetPatchMemberKey,
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
    key: RulesetPatchMemberKey,
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

fn patch_member_matches(value: &Value, key: RulesetPatchMemberKey, expected: &str) -> bool {
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
    path: &[RulesetPatchPathSegment],
    identity: &RulesetPatchMemberSelector,
    value: &BTreeMap<String, Value>,
    position: &RulesetPatchPosition,
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
        RulesetPatchPosition::Start => 0,
        RulesetPatchPosition::End => list.len(),
        RulesetPatchPosition::Before { anchor } | RulesetPatchPosition::After { anchor } => {
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
            matches[0] + usize::from(matches!(position, RulesetPatchPosition::After { .. }))
        }
    };
    list.insert(index, member);
    Ok(())
}

fn remove_patch_member(
    root: &mut Value,
    path: &[RulesetPatchPathSegment],
    identity: &RulesetPatchMemberSelector,
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

fn patch_member_key(key: RulesetPatchMemberKey) -> &'static str {
    match key {
        RulesetPatchMemberKey::Id => "id",
        RulesetPatchMemberKey::ResourceId => "resourceId",
        RulesetPatchMemberKey::StatId => "statId",
        RulesetPatchMemberKey::DefenseId => "defenseId",
        RulesetPatchMemberKey::ModifierId => "modifierId",
        RulesetPatchMemberKey::DamageType => "damageType",
        RulesetPatchMemberKey::Kind => "kind",
    }
}

fn patch_change_path(path: &[RulesetPatchPathSegment]) -> String {
    path.iter()
        .map(|segment| match segment {
            RulesetPatchPathSegment::Field { name } => name.clone(),
            RulesetPatchPathSegment::Member { key, value } => {
                format!("[{}={value}]", patch_member_key(*key))
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn validate_patch_changes(
    changes: &[rpg_ir::RulesetPatchChangeProvenance],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for (index, change) in changes.iter().enumerate() {
        if change.path_segments.is_empty() {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_PATCH_CHANGE_PATH_SEGMENTS_MISSING",
                format!("{path}[{index}].pathSegments"),
                "patch changes must carry a typed path for authoritative reconstruction",
            ));
        }
        let rendered_path = patch_change_path(&change.path_segments);
        if rendered_path != change.path {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_PATCH_CHANGE_PATH_MISMATCH",
                format!("{path}[{index}].path"),
                "the display path must match the canonical typed path",
            ));
        }
        if change.plane == RulesetImpactPlane::Both {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_PATCH_CHANGE_PLANE_INVALID",
                format!("{path}[{index}].plane"),
                "an individual patch change must name semantic or presentation",
            ));
        }
        let effective = change.before != change.after;
        if effective != change.effective {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_PATCH_CHANGE_EFFECT_MISMATCH",
                format!("{path}[{index}].effective"),
                "patch change effectiveness must match canonical before/after values",
            ));
        }
    }
}

pub fn materialized_definition_fingerprint(
    definition: &MaterializedRulesetDefinition,
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
        "languageIdentity": prepared.language_identity,
        "definitions": prepared.materialized_definitions.iter().map(|definition| json!({
            "id": definition.id,
            "kind": definition.kind,
            "visibility": definition.visibility,
            "extensionPolicy": definition.extension_policy,
            "semantic": semantic_definition_value(definition),
            "references": definition.references,
        })).collect::<Vec<_>>(),
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
            "actionName": action_semantic_field(definition, "name"),
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

fn semantic_definition_value(definition: &MaterializedRulesetDefinition) -> Value {
    let mut semantic = definition.semantic.clone();
    if definition.kind == MaterializedRulesetDefinitionKind::Action {
        if let Some(object) = semantic.as_object_mut() {
            object.remove("name");
            object.remove("sourcePath");
        }
    }
    semantic
}

fn action_semantic_field(definition: &MaterializedRulesetDefinition, field: &str) -> Value {
    if definition.kind != MaterializedRulesetDefinitionKind::Action {
        return Value::Null;
    }
    definition
        .semantic
        .get(field)
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
