use std::fmt;

use rpg_compiler::{load_compiled_ruleset_artifact, CompiledRulesetBundle};
use rpg_core::{
    ActiveRpgModifier, BoundedValue, GridPosition, RpgCapabilityState, RpgEntityState,
    RpgResolutionReceipt, StateFingerprint, Team,
};
use rpg_ir::{
    CompiledRulesetArtifact, CompiledRulesetIdentity, ResolvedRulesetSourcePackage,
    RulesetArtifactFingerprints, RulesetArtifactSchema, RulesetDependencyLockEntry,
    VersionedRulesetRequirement,
};
use serde::{Deserialize, Serialize};

use crate::semantic_session::{
    PendingTransaction, RpgAuthorityCommand, RpgAuthoritySession, RpgCommandOutcome,
    RpgPendingReaction, RpgReactionCommand,
};

pub const RPG_CHECKPOINT_SCHEMA_ID: &str = "asha.rpg.session.checkpoint";
pub const RPG_REPLAY_ENTRY_SCHEMA_ID: &str = "asha.rpg.session.replay-entry";
pub const RPG_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
pub const RPG_REPLAY_ENTRY_SCHEMA_VERSION: u32 = 1;
pub const RPG_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableSchemaIdentity {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplaySchemaVersions {
    pub checkpoint: u32,
    pub replay_entry: u32,
    pub event: u32,
    pub operations: Vec<VersionedRulesetRequirement>,
    pub capabilities: Vec<VersionedRulesetRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgDefinitionFingerprintBinding {
    pub id: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayArtifactBinding {
    pub artifact_schema: RulesetArtifactSchema,
    pub artifact_id: String,
    pub composition: CompiledRulesetIdentity,
    pub language: CompiledRulesetIdentity,
    pub source_packages: Vec<ResolvedRulesetSourcePackage>,
    pub dependency_lock: Vec<RulesetDependencyLockEntry>,
    pub fingerprints: RulesetArtifactFingerprints,
    pub definitions: Vec<RpgDefinitionFingerprintBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableNamedInteger {
    pub id: String,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableNamedBoundedValue {
    pub id: String,
    pub value: BoundedValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableModifier {
    pub stacking_group: String,
    pub id: String,
    pub value: i32,
    pub remaining_turns: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableEntityState {
    pub id: String,
    pub team: Team,
    pub position: GridPosition,
    pub vitality: BoundedValue,
    pub stats: Vec<RpgPortableNamedInteger>,
    pub defenses: Vec<RpgPortableNamedInteger>,
    pub resources: Vec<RpgPortableNamedBoundedValue>,
    pub modifiers: Vec<RpgPortableModifier>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableCapabilityState {
    pub revision: u64,
    pub entities: Vec<RpgPortableEntityState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgCheckpointPhase {
    Ready,
    AwaitingReaction {
        expected_revision: u64,
        intent: rpg_core::RpgIntent,
        random_values: Vec<u32>,
        pending: Box<RpgPendingReaction>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgSessionCheckpoint {
    pub schema: RpgPortableSchemaIdentity,
    pub schemas: RpgReplaySchemaVersions,
    pub artifact_binding: RpgReplayArtifactBinding,
    pub artifact: CompiledRulesetArtifact,
    pub state: RpgPortableCapabilityState,
    pub accepted_random_position: u64,
    pub phase: RpgCheckpointPhase,
    pub state_hash: StateFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgReplayPhase {
    Ready,
    AwaitingReaction { reaction_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayBoundary {
    pub revision: u64,
    pub accepted_random_position: u64,
    pub phase: RpgReplayPhase,
    pub state_hash: StateFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgReplayOperation {
    Submit { command: RpgAuthorityCommand },
    React { command: RpgReactionCommand },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayEntry {
    pub schema: RpgPortableSchemaIdentity,
    pub schemas: RpgReplaySchemaVersions,
    pub before: RpgReplayBoundary,
    pub operation: RpgReplayOperation,
    pub outcome: RpgCommandOutcome,
    pub after: RpgReplayBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayFailure {
    pub diagnostics: Vec<RpgReplayDiagnostic>,
}

impl fmt::Display for RpgReplayFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let summary = self
            .diagnostics
            .first()
            .map(|diagnostic| diagnostic.message.as_str())
            .unwrap_or("portable RPG replay failed");
        formatter.write_str(summary)
    }
}

impl std::error::Error for RpgReplayFailure {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgArtifactCompatibilityReport {
    pub exact: bool,
    pub semantic_compatible: bool,
    pub diagnostics: Vec<RpgReplayDiagnostic>,
}

impl RpgAuthoritySession {
    pub fn checkpoint(&self) -> Result<RpgSessionCheckpoint, RpgReplayFailure> {
        let artifact = self.artifact.clone().ok_or_else(|| {
            replay_failure(
                "RPG_CHECKPOINT_ARTIFACT_REQUIRED",
                "$.artifact",
                "portable checkpoints require a session created from a compiled ruleset artifact",
            )
        })?;
        let state = portable_state(&self.state);
        let phase = checkpoint_phase(self);
        let accepted_random_position = self.accepted_random_values;
        let state_hash = session_state_hash(&state, accepted_random_position, &phase)?;
        Ok(RpgSessionCheckpoint {
            schema: checkpoint_schema(),
            schemas: replay_versions(&artifact),
            artifact_binding: artifact_binding(&artifact),
            artifact,
            state,
            accepted_random_position,
            phase,
            state_hash,
        })
    }

    pub fn checkpoint_json(&self) -> Result<Vec<u8>, RpgReplayFailure> {
        serde_json::to_vec(&self.checkpoint()?).map_err(|error| {
            replay_failure(
                "RPG_CHECKPOINT_ENCODE_FAILED",
                "$",
                format!("checkpoint encoding failed: {error}"),
            )
        })
    }

    pub fn restore_checkpoint(checkpoint: RpgSessionCheckpoint) -> Result<Self, RpgReplayFailure> {
        validate_checkpoint_schema(&checkpoint)?;
        validate_artifact_binding(&checkpoint.artifact_binding, &checkpoint.artifact)?;
        let bundle =
            load_compiled_ruleset_artifact(checkpoint.artifact.clone()).map_err(|failure| {
                RpgReplayFailure {
                    diagnostics: failure
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| {
                            let code = checkpoint_artifact_mismatch_code(
                                &diagnostic.path,
                                &diagnostic.code,
                            );
                            RpgReplayDiagnostic {
                                code: code.to_owned(),
                                path: diagnostic.path,
                                message: format!("{}: {}", diagnostic.code, diagnostic.message),
                                expected: None,
                                actual: None,
                            }
                        })
                        .collect(),
                }
            })?;
        let expected_versions = replay_versions(bundle.artifact());
        if checkpoint.schemas != expected_versions {
            return Err(replay_mismatch(
                "RPG_CHECKPOINT_SCHEMA_VERSIONS_MISMATCH",
                "$.schemas",
                &expected_versions,
                &checkpoint.schemas,
            ));
        }
        let state = restore_state(&checkpoint.state)?;
        let actual_hash = session_state_hash(
            &checkpoint.state,
            checkpoint.accepted_random_position,
            &checkpoint.phase,
        )?;
        if checkpoint.state_hash != actual_hash {
            return Err(replay_mismatch(
                "RPG_CHECKPOINT_STATE_HASH_MISMATCH",
                "$.stateHash",
                &checkpoint.state_hash,
                &actual_hash,
            ));
        }
        let pending = restore_phase(&checkpoint.phase, &bundle, &state)?;
        Ok(Self {
            artifact: Some(bundle.artifact().clone()),
            ruleset: bundle.ruleset().clone(),
            state,
            pending,
            accepted_random_values: checkpoint.accepted_random_position,
        })
    }

    pub fn restore_checkpoint_json(source: &[u8]) -> Result<Self, RpgReplayFailure> {
        let checkpoint =
            serde_json::from_slice::<RpgSessionCheckpoint>(source).map_err(|error| {
                replay_failure(
                    "RPG_CHECKPOINT_DECODE_FAILED",
                    "$",
                    format!("checkpoint decoding failed: {error}"),
                )
            })?;
        Self::restore_checkpoint(checkpoint)
    }

    pub fn replace_from_checkpoint(
        &mut self,
        checkpoint: RpgSessionCheckpoint,
    ) -> Result<(), RpgReplayFailure> {
        let restored = Self::restore_checkpoint(checkpoint)?;
        *self = restored;
        Ok(())
    }

    pub fn submit_recorded(
        &mut self,
        command: RpgAuthorityCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::Submit { command })
    }

    pub fn react_recorded(
        &mut self,
        command: RpgReactionCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::React { command })
    }

    pub fn replay(
        checkpoint: RpgSessionCheckpoint,
        entries: &[RpgReplayEntry],
    ) -> Result<Self, RpgReplayFailure> {
        let mut session = Self::restore_checkpoint(checkpoint)?;
        for (index, entry) in entries.iter().enumerate() {
            session.replay_entry(entry, index)?;
        }
        Ok(session)
    }

    pub fn replay_into(
        &mut self,
        checkpoint: RpgSessionCheckpoint,
        entries: &[RpgReplayEntry],
    ) -> Result<(), RpgReplayFailure> {
        let replayed = Self::replay(checkpoint, entries)?;
        *self = replayed;
        Ok(())
    }

    pub fn state_hash(&self) -> Result<StateFingerprint, RpgReplayFailure> {
        let state = portable_state(&self.state);
        let phase = checkpoint_phase(self);
        session_state_hash(&state, self.accepted_random_values, &phase)
    }

    fn record_operation(
        &mut self,
        operation: RpgReplayOperation,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        let artifact = self.artifact.as_ref().ok_or_else(|| {
            replay_failure(
                "RPG_REPLAY_ARTIFACT_REQUIRED",
                "$.artifact",
                "recording requires a session created from a compiled ruleset artifact",
            )
        })?;
        let schemas = replay_versions(artifact);
        let before = replay_boundary(self)?;
        let outcome = match &operation {
            RpgReplayOperation::Submit { command } => self.submit(command.clone()),
            RpgReplayOperation::React { command } => self.react(command.clone()),
        };
        let after = replay_boundary(self)?;
        let entry = RpgReplayEntry {
            schema: replay_entry_schema(),
            schemas,
            before,
            operation,
            outcome: outcome.clone(),
            after,
        };
        Ok((outcome, entry))
    }

    fn replay_entry(
        &mut self,
        expected: &RpgReplayEntry,
        index: usize,
    ) -> Result<(), RpgReplayFailure> {
        let base_path = format!("$.entries[{index}]");
        validate_replay_entry_schema(expected, &base_path)?;
        let artifact = self.artifact.as_ref().ok_or_else(|| {
            replay_failure(
                "RPG_REPLAY_ARTIFACT_REQUIRED",
                format!("{base_path}.artifact"),
                "replay requires an exact compiled artifact",
            )
        })?;
        let versions = replay_versions(artifact);
        if expected.schemas != versions {
            return Err(replay_mismatch(
                "RPG_REPLAY_SCHEMA_VERSIONS_MISMATCH",
                format!("{base_path}.schemas"),
                &versions,
                &expected.schemas,
            ));
        }
        let before = replay_boundary(self)?;
        compare_boundary(&expected.before, &before, &format!("{base_path}.before"))?;
        let outcome = match &expected.operation {
            RpgReplayOperation::Submit { command } => self.submit(command.clone()),
            RpgReplayOperation::React { command } => self.react(command.clone()),
        };
        compare_outcome(&expected.outcome, &outcome, &format!("{base_path}.outcome"))?;
        let after = replay_boundary(self)?;
        compare_boundary(&expected.after, &after, &format!("{base_path}.after"))?;
        Ok(())
    }
}

pub fn classify_checkpoint_artifact(
    checkpoint: &RpgSessionCheckpoint,
    candidate: &CompiledRulesetArtifact,
) -> RpgArtifactCompatibilityReport {
    let historical = &checkpoint.artifact;
    let mut diagnostics = Vec::new();
    if historical.source_packages != candidate.source_packages {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_PACKAGE_SET_CHANGED",
            "$.artifact.sourcePackages",
            "exact source package identities, versions, or fingerprints changed",
        ));
    }
    if historical.dependency_lock != candidate.dependency_lock {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_DEPENDENCY_LOCK_CHANGED",
            "$.artifact.dependencyLock",
            "the exact dependency lock changed",
        ));
    }
    if historical.fingerprints.source != candidate.fingerprints.source {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_SOURCE_FINGERPRINT_CHANGED",
            "$.artifact.fingerprints.source",
            "source identity changed",
        ));
    }
    if historical.fingerprints.presentation != candidate.fingerprints.presentation {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_PRESENTATION_FINGERPRINT_CHANGED",
            "$.artifact.fingerprints.presentation",
            "presentation identity changed",
        ));
    }
    if historical.fingerprints.semantic != candidate.fingerprints.semantic {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_SEMANTIC_FINGERPRINT_CHANGED",
            "$.artifact.fingerprints.semantic",
            "semantic identity changed",
        ));
    }
    if historical.artifact_id != candidate.artifact_id {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_ARTIFACT_ID_CHANGED",
            "$.artifact.artifactId",
            "artifact identity changed; exact replay will continue using the embedded historical artifact",
        ));
    }
    RpgArtifactCompatibilityReport {
        exact: diagnostics.is_empty(),
        semantic_compatible: historical.fingerprints.semantic == candidate.fingerprints.semantic,
        diagnostics,
    }
}

pub fn encode_replay_entries(entries: &[RpgReplayEntry]) -> Result<Vec<u8>, RpgReplayFailure> {
    serde_json::to_vec(entries).map_err(|error| {
        replay_failure(
            "RPG_REPLAY_ENTRIES_ENCODE_FAILED",
            "$",
            format!("replay entry encoding failed: {error}"),
        )
    })
}

pub fn decode_replay_entries(source: &[u8]) -> Result<Vec<RpgReplayEntry>, RpgReplayFailure> {
    serde_json::from_slice(source).map_err(|error| {
        replay_failure(
            "RPG_REPLAY_ENTRIES_DECODE_FAILED",
            "$",
            format!("replay entry decoding failed: {error}"),
        )
    })
}

fn checkpoint_schema() -> RpgPortableSchemaIdentity {
    RpgPortableSchemaIdentity {
        id: RPG_CHECKPOINT_SCHEMA_ID.to_owned(),
        version: RPG_CHECKPOINT_SCHEMA_VERSION,
    }
}

fn replay_entry_schema() -> RpgPortableSchemaIdentity {
    RpgPortableSchemaIdentity {
        id: RPG_REPLAY_ENTRY_SCHEMA_ID.to_owned(),
        version: RPG_REPLAY_ENTRY_SCHEMA_VERSION,
    }
}

fn replay_versions(artifact: &CompiledRulesetArtifact) -> RpgReplaySchemaVersions {
    RpgReplaySchemaVersions {
        checkpoint: RPG_CHECKPOINT_SCHEMA_VERSION,
        replay_entry: RPG_REPLAY_ENTRY_SCHEMA_VERSION,
        event: RPG_EVENT_SCHEMA_VERSION,
        operations: artifact.required_operations.clone(),
        capabilities: artifact.required_capabilities.clone(),
    }
}

fn artifact_binding(artifact: &CompiledRulesetArtifact) -> RpgReplayArtifactBinding {
    RpgReplayArtifactBinding {
        artifact_schema: artifact.artifact_schema.clone(),
        artifact_id: artifact.artifact_id.clone(),
        composition: artifact.composition_identity.clone(),
        language: artifact.language_identity.clone(),
        source_packages: artifact.source_packages.clone(),
        dependency_lock: artifact.dependency_lock.clone(),
        fingerprints: artifact.fingerprints.clone(),
        definitions: artifact
            .materialized_definitions
            .iter()
            .map(|definition| RpgDefinitionFingerprintBinding {
                id: definition.id.clone(),
                fingerprint: definition.fingerprint.clone(),
            })
            .collect(),
    }
}

fn validate_artifact_binding(
    expected: &RpgReplayArtifactBinding,
    artifact: &CompiledRulesetArtifact,
) -> Result<(), RpgReplayFailure> {
    let actual = artifact_binding(artifact);
    if expected.artifact_schema != actual.artifact_schema {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_ARTIFACT_SCHEMA_MISMATCH",
            "$.artifact.artifactSchema",
            &expected.artifact_schema,
            &actual.artifact_schema,
        ));
    }
    if expected.artifact_id != actual.artifact_id {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_ARTIFACT_ID_MISMATCH",
            "$.artifact.artifactId",
            &expected.artifact_id,
            &actual.artifact_id,
        ));
    }
    if expected.composition != actual.composition {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_COMPOSITION_MISMATCH",
            "$.artifact.compositionIdentity",
            &expected.composition,
            &actual.composition,
        ));
    }
    if expected.language != actual.language {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_LANGUAGE_MISMATCH",
            "$.artifact.languageIdentity",
            &expected.language,
            &actual.language,
        ));
    }
    if expected.source_packages != actual.source_packages {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_PACKAGE_MISMATCH",
            "$.artifact.sourcePackages",
            &expected.source_packages,
            &actual.source_packages,
        ));
    }
    if expected.dependency_lock != actual.dependency_lock {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_LOCK_MISMATCH",
            "$.artifact.dependencyLock",
            &expected.dependency_lock,
            &actual.dependency_lock,
        ));
    }
    if expected.fingerprints.source != actual.fingerprints.source {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_SOURCE_MISMATCH",
            "$.artifact.fingerprints.source",
            &expected.fingerprints.source,
            &actual.fingerprints.source,
        ));
    }
    if expected.fingerprints.semantic != actual.fingerprints.semantic {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_SEMANTIC_MISMATCH",
            "$.artifact.fingerprints.semantic",
            &expected.fingerprints.semantic,
            &actual.fingerprints.semantic,
        ));
    }
    if expected.fingerprints.presentation != actual.fingerprints.presentation {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_PRESENTATION_MISMATCH",
            "$.artifact.fingerprints.presentation",
            &expected.fingerprints.presentation,
            &actual.fingerprints.presentation,
        ));
    }
    if expected.definitions != actual.definitions {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_DEFINITION_FINGERPRINT_MISMATCH",
            "$.artifact.materializedDefinitions",
            &expected.definitions,
            &actual.definitions,
        ));
    }
    Ok(())
}

fn validate_checkpoint_schema(checkpoint: &RpgSessionCheckpoint) -> Result<(), RpgReplayFailure> {
    let expected = checkpoint_schema();
    if checkpoint.schema != expected {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_SCHEMA_MISMATCH",
            "$.schema",
            &expected,
            &checkpoint.schema,
        ));
    }
    Ok(())
}

fn validate_replay_entry_schema(
    entry: &RpgReplayEntry,
    path: &str,
) -> Result<(), RpgReplayFailure> {
    let expected = replay_entry_schema();
    if entry.schema != expected {
        return Err(replay_mismatch(
            "RPG_REPLAY_ENTRY_SCHEMA_MISMATCH",
            format!("{path}.schema"),
            &expected,
            &entry.schema,
        ));
    }
    Ok(())
}

fn portable_state(state: &RpgCapabilityState) -> RpgPortableCapabilityState {
    RpgPortableCapabilityState {
        revision: state.revision(),
        entities: state
            .entities()
            .map(|entity| RpgPortableEntityState {
                id: entity.id().to_owned(),
                team: entity.team(),
                position: entity.position(),
                vitality: entity.vitality(),
                stats: entity
                    .stats()
                    .map(|(id, value)| RpgPortableNamedInteger {
                        id: id.to_owned(),
                        value,
                    })
                    .collect(),
                defenses: entity
                    .defenses()
                    .map(|(id, value)| RpgPortableNamedInteger {
                        id: id.to_owned(),
                        value,
                    })
                    .collect(),
                resources: entity
                    .resources()
                    .map(|(id, value)| RpgPortableNamedBoundedValue {
                        id: id.to_owned(),
                        value,
                    })
                    .collect(),
                modifiers: entity
                    .modifiers()
                    .map(|(stacking_group, modifier)| RpgPortableModifier {
                        stacking_group: stacking_group.to_owned(),
                        id: modifier.id().to_owned(),
                        value: modifier.value(),
                        remaining_turns: modifier.remaining_turns(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn restore_state(
    state: &RpgPortableCapabilityState,
) -> Result<RpgCapabilityState, RpgReplayFailure> {
    let mut entities = Vec::with_capacity(state.entities.len());
    for (entity_index, source) in state.entities.iter().enumerate() {
        let path = format!("$.state.entities[{entity_index}]");
        let mut entity = RpgEntityState::restore(
            source.id.clone(),
            source.team,
            source.position,
            source.vitality,
        )
        .map_err(|error| state_restore_failure(&path, error))?;
        for stat in &source.stats {
            entity
                .restore_stat(stat.id.clone(), stat.value)
                .map_err(|error| state_restore_failure(&format!("{path}.stats"), error))?;
        }
        for defense in &source.defenses {
            entity
                .restore_defense(defense.id.clone(), defense.value)
                .map_err(|error| state_restore_failure(&format!("{path}.defenses"), error))?;
        }
        for resource in &source.resources {
            entity
                .restore_resource(resource.id.clone(), resource.value)
                .map_err(|error| state_restore_failure(&format!("{path}.resources"), error))?;
        }
        for modifier in &source.modifiers {
            entity
                .restore_modifier(
                    modifier.stacking_group.clone(),
                    ActiveRpgModifier::restore(
                        modifier.id.clone(),
                        modifier.value,
                        modifier.remaining_turns,
                    ),
                )
                .map_err(|error| state_restore_failure(&format!("{path}.modifiers"), error))?;
        }
        entities.push(entity);
    }
    RpgCapabilityState::restore(state.revision, entities)
        .map_err(|error| state_restore_failure("$.state", error))
}

fn state_restore_failure(path: &str, error: rpg_core::RpgStateRestoreError) -> RpgReplayFailure {
    replay_failure(
        "RPG_CHECKPOINT_STATE_INVALID",
        path,
        format!("portable capability state is invalid: {error:?}"),
    )
}

fn checkpoint_phase(session: &RpgAuthoritySession) -> RpgCheckpointPhase {
    match &session.pending {
        None => RpgCheckpointPhase::Ready,
        Some(transaction) => RpgCheckpointPhase::AwaitingReaction {
            expected_revision: transaction.expected_revision,
            intent: transaction.intent.clone(),
            random_values: transaction.random_values.clone(),
            pending: Box::new(transaction.pending.clone()),
        },
    }
}

fn restore_phase(
    phase: &RpgCheckpointPhase,
    bundle: &CompiledRulesetBundle,
    state: &RpgCapabilityState,
) -> Result<Option<PendingTransaction>, RpgReplayFailure> {
    match phase {
        RpgCheckpointPhase::Ready => Ok(None),
        RpgCheckpointPhase::AwaitingReaction {
            expected_revision,
            intent,
            random_values,
            pending,
        } => {
            let mut proof =
                RpgAuthoritySession::from_compiled_ruleset(bundle.clone(), state.clone());
            let outcome = proof.submit(RpgAuthorityCommand {
                expected_revision: *expected_revision,
                intent: intent.clone(),
                random_values: random_values.clone(),
            });
            let RpgCommandOutcome::AwaitingReaction(actual) = outcome else {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase",
                    pending.as_ref(),
                    &outcome,
                ));
            };
            if pending.random_evidence != actual.random_evidence
                || pending.random_attempted != actual.random_attempted
            {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_EVIDENCE_MISMATCH",
                    "$.phase.pending.randomEvidence",
                    &(&pending.random_evidence, pending.random_attempted),
                    &(&actual.random_evidence, actual.random_attempted),
                ));
            }
            if pending.as_ref() != &actual {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase.pending",
                    pending.as_ref(),
                    &actual,
                ));
            }
            proof
                .pending
                .take()
                .ok_or_else(|| {
                    replay_failure(
                        "RPG_CHECKPOINT_PHASE_MISMATCH",
                        "$.phase.pending",
                        "Rust authority did not retain the verified pending transaction",
                    )
                })
                .map(Some)
        }
    }
}

fn replay_boundary(session: &RpgAuthoritySession) -> Result<RpgReplayBoundary, RpgReplayFailure> {
    let phase = match session.pending_reaction() {
        None => RpgReplayPhase::Ready,
        Some(pending) => RpgReplayPhase::AwaitingReaction {
            reaction_id: pending.request.reaction_id.clone(),
        },
    };
    Ok(RpgReplayBoundary {
        revision: session.state.revision(),
        accepted_random_position: session.accepted_random_values,
        phase,
        state_hash: session.state_hash()?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionHashInput<'a> {
    state: &'a RpgPortableCapabilityState,
    accepted_random_position: u64,
    phase: &'a RpgCheckpointPhase,
}

fn session_state_hash(
    state: &RpgPortableCapabilityState,
    accepted_random_position: u64,
    phase: &RpgCheckpointPhase,
) -> Result<StateFingerprint, RpgReplayFailure> {
    let bytes = serde_json::to_vec(&SessionHashInput {
        state,
        accepted_random_position,
        phase,
    })
    .map_err(|error| {
        replay_failure(
            "RPG_CHECKPOINT_STATE_HASH_FAILED",
            "$.state",
            format!("canonical state hashing failed: {error}"),
        )
    })?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(StateFingerprint {
        algorithm: "fnv1a64.rpg-session.v1".to_owned(),
        value: format!("{hash:016x}"),
    })
}

fn compare_boundary(
    expected: &RpgReplayBoundary,
    actual: &RpgReplayBoundary,
    path: &str,
) -> Result<(), RpgReplayFailure> {
    if expected.revision != actual.revision {
        return Err(replay_mismatch(
            "RPG_REPLAY_REVISION_MISMATCH",
            format!("{path}.revision"),
            &expected.revision,
            &actual.revision,
        ));
    }
    if expected.phase != actual.phase {
        return Err(replay_mismatch(
            "RPG_REPLAY_PHASE_MISMATCH",
            format!("{path}.phase"),
            &expected.phase,
            &actual.phase,
        ));
    }
    if expected.accepted_random_position != actual.accepted_random_position {
        return Err(replay_mismatch(
            "RPG_REPLAY_EVIDENCE_POSITION_MISMATCH",
            format!("{path}.acceptedRandomPosition"),
            &expected.accepted_random_position,
            &actual.accepted_random_position,
        ));
    }
    if expected.state_hash != actual.state_hash {
        return Err(replay_mismatch(
            "RPG_REPLAY_STATE_HASH_MISMATCH",
            format!("{path}.stateHash"),
            &expected.state_hash,
            &actual.state_hash,
        ));
    }
    Ok(())
}

fn compare_outcome(
    expected: &RpgCommandOutcome,
    actual: &RpgCommandOutcome,
    path: &str,
) -> Result<(), RpgReplayFailure> {
    match (expected, actual) {
        (RpgCommandOutcome::Accepted(expected), RpgCommandOutcome::Accepted(actual)) => {
            compare_receipt(expected, actual, path)?;
        }
        (
            RpgCommandOutcome::AwaitingReaction(expected),
            RpgCommandOutcome::AwaitingReaction(actual),
        ) => {
            if expected.random_evidence != actual.random_evidence {
                return Err(replay_mismatch(
                    "RPG_REPLAY_EVIDENCE_MISMATCH",
                    format!("{path}.result.randomEvidence"),
                    &expected.random_evidence,
                    &actual.random_evidence,
                ));
            }
            if expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_PHASE_MISMATCH",
                    path,
                    expected,
                    actual,
                ));
            }
        }
        (RpgCommandOutcome::Rejected(expected), RpgCommandOutcome::Rejected(actual)) => {
            if expected.random_evidence != actual.random_evidence {
                return Err(replay_mismatch(
                    "RPG_REPLAY_EVIDENCE_MISMATCH",
                    format!("{path}.result.randomEvidence"),
                    &expected.random_evidence,
                    &actual.random_evidence,
                ));
            }
            if expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_REJECTION_MISMATCH",
                    path,
                    expected,
                    actual,
                ));
            }
        }
        _ => {
            return Err(replay_mismatch(
                "RPG_REPLAY_PHASE_MISMATCH",
                path,
                expected,
                actual,
            ));
        }
    }
    Ok(())
}

fn compare_receipt(
    expected: &RpgResolutionReceipt,
    actual: &RpgResolutionReceipt,
    path: &str,
) -> Result<(), RpgReplayFailure> {
    if expected.random_evidence != actual.random_evidence
        || expected.random_consumed != actual.random_consumed
    {
        return Err(replay_mismatch(
            "RPG_REPLAY_EVIDENCE_MISMATCH",
            format!("{path}.result.randomEvidence"),
            &(&expected.random_evidence, expected.random_consumed),
            &(&actual.random_evidence, actual.random_consumed),
        ));
    }
    if expected.events != actual.events {
        return Err(replay_mismatch(
            "RPG_REPLAY_EVENT_MISMATCH",
            format!("{path}.result.events"),
            &expected.events,
            &actual.events,
        ));
    }
    if expected.state_revision != actual.state_revision {
        return Err(replay_mismatch(
            "RPG_REPLAY_REVISION_MISMATCH",
            format!("{path}.result.stateRevision"),
            &expected.state_revision,
            &actual.state_revision,
        ));
    }
    if expected != actual {
        return Err(replay_mismatch(
            "RPG_REPLAY_RECORD_MISMATCH",
            path,
            expected,
            actual,
        ));
    }
    Ok(())
}

fn compatibility_diagnostic(code: &str, path: &str, message: &str) -> RpgReplayDiagnostic {
    RpgReplayDiagnostic {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
        expected: None,
        actual: None,
    }
}

fn checkpoint_artifact_mismatch_code(path: &str, compiler_code: &str) -> &'static str {
    if path.contains("sourcePackages") {
        "RPG_CHECKPOINT_PACKAGE_MISMATCH"
    } else if path.contains("dependencyLock") {
        "RPG_CHECKPOINT_LOCK_MISMATCH"
    } else if path.contains("fingerprints.semantic") {
        "RPG_CHECKPOINT_SEMANTIC_MISMATCH"
    } else if path.contains("fingerprints.presentation") {
        "RPG_CHECKPOINT_PRESENTATION_MISMATCH"
    } else if path.contains("fingerprints.source") {
        "RPG_CHECKPOINT_SOURCE_MISMATCH"
    } else if compiler_code.contains("ARTIFACT_ID") {
        "RPG_CHECKPOINT_ARTIFACT_ID_MISMATCH"
    } else {
        "RPG_CHECKPOINT_ARTIFACT_INVALID"
    }
}

fn replay_failure(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> RpgReplayFailure {
    RpgReplayFailure {
        diagnostics: vec![RpgReplayDiagnostic {
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
            expected: None,
            actual: None,
        }],
    }
}

fn replay_mismatch<Expected: fmt::Debug, Actual: fmt::Debug>(
    code: &str,
    path: impl Into<String>,
    expected: &Expected,
    actual: &Actual,
) -> RpgReplayFailure {
    RpgReplayFailure {
        diagnostics: vec![RpgReplayDiagnostic {
            code: code.to_owned(),
            path: path.into(),
            message: "portable replay evidence did not match Rust authority".to_owned(),
            expected: Some(format!("{expected:?}")),
            actual: Some(format!("{actual:?}")),
        }],
    }
}

#[cfg(test)]
mod tests {
    use rpg_compiler::{compile_prepared_ruleset, materialized_definition_fingerprint};
    use rpg_core::{GridPosition, RpgDomainEvent, RpgEntityState, RpgIntent, Team};
    use rpg_ir::{
        CompiledRulesetIdentity, MaterializedRulesetDefinition, MaterializedRulesetDefinitionKind,
        MaterializedRulesetVisibility, PreparedRulesetCompilation, ResolvedRulesetSourcePackage,
        RulesetArtifactSchema, RulesetDefinitionProvenance, RulesetDependencyLockEntry,
        RulesetDependencyRelationship, RulesetExtensionPolicy, RulesetRelationshipKind,
        RulesetRelationshipProvenance, RulesetSourceLocation, VersionedRulesetRequirement,
        PREPARED_RULESET_IDENTITY, RULESET_ARTIFACT_MAJOR,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn clean_process_restores_pending_phase_and_replays_identically() {
        let mut recorded = artifact_session();
        let initial = recorded.checkpoint().expect("initial checkpoint");
        let (pending_outcome, pending_entry) = recorded
            .submit_recorded(command())
            .expect("pending command records");
        let RpgCommandOutcome::AwaitingReaction(pending) = pending_outcome else {
            panic!("command must suspend");
        };
        let pending_checkpoint = recorded.checkpoint().expect("pending checkpoint");
        let pending_json = serde_json::to_vec(&pending_checkpoint).expect("checkpoint encodes");
        let mut restored = RpgAuthoritySession::restore_checkpoint_json(&pending_json)
            .expect("checkpoint restores");
        assert_eq!(
            restored.state_hash().unwrap(),
            recorded.state_hash().unwrap()
        );
        assert_eq!(
            restored.pending_reaction().unwrap().request.reaction_id,
            pending.request.reaction_id
        );

        let reaction = reaction_command();
        let (recorded_outcome, reaction_entry) = recorded
            .react_recorded(reaction.clone())
            .expect("reaction records");
        let restored_outcome = restored.react(reaction);
        assert_eq!(recorded_outcome, restored_outcome);
        assert_eq!(
            recorded.state_hash().unwrap(),
            restored.state_hash().unwrap()
        );

        let replayed =
            RpgAuthoritySession::replay(initial, &[pending_entry.clone(), reaction_entry.clone()])
                .expect("clean replay matches");
        assert_eq!(replayed.state(), recorded.state());
        assert_eq!(
            replayed.state_hash().unwrap(),
            recorded.state_hash().unwrap()
        );
        assert_eq!(replayed.accepted_random_values(), 2);

        let RpgCommandOutcome::Accepted(receipt) = recorded_outcome else {
            panic!("reaction must commit");
        };
        assert_eq!(receipt.random_evidence.len(), 2);
        assert!(receipt
            .events
            .iter()
            .any(|event| matches!(event, RpgDomainEvent::DamageApplied { amount: 1, .. })));
        assert_eq!(reaction_entry.after.revision, 1);
        assert_eq!(reaction_entry.after.phase, RpgReplayPhase::Ready);
    }

    #[test]
    fn corrupt_restore_and_mismatched_replay_are_atomic_and_classified() {
        let mut source = artifact_session();
        let initial = source.checkpoint().unwrap();
        let (_, pending_entry) = source.submit_recorded(command()).unwrap();
        let (_, reaction_entry) = source.react_recorded(reaction_command()).unwrap();

        let mut target = artifact_session();
        let target_hash = target.state_hash().unwrap();
        let mut corrupt = initial.clone();
        corrupt.state.entities[0].vitality.current = 19;
        let restore_error = target.replace_from_checkpoint(corrupt).unwrap_err();
        assert_eq!(
            restore_error.diagnostics[0].code,
            "RPG_CHECKPOINT_STATE_HASH_MISMATCH"
        );
        assert_eq!(target.state_hash().unwrap(), target_hash);

        let mut lock_mismatch = initial.clone();
        lock_mismatch
            .artifact
            .dependency_lock
            .push(RulesetDependencyLockEntry {
                requester: "replay.test@1.0.0".to_owned(),
                package_id: "support.test".to_owned(),
                requested_version: "^1.0.0".to_owned(),
                resolved_version: "1.0.0".to_owned(),
                source_fingerprint: "fnv1a64:2222222222222222".to_owned(),
                import_as: "support".to_owned(),
                relationship: RulesetDependencyRelationship::DependsOn,
            });
        let lock_error = target.replace_from_checkpoint(lock_mismatch).unwrap_err();
        assert_eq!(
            lock_error.diagnostics[0].code,
            "RPG_CHECKPOINT_LOCK_MISMATCH"
        );

        let mut artifact_mismatch = initial.clone();
        artifact_mismatch.artifact.artifact_id = "fnv1a64:changed".to_owned();
        let artifact_error = target
            .replace_from_checkpoint(artifact_mismatch)
            .unwrap_err();
        assert_eq!(
            artifact_error.diagnostics[0].code,
            "RPG_CHECKPOINT_ARTIFACT_ID_MISMATCH"
        );

        let mut semantic_mismatch = initial.clone();
        semantic_mismatch.artifact.fingerprints.semantic = "fnv1a64:changed".to_owned();
        let semantic_error = target
            .replace_from_checkpoint(semantic_mismatch)
            .unwrap_err();
        assert_eq!(
            semantic_error.diagnostics[0].code,
            "RPG_CHECKPOINT_SEMANTIC_MISMATCH"
        );

        let mut definition_mismatch = initial.clone();
        definition_mismatch.artifact.materialized_definitions[0].fingerprint =
            "fnv1a64:changed".to_owned();
        let definition_error = target
            .replace_from_checkpoint(definition_mismatch)
            .unwrap_err();
        assert_eq!(
            definition_error.diagnostics[0].code,
            "RPG_CHECKPOINT_DEFINITION_FINGERPRINT_MISMATCH"
        );
        assert_eq!(target.state_hash().unwrap(), target_hash);

        let mut package_mismatch = initial.clone();
        package_mismatch.artifact.source_packages[0].version = "9.0.0".to_owned();
        let package_error = target
            .replace_from_checkpoint(package_mismatch)
            .unwrap_err();
        assert_eq!(
            package_error.diagnostics[0].code, "RPG_CHECKPOINT_PACKAGE_MISMATCH",
            "{package_error:?}"
        );
        assert_eq!(target.state_hash().unwrap(), target_hash);

        let mut evidence_mismatch = reaction_entry.clone();
        let RpgReplayOperation::React { command } = &mut evidence_mismatch.operation else {
            panic!("reaction entry expected");
        };
        command.additional_random_values[1] = 3;
        let replay_error = target
            .replay_into(initial.clone(), &[pending_entry.clone(), evidence_mismatch])
            .unwrap_err();
        assert_eq!(
            replay_error.diagnostics[0].code,
            "RPG_REPLAY_EVIDENCE_MISMATCH"
        );
        assert_eq!(target.state_hash().unwrap(), target_hash);

        let mut event_mismatch = reaction_entry.clone();
        let RpgCommandOutcome::Accepted(receipt) = &mut event_mismatch.outcome else {
            panic!("accepted replay entry expected");
        };
        receipt.events.clear();
        let event_error =
            RpgAuthoritySession::replay(initial.clone(), &[pending_entry.clone(), event_mismatch])
                .unwrap_err();
        assert_eq!(event_error.diagnostics[0].code, "RPG_REPLAY_EVENT_MISMATCH");

        let mut phase_mismatch = pending_entry.clone();
        phase_mismatch.after.phase = RpgReplayPhase::Ready;
        let phase_error =
            RpgAuthoritySession::replay(initial.clone(), &[phase_mismatch]).unwrap_err();
        assert_eq!(phase_error.diagnostics[0].code, "RPG_REPLAY_PHASE_MISMATCH");

        let mut revision_mismatch = reaction_entry;
        revision_mismatch.after.revision = 9;
        let revision_error =
            RpgAuthoritySession::replay(initial, &[pending_entry, revision_mismatch]).unwrap_err();
        assert_eq!(
            revision_error.diagnostics[0].code,
            "RPG_REPLAY_REVISION_MISMATCH"
        );
    }

    #[test]
    fn artifact_compatibility_never_substitutes_changed_history() {
        let session = artifact_session();
        let checkpoint = session.checkpoint().unwrap();
        let mut candidate = checkpoint.artifact.clone();
        candidate.fingerprints.source = "fnv1a64:source-changed".to_owned();
        candidate.fingerprints.presentation = "fnv1a64:presentation-changed".to_owned();
        let compatible = classify_checkpoint_artifact(&checkpoint, &candidate);
        assert!(!compatible.exact);
        assert!(compatible.semantic_compatible);
        assert!(compatible
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "RPG_REPLAY_SOURCE_FINGERPRINT_CHANGED" }));
        assert!(compatible.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "RPG_REPLAY_PRESENTATION_FINGERPRINT_CHANGED"
        }));

        candidate.fingerprints.semantic = "fnv1a64:semantic-changed".to_owned();
        let incompatible = classify_checkpoint_artifact(&checkpoint, &candidate);
        assert!(!incompatible.semantic_compatible);
        assert!(incompatible
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "RPG_REPLAY_SEMANTIC_FINGERPRINT_CHANGED" }));

        let exact = RpgAuthoritySession::restore_checkpoint(checkpoint).unwrap();
        assert_eq!(
            exact.artifact().unwrap().artifact_id,
            session.artifact().unwrap().artifact_id
        );
    }

    fn artifact_session() -> RpgAuthoritySession {
        let source_location = RulesetSourceLocation {
            module: "actions/replay.rs".to_owned(),
            declaration: "reactiveStrike".to_owned(),
        };
        let provenance = RulesetDefinitionProvenance {
            definition_id: "action.reactive".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: source_location.clone(),
        };
        let mut action = MaterializedRulesetDefinition {
            id: "action.reactive".to_owned(),
            kind: MaterializedRulesetDefinitionKind::Action,
            visibility: MaterializedRulesetVisibility::Exported,
            extension_policy: RulesetExtensionPolicy::Sealed,
            semantic: json!({
                "id": "action.reactive",
                "name": "Reactive strike",
                "sourcePath": "actions/replay.rs#reactiveStrike",
                "targets": {"team": "hostile", "maximumRange": 3, "maximumTargets": 1},
                "check": {"kind": "noRoll"},
                "rollScope": "none",
                "costs": [],
                "program": {"kind": "atomic", "body": {"kind": "sequence", "steps": [
                    {"kind": "operation", "operation": {"kind": "openReaction", "reactionId": "reaction.ward", "options": [
                        {"id": "ward", "label": "Raise ward", "damageReduction": 3}
                    ]}},
                    {"kind": "operation", "operation": {"kind": "damage", "amount": {"kind": "dice", "count": 2, "sides": 6, "bonus": 0}, "damageType": "catalog.damage.force"}}
                ]}}
            }),
            presentation: json!({"label": "Reactive strike"}),
            references: vec!["catalog.damage.force".to_owned()],
            provenance: provenance.clone(),
            fingerprint: String::new(),
        };
        action.fingerprint = materialized_definition_fingerprint(&action).unwrap();

        let support_provenance = RulesetDefinitionProvenance {
            definition_id: "catalog.damage.force".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: RulesetSourceLocation {
                module: "catalogs/replay.rs".to_owned(),
                declaration: "force".to_owned(),
            },
        };
        let mut support = MaterializedRulesetDefinition {
            id: "catalog.damage.force".to_owned(),
            kind: MaterializedRulesetDefinitionKind::Support,
            visibility: MaterializedRulesetVisibility::Exported,
            extension_policy: RulesetExtensionPolicy::Sealed,
            semantic: json!({"catalog": "damageType", "id": "force"}),
            presentation: json!({"label": "Force"}),
            references: Vec::new(),
            provenance: support_provenance.clone(),
            fingerprint: String::new(),
        };
        support.fingerprint = materialized_definition_fingerprint(&support).unwrap();

        let operations = vec![
            VersionedRulesetRequirement {
                id: "operation.damage".to_owned(),
                version: 1,
            },
            VersionedRulesetRequirement {
                id: "operation.openReaction".to_owned(),
                version: 1,
            },
        ];
        let capabilities = vec![
            VersionedRulesetRequirement {
                id: "capability.random".to_owned(),
                version: 1,
            },
            VersionedRulesetRequirement {
                id: "capability.reactions".to_owned(),
                version: 1,
            },
            VersionedRulesetRequirement {
                id: "capability.vitality".to_owned(),
                version: 1,
            },
        ];
        let package_identity = "replay.test@1.0.0".to_owned();
        let prepared = PreparedRulesetCompilation {
            schema: RulesetArtifactSchema {
                identity: PREPARED_RULESET_IDENTITY.to_owned(),
                major: RULESET_ARTIFACT_MAJOR,
            },
            composition_identity: CompiledRulesetIdentity {
                id: "replay.test".to_owned(),
                version: "1.0.0".to_owned(),
            },
            language_identity: CompiledRulesetIdentity {
                id: "asha-rpg".to_owned(),
                version: "1.0.0".to_owned(),
            },
            source_packages: vec![ResolvedRulesetSourcePackage {
                id: "replay.test".to_owned(),
                version: "1.0.0".to_owned(),
                source_fingerprint: "fnv1a64:1111111111111111".to_owned(),
            }],
            dependency_lock: Vec::new(),
            required_operations: operations,
            required_capabilities: capabilities,
            exported_roots: vec![
                "action.reactive".to_owned(),
                "catalog.damage.force".to_owned(),
            ],
            materialized_definitions: vec![action, support],
            compiled_policy_bindings: Vec::new(),
            definition_provenance: vec![provenance, support_provenance],
            definition_commitments: Vec::new(),
            relationships: vec![
                RulesetRelationshipProvenance {
                    kind: RulesetRelationshipKind::Exports,
                    source: package_identity.clone(),
                    target: "action.reactive".to_owned(),
                    order: 0,
                },
                RulesetRelationshipProvenance {
                    kind: RulesetRelationshipKind::Exports,
                    source: package_identity,
                    target: "catalog.damage.force".to_owned(),
                    order: 1,
                },
            ],
            derivation_provenance: Vec::new(),
            overlay_provenance: Vec::new(),
        };
        let bundle = compile_prepared_ruleset(prepared).expect("prepared replay artifact compiles");
        let actor = RpgEntityState::new("hero", Team::Ally, GridPosition { x: 0, y: 0 }, 20);
        let target = RpgEntityState::new("guardian", Team::Enemy, GridPosition { x: 1, y: 0 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(target);
        RpgAuthoritySession::from_compiled_ruleset(bundle, state)
    }

    fn command() -> RpgAuthorityCommand {
        RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
            },
            random_values: Vec::new(),
        }
    }

    fn reaction_command() -> RpgReactionCommand {
        RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2],
        }
    }
}
