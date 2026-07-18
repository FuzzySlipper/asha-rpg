use std::collections::{BTreeMap, BTreeSet};

use rpg_ir::{
    CompiledRulesetArtifact, MaterializedRulesetDefinition, MaterializedRulesetDefinitionKind,
    MaterializedRulesetVisibility, NormalizedRpgIr, PreparedRulesetCompilation, RpgIrAction,
    RpgIrCatalogs, RpgIrCheck, RpgIrFormula, RpgIrOperation, RpgIrPackage, RpgIrPredicate,
    RpgIrProgram, RpgIrRequirement, RpgIrRequirementKind, RpgIrSchema, RulesetArtifactFingerprints,
    RulesetArtifactSchema, RulesetRelationshipKind, VersionedRulesetRequirement,
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
    }

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
    for (index, relationship) in prepared.relationships.iter().enumerate() {
        if matches!(relationship.kind, RulesetRelationshipKind::DerivesFrom)
            && prepared.derivation_provenance.is_empty()
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_DERIVATION_PROVENANCE_MISSING",
                format!("$.relationships[{index}]"),
                "derivation relationships require typed materialization provenance",
            ));
        }
        if matches!(relationship.kind, RulesetRelationshipKind::Patches)
            && prepared.overlay_provenance.is_empty()
        {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_OVERLAY_PROVENANCE_MISSING",
                format!("$.relationships[{index}]"),
                "patch relationships require typed overlay provenance",
            ));
        }
    }
}

fn validate_patch_changes(
    changes: &[rpg_ir::RulesetPatchChangeProvenance],
    path: &str,
    diagnostics: &mut Vec<RpgDiagnostic>,
) {
    for (index, change) in changes.iter().enumerate() {
        if change.path.is_empty() {
            diagnostics.push(RpgDiagnostic::error(
                RpgDiagnosticStage::Artifact,
                "RULESET_PATCH_CHANGE_PATH_MISSING",
                format!("{path}[{index}].path"),
                "patch change paths must be explicit",
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
