use std::fmt;

use rpg_compiler::load_compiled_play_bundle;
use rpg_core::{
    ActiveRpgEffect, ActiveRpgModifier, BoundedValue, GridPosition, RpgCapabilityState,
    RpgEntityState, RpgResolutionReceipt, StateFingerprint, Team,
};
use rpg_ir::{
    CompiledEffectDefinition, CompiledPlayBundleArtifact, ContentPackDependencyLockEntry,
    PlayBundleArtifactSchema, PlayBundleFingerprints, ResolvedContentPack, RpgVersionedIdentity,
    VersionedRpgRequirement,
};
use serde::{Deserialize, Serialize};

use crate::semantic_session::{
    PendingForcedMovementTransaction, PendingTransaction, PreparedAreaCommand, RpgAuthorityCommand,
    RpgAuthoritySession, RpgCommandOutcome, RpgForcedMovementCommand, RpgPendingForcedMovement,
    RpgPendingReaction, RpgPendingTurnSave, RpgReactionCommand, RpgTurnControlCommand,
};
use crate::{
    encounter::{validate_derived_state, validate_restored_encounter},
    RpgEncounterLogEntry, RpgRandomSourceBinding, RpgScenario, RpgTurnState,
};

pub const RPG_CHECKPOINT_SCHEMA_ID: &str = "asha.rpg.session.checkpoint";
pub const RPG_REPLAY_ENTRY_SCHEMA_ID: &str = "asha.rpg.session.replay-entry";
pub const RPG_CHECKPOINT_SCHEMA_VERSION: u32 = 13;
pub const RPG_REPLAY_ENTRY_SCHEMA_VERSION: u32 = 14;
pub const RPG_EVENT_SCHEMA_VERSION: u32 = 12;

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
    pub operations: Vec<VersionedRpgRequirement>,
    pub capabilities: Vec<VersionedRpgRequirement>,
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
    pub artifact_schema: PlayBundleArtifactSchema,
    pub artifact_id: String,
    pub play_bundle: RpgVersionedIdentity,
    pub ruleset: RpgVersionedIdentity,
    pub content_packs: Vec<ResolvedContentPack>,
    pub dependency_lock: Vec<ContentPackDependencyLockEntry>,
    pub fingerprints: PlayBundleFingerprints,
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
pub struct RpgPortableEffect {
    pub instance_id: String,
    pub definition_id: String,
    pub definition_version: u32,
    pub source_entity_id: String,
    pub stacking_id: String,
    pub stacking: rpg_core::RpgEffectStackingPolicy,
    pub rank: i32,
    pub remaining_count: u32,
    pub application_revision: u64,
    pub duration_anchor: rpg_core::RpgEffectDurationAnchor,
    pub tenure: rpg_core::RpgEffectTenure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableEntityState {
    pub id: String,
    pub team: Team,
    pub position: GridPosition,
    pub class_definition_id: Option<String>,
    pub character_feature_ids: Vec<String>,
    pub vitality: BoundedValue,
    pub stats: Vec<RpgPortableNamedInteger>,
    pub defenses: Vec<RpgPortableNamedInteger>,
    pub resources: Vec<RpgPortableNamedBoundedValue>,
    pub modifiers: Vec<RpgPortableModifier>,
    pub effects: Vec<RpgPortableEffect>,
    pub activation_budgets: Vec<RpgPortableNamedInteger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgPortableCapabilityState {
    pub revision: u64,
    pub accepted_activations_this_turn: u32,
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
        intent: Box<rpg_core::RpgIntent>,
        random_values: Vec<u32>,
        pending: Box<RpgPendingReaction>,
    },
    AwaitingForcedMovement {
        expected_revision: u64,
        intent: Box<rpg_core::RpgIntent>,
        random_values: Vec<u32>,
        pending: Box<RpgPendingForcedMovement>,
    },
    AwaitingTurnSave {
        command: RpgTurnControlCommand,
        pending: Box<RpgPendingTurnSave>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgSessionCheckpoint {
    pub schema: RpgPortableSchemaIdentity,
    pub schemas: RpgReplaySchemaVersions,
    pub artifact_binding: RpgReplayArtifactBinding,
    pub artifact: CompiledPlayBundleArtifact,
    pub scenario: RpgScenario,
    pub scenario_fingerprint: StateFingerprint,
    pub state: RpgPortableCapabilityState,
    pub turn: RpgTurnState,
    pub log: Vec<RpgEncounterLogEntry>,
    pub accepted_random_position: u64,
    pub phase: RpgCheckpointPhase,
    pub state_hash: StateFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum RpgReplayPhase {
    Ready,
    AwaitingReaction {
        reaction_id: String,
    },
    AwaitingForcedMovement {
        source_id: String,
        moved_participant_id: String,
        operation_path: String,
    },
    AwaitingTurnSave {
        actor_id: String,
        effect_instance_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgReplayBoundary {
    pub revision: u64,
    pub accepted_random_position: u64,
    pub scenario_fingerprint: StateFingerprint,
    pub random_source: RpgRandomSourceBinding,
    pub turn: RpgTurnState,
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
    Submit {
        command: RpgAuthorityCommand,
    },
    React {
        command: RpgReactionCommand,
    },
    ForcedMovement {
        command: RpgForcedMovementCommand,
        additional_random_values: Vec<u32>,
    },
    TurnControl {
        command: RpgTurnControlCommand,
    },
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
                "portable checkpoints require a session created from a compiled PlayBundle",
            )
        })?;
        let state = portable_state(&self.state, &self.encounter.effect_definitions);
        let phase = checkpoint_phase(self);
        let accepted_random_position = self.accepted_random_values;
        let state_hash = session_state_hash(
            &self.encounter.scenario,
            &state,
            &self.encounter.turn,
            &self.encounter.log,
            accepted_random_position,
            &phase,
        )?;
        let scenario_fingerprint = scenario_fingerprint(&self.encounter.scenario)?;
        Ok(RpgSessionCheckpoint {
            schema: checkpoint_schema(),
            schemas: replay_versions(&artifact),
            artifact_binding: artifact_binding(&artifact),
            artifact,
            scenario: self.encounter.scenario.clone(),
            scenario_fingerprint,
            state,
            turn: self.encounter.turn.clone(),
            log: self.encounter.log.clone(),
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
        let bundle = load_compiled_play_bundle(checkpoint.artifact.clone()).map_err(|failure| {
            RpgReplayFailure {
                diagnostics: failure
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        let code =
                            checkpoint_artifact_mismatch_code(&diagnostic.path, &diagnostic.code);
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
        validate_portable_effect_tenure(&checkpoint.state, bundle.effects())?;
        let state = restore_state(&checkpoint.state)?;
        let derived_diagnostics = validate_derived_state(&bundle, &state);
        if !derived_diagnostics.is_empty() {
            return Err(RpgReplayFailure {
                diagnostics: derived_diagnostics
                    .into_iter()
                    .map(|diagnostic| RpgReplayDiagnostic {
                        code: diagnostic.code,
                        path: diagnostic.path,
                        message: diagnostic.message,
                        expected: None,
                        actual: None,
                    })
                    .collect(),
            });
        }
        let actual_scenario_fingerprint = scenario_fingerprint(&checkpoint.scenario)?;
        if checkpoint.scenario_fingerprint != actual_scenario_fingerprint {
            return Err(replay_mismatch(
                "RPG_CHECKPOINT_SETUP_FINGERPRINT_MISMATCH",
                "$.scenarioFingerprint",
                &checkpoint.scenario_fingerprint,
                &actual_scenario_fingerprint,
            ));
        }
        let actual_hash = session_state_hash(
            &checkpoint.scenario,
            &checkpoint.state,
            &checkpoint.turn,
            &checkpoint.log,
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
        let mut restored =
            Self::from_scenario(bundle, checkpoint.scenario.clone()).map_err(|failure| {
                RpgReplayFailure {
                    diagnostics: failure
                        .diagnostics
                        .into_iter()
                        .map(|diagnostic| RpgReplayDiagnostic {
                            code: "RPG_CHECKPOINT_SETUP_INVALID".to_owned(),
                            path: diagnostic.path,
                            message: format!("{}: {}", diagnostic.code, diagnostic.message),
                            expected: None,
                            actual: None,
                        })
                        .collect(),
                }
            })?;
        restored.state = state;
        restored.encounter.turn = checkpoint.turn;
        restored.encounter.log = checkpoint.log;
        restored.accepted_random_values = checkpoint.accepted_random_position;
        let encounter_diagnostics =
            validate_restored_encounter(&restored.encounter, &restored.state, &restored.rules);
        if !encounter_diagnostics.is_empty() {
            return Err(RpgReplayFailure {
                diagnostics: encounter_diagnostics
                    .into_iter()
                    .map(|diagnostic| RpgReplayDiagnostic {
                        code: diagnostic.code,
                        path: diagnostic.path,
                        message: diagnostic.message,
                        expected: None,
                        actual: None,
                    })
                    .collect(),
            });
        }
        let (pending_reaction, pending_forced_movement, pending_turn_save) =
            restore_phase(&checkpoint.phase, &restored)?;
        restored.pending = pending_reaction;
        restored.pending_forced_movement = pending_forced_movement;
        restored.pending_turn_save = pending_turn_save;
        restored.rebind_pending_session_identity();
        Ok(restored)
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

    pub(crate) fn submit_recorded(
        &mut self,
        command: RpgAuthorityCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::Submit { command })
    }

    pub(crate) fn submit_prepared_area_recorded(
        &mut self,
        command: RpgAuthorityCommand,
        prepared: PreparedAreaCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        let artifact = self.artifact.as_ref().ok_or_else(|| {
            replay_failure(
                "RPG_REPLAY_ARTIFACT_REQUIRED",
                "$.artifact",
                "recording requires a session created from a compiled PlayBundle",
            )
        })?;
        let schemas = replay_versions(artifact);
        let before = replay_boundary(self)?;
        let operation = RpgReplayOperation::Submit {
            command: command.clone(),
        };
        let outcome = self.submit_with_prepared_area(command, Some(prepared));
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

    pub(crate) fn react_recorded(
        &mut self,
        command: RpgReactionCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::React { command })
    }

    pub fn resolve_forced_movement_recorded(
        &mut self,
        command: RpgForcedMovementCommand,
        additional_random_values: Vec<u32>,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::ForcedMovement {
            command,
            additional_random_values,
        })
    }

    pub(crate) fn record_turn_control(
        &mut self,
        command: RpgTurnControlCommand,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        self.record_operation(RpgReplayOperation::TurnControl { command })
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
        let state = portable_state(&self.state, &self.encounter.effect_definitions);
        let phase = checkpoint_phase(self);
        session_state_hash(
            &self.encounter.scenario,
            &state,
            &self.encounter.turn,
            &self.encounter.log,
            self.accepted_random_values,
            &phase,
        )
    }

    fn record_operation(
        &mut self,
        operation: RpgReplayOperation,
    ) -> Result<(RpgCommandOutcome, RpgReplayEntry), RpgReplayFailure> {
        let artifact = self.artifact.as_ref().ok_or_else(|| {
            replay_failure(
                "RPG_REPLAY_ARTIFACT_REQUIRED",
                "$.artifact",
                "recording requires a session created from a compiled PlayBundle",
            )
        })?;
        let schemas = replay_versions(artifact);
        let before = replay_boundary(self)?;
        let outcome = match &operation {
            RpgReplayOperation::Submit { command } => self.submit(command.clone()),
            RpgReplayOperation::React { command } => self.react(command.clone()),
            RpgReplayOperation::ForcedMovement {
                command,
                additional_random_values,
            } => self.resolve_forced_movement(command.clone(), additional_random_values.clone()),
            RpgReplayOperation::TurnControl { command } => self.control(command.clone()),
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
            RpgReplayOperation::React { command } => {
                let mut rebound = command.clone();
                if let Some(pending) = self.pending_reaction() {
                    if pending.request.movement.is_some() {
                        rebound.reaction_id = pending.request.reaction_id.clone();
                    }
                }
                self.react(rebound)
            }
            RpgReplayOperation::ForcedMovement {
                command,
                additional_random_values,
            } => {
                let mut rebound = command.clone();
                rebound.option.session_binding_id = self.session_binding_id().to_owned();
                self.resolve_forced_movement(rebound, additional_random_values.clone())
            }
            RpgReplayOperation::TurnControl { command } => self.control(command.clone()),
        };
        compare_outcome(&expected.outcome, &outcome, &format!("{base_path}.outcome"))?;
        let after = replay_boundary(self)?;
        compare_boundary(&expected.after, &after, &format!("{base_path}.after"))?;
        Ok(())
    }
}

pub fn classify_checkpoint_artifact(
    checkpoint: &RpgSessionCheckpoint,
    candidate: &CompiledPlayBundleArtifact,
) -> RpgArtifactCompatibilityReport {
    let historical = &checkpoint.artifact;
    let mut diagnostics = Vec::new();
    if historical.content_packs != candidate.content_packs {
        diagnostics.push(compatibility_diagnostic(
            "RPG_REPLAY_PACKAGE_SET_CHANGED",
            "$.artifact.contentPacks",
            "exact content pack identities, versions, or fingerprints changed",
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

fn replay_versions(artifact: &CompiledPlayBundleArtifact) -> RpgReplaySchemaVersions {
    RpgReplaySchemaVersions {
        checkpoint: RPG_CHECKPOINT_SCHEMA_VERSION,
        replay_entry: RPG_REPLAY_ENTRY_SCHEMA_VERSION,
        event: RPG_EVENT_SCHEMA_VERSION,
        operations: artifact.content_requirements.operations.clone(),
        capabilities: artifact.content_requirements.capabilities.clone(),
    }
}

fn artifact_binding(artifact: &CompiledPlayBundleArtifact) -> RpgReplayArtifactBinding {
    RpgReplayArtifactBinding {
        artifact_schema: artifact.artifact_schema.clone(),
        artifact_id: artifact.artifact_id.clone(),
        play_bundle: artifact.play_bundle_identity.clone(),
        ruleset: artifact.ruleset.identity.clone(),
        content_packs: artifact.content_packs.clone(),
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
    artifact: &CompiledPlayBundleArtifact,
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
    if expected.play_bundle != actual.play_bundle {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_PLAY_BUNDLE_MISMATCH",
            "$.artifact.playBundle",
            &expected.play_bundle,
            &actual.play_bundle,
        ));
    }
    if expected.ruleset != actual.ruleset {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_RULESET_MISMATCH",
            "$.artifact.ruleset",
            &expected.ruleset,
            &actual.ruleset,
        ));
    }
    if expected.content_packs != actual.content_packs {
        return Err(replay_mismatch(
            "RPG_CHECKPOINT_CONTENT_PACK_MISMATCH",
            "$.artifact.contentPacks",
            &expected.content_packs,
            &actual.content_packs,
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

fn portable_state(
    state: &RpgCapabilityState,
    effect_definitions: &std::collections::BTreeMap<String, CompiledEffectDefinition>,
) -> RpgPortableCapabilityState {
    RpgPortableCapabilityState {
        revision: state.revision(),
        accepted_activations_this_turn: state.accepted_activations_this_turn(),
        entities: state
            .entities()
            .map(|entity| RpgPortableEntityState {
                id: entity.id().to_owned(),
                team: entity.team().clone(),
                position: entity.position(),
                class_definition_id: entity.class_definition_id().map(str::to_owned),
                character_feature_ids: entity.character_feature_ids().to_vec(),
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
                effects: entity
                    .effects()
                    .map(|effect| RpgPortableEffect {
                        instance_id: effect.instance_id().to_owned(),
                        definition_id: effect.definition_id().to_owned(),
                        definition_version: effect.definition_version(),
                        source_entity_id: effect.source_entity_id().to_owned(),
                        stacking_id: effect.stacking_id().to_owned(),
                        stacking: effect.stacking(),
                        rank: effect.rank(),
                        remaining_count: effect.remaining_count(),
                        application_revision: effect.application_revision(),
                        duration_anchor: effect.duration_anchor(),
                        tenure: effect_definitions
                            .get(effect.definition_id())
                            .map(|definition| definition.tenure)
                            .unwrap_or_else(|| effect.tenure()),
                    })
                    .collect(),
                activation_budgets: entity
                    .activation_budgets()
                    .map(|(id, value)| RpgPortableNamedInteger {
                        id: id.to_owned(),
                        value,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn validate_portable_effect_tenure(
    state: &RpgPortableCapabilityState,
    effect_definitions: &[CompiledEffectDefinition],
) -> Result<(), RpgReplayFailure> {
    for (entity_index, entity) in state.entities.iter().enumerate() {
        for (effect_index, effect) in entity.effects.iter().enumerate() {
            let Some(definition) = effect_definitions.iter().find(|definition| {
                definition.definition_id == effect.definition_id
                    && definition.definition_version == effect.definition_version
            }) else {
                continue;
            };
            if effect.tenure != definition.tenure
                || effect.duration_anchor != definition.duration_anchor
            {
                return Err(replay_failure(
                    "RPG_CHECKPOINT_EFFECT_STATE_MISMATCH",
                    format!("$.state.entities[{entity_index}].effects[{effect_index}].tenure"),
                    format!(
                        "effect {} tenure does not match the compiled definition",
                        effect.instance_id
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn restore_state(
    state: &RpgPortableCapabilityState,
) -> Result<RpgCapabilityState, RpgReplayFailure> {
    let mut entities = Vec::with_capacity(state.entities.len());
    for (entity_index, source) in state.entities.iter().enumerate() {
        let path = format!("$.state.entities[{entity_index}]");
        let mut entity = RpgEntityState::restore(
            source.id.clone(),
            source.team.clone(),
            source.position,
            source.vitality,
        )
        .map_err(|error| state_restore_failure(&path, error))?;
        entity
            .restore_character_selection(
                source.class_definition_id.clone(),
                source.character_feature_ids.clone(),
            )
            .map_err(|error| {
                state_restore_failure(&format!("{path}.characterFeatureIds"), error)
            })?;
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
                    )
                    .map_err(|error| state_restore_failure(&format!("{path}.modifiers"), error))?,
                )
                .map_err(|error| state_restore_failure(&format!("{path}.modifiers"), error))?;
        }
        for effect in &source.effects {
            if effect.application_revision > state.revision {
                return Err(state_restore_failure(
                    &format!("{path}.effects"),
                    rpg_core::RpgStateRestoreError::ValueOutOfBounds,
                ));
            }
            entity
                .restore_effect(
                    ActiveRpgEffect::restore(
                        effect.instance_id.clone(),
                        effect.definition_id.clone(),
                        effect.definition_version,
                        effect.source_entity_id.clone(),
                        effect.stacking_id.clone(),
                        effect.stacking,
                        effect.rank,
                        effect.remaining_count,
                        effect.application_revision,
                        effect.duration_anchor,
                    )
                    .map_err(|error| state_restore_failure(&format!("{path}.effects"), error))?,
                )
                .map_err(|error| state_restore_failure(&format!("{path}.effects"), error))?;
        }
        for budget in &source.activation_budgets {
            entity
                .restore_activation_budget(budget.id.clone(), budget.value)
                .map_err(|error| {
                    state_restore_failure(&format!("{path}.activationBudgets"), error)
                })?;
        }
        entities.push(entity);
    }
    RpgCapabilityState::restore_with_activation_count(
        state.revision,
        entities,
        state.accepted_activations_this_turn,
    )
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
    match (
        &session.pending,
        &session.pending_forced_movement,
        &session.pending_turn_save,
    ) {
        (None, None, None) => RpgCheckpointPhase::Ready,
        (Some(transaction), None, None) => {
            let mut pending = transaction.pending.clone();
            if let Some(movement) = pending.request.movement.as_mut() {
                movement.session_binding_id.clear();
                pending.request.reaction_id.clear();
            }
            RpgCheckpointPhase::AwaitingReaction {
                expected_revision: transaction.expected_revision,
                intent: Box::new(transaction.portable_intent.clone()),
                random_values: transaction.random_values.clone(),
                pending: Box::new(pending),
            }
        }
        (None, Some(transaction), None) => {
            let mut pending = transaction.pending.clone();
            for option in &mut pending.options {
                option.session_binding_id.clear();
            }
            RpgCheckpointPhase::AwaitingForcedMovement {
                expected_revision: session.state.revision(),
                intent: Box::new(transaction.portable_intent.clone()),
                random_values: transaction.random_values.clone(),
                pending: Box::new(pending),
            }
        }
        (None, None, Some(pending)) => RpgCheckpointPhase::AwaitingTurnSave {
            command: RpgTurnControlCommand {
                expected_revision: pending.expected_revision,
                actor_id: pending.actor_id.clone(),
                control: pending.control.clone(),
                random_values: Vec::new(),
            },
            pending: Box::new(pending.clone()),
        },
        _ => {
            unreachable!("reaction, forced-movement, and turn-save phases are mutually exclusive")
        }
    }
}

type RestoredPendingPhases = (
    Option<PendingTransaction>,
    Option<PendingForcedMovementTransaction>,
    Option<RpgPendingTurnSave>,
);

fn restore_phase(
    phase: &RpgCheckpointPhase,
    baseline: &RpgAuthoritySession,
) -> Result<RestoredPendingPhases, RpgReplayFailure> {
    match phase {
        RpgCheckpointPhase::Ready => Ok((None, None, None)),
        RpgCheckpointPhase::AwaitingReaction {
            expected_revision,
            intent,
            random_values,
            pending,
        } => {
            let mut proof = baseline.clone();
            proof.pending = None;
            let outcome = proof.submit(RpgAuthorityCommand {
                expected_revision: *expected_revision,
                intent: intent.as_ref().clone(),
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
            let mut expected = pending.as_ref().clone();
            if let Some(movement) = expected.request.movement.as_mut() {
                movement.session_binding_id = proof.session_binding_id().to_owned();
                expected.request.reaction_id = actual.request.reaction_id.clone();
            }
            if expected.random_evidence != actual.random_evidence
                || expected.random_attempted != actual.random_attempted
            {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_EVIDENCE_MISMATCH",
                    "$.phase.pending.randomEvidence",
                    &(&expected.random_evidence, expected.random_attempted),
                    &(&actual.random_evidence, actual.random_attempted),
                ));
            }
            if expected != *actual {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase.pending",
                    &expected,
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
                .map(|pending| (Some(pending), None, None))
        }
        RpgCheckpointPhase::AwaitingForcedMovement {
            expected_revision,
            intent,
            random_values,
            pending,
        } => {
            let mut proof = baseline.clone();
            proof.pending = None;
            proof.pending_forced_movement = None;
            let outcome = proof.submit(RpgAuthorityCommand {
                expected_revision: *expected_revision,
                intent: intent.as_ref().clone(),
                random_values: random_values.clone(),
            });
            let RpgCommandOutcome::AwaitingForcedMovement(actual) = outcome else {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase",
                    pending.as_ref(),
                    &outcome,
                ));
            };
            let mut expected = pending.as_ref().clone();
            for option in &mut expected.options {
                option.session_binding_id = proof.session_binding_id().to_owned();
            }
            if expected != actual {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase.pending",
                    &expected,
                    &actual,
                ));
            }
            proof
                .pending_forced_movement
                .take()
                .ok_or_else(|| {
                    replay_failure(
                        "RPG_CHECKPOINT_PHASE_MISMATCH",
                        "$.phase.pending",
                        "Rust authority did not retain the verified pending forced movement",
                    )
                })
                .map(|pending| (None, Some(pending), None))
        }
        RpgCheckpointPhase::AwaitingTurnSave { command, pending } => {
            let mut proof = baseline.clone();
            proof.pending = None;
            proof.pending_turn_save = None;
            let outcome = proof.control(command.clone());
            let RpgCommandOutcome::AwaitingTurnSave(actual) = outcome else {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase",
                    pending.as_ref(),
                    &outcome,
                ));
            };
            if pending.as_ref() != &actual {
                return Err(replay_mismatch(
                    "RPG_CHECKPOINT_PHASE_MISMATCH",
                    "$.phase.pending",
                    pending.as_ref(),
                    &actual,
                ));
            }
            proof
                .pending_turn_save
                .take()
                .ok_or_else(|| {
                    replay_failure(
                        "RPG_CHECKPOINT_PHASE_MISMATCH",
                        "$.phase.pending",
                        "Rust authority did not retain the verified pending turn save",
                    )
                })
                .map(|pending| (None, None, Some(pending)))
        }
    }
}

fn replay_boundary(session: &RpgAuthoritySession) -> Result<RpgReplayBoundary, RpgReplayFailure> {
    let phase = match (
        session.pending_reaction(),
        session.pending_forced_movement(),
        &session.pending_turn_save,
    ) {
        (None, None, None) => RpgReplayPhase::Ready,
        (Some(pending), None, None) => RpgReplayPhase::AwaitingReaction {
            reaction_id: pending
                .request
                .movement
                .as_ref()
                .map(|movement| {
                    format!(
                        "movement:{}:{}:{}:{}",
                        movement.authority_revision,
                        pending.request.target_id,
                        movement.source_definition_id,
                        movement.registration_id
                    )
                })
                .unwrap_or_else(|| pending.request.reaction_id.clone()),
        },
        (None, Some(pending), None) => RpgReplayPhase::AwaitingForcedMovement {
            source_id: pending.request.source_id.clone(),
            moved_participant_id: pending.request.moved_participant_id.clone(),
            operation_path: pending.request.operation_path.clone(),
        },
        (None, None, Some(pending)) => RpgReplayPhase::AwaitingTurnSave {
            actor_id: pending.actor_id.clone(),
            effect_instance_ids: pending
                .candidates
                .iter()
                .map(|candidate| candidate.instance_id.clone())
                .collect(),
        },
        _ => {
            unreachable!("reaction, forced-movement, and turn-save phases are mutually exclusive")
        }
    };
    Ok(RpgReplayBoundary {
        revision: session.state.revision(),
        accepted_random_position: session.accepted_random_values,
        scenario_fingerprint: scenario_fingerprint(&session.encounter.scenario)?,
        random_source: session.encounter.scenario.random_source.clone(),
        turn: session.encounter.turn.clone(),
        phase,
        state_hash: session.state_hash()?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionHashInput<'a> {
    scenario: &'a RpgScenario,
    state: &'a RpgPortableCapabilityState,
    turn: &'a RpgTurnState,
    log: &'a [RpgEncounterLogEntry],
    accepted_random_position: u64,
    phase: &'a RpgCheckpointPhase,
}

fn session_state_hash(
    scenario: &RpgScenario,
    state: &RpgPortableCapabilityState,
    turn: &RpgTurnState,
    log: &[RpgEncounterLogEntry],
    accepted_random_position: u64,
    phase: &RpgCheckpointPhase,
) -> Result<StateFingerprint, RpgReplayFailure> {
    let bytes = serde_json::to_vec(&SessionHashInput {
        scenario,
        state,
        turn,
        log,
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
        algorithm: "fnv1a64.rpg-session.v3".to_owned(),
        value: format!("{hash:016x}"),
    })
}

pub(crate) fn scenario_fingerprint(
    scenario: &RpgScenario,
) -> Result<StateFingerprint, RpgReplayFailure> {
    let bytes = serde_json::to_vec(scenario).map_err(|error| {
        replay_failure(
            "RPG_CHECKPOINT_SETUP_HASH_FAILED",
            "$.scenario",
            format!("canonical scenario hashing failed: {error}"),
        )
    })?;
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(StateFingerprint {
        algorithm: "fnv1a64.rpg-scenario.v1".to_owned(),
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
    if expected.scenario_fingerprint != actual.scenario_fingerprint {
        return Err(replay_mismatch(
            "RPG_REPLAY_SETUP_MISMATCH",
            format!("{path}.scenarioFingerprint"),
            &expected.scenario_fingerprint,
            &actual.scenario_fingerprint,
        ));
    }
    if expected.random_source != actual.random_source {
        return Err(replay_mismatch(
            "RPG_REPLAY_RANDOM_SOURCE_MISMATCH",
            format!("{path}.randomSource"),
            &expected.random_source,
            &actual.random_source,
        ));
    }
    if expected.turn != actual.turn {
        return Err(replay_mismatch(
            "RPG_REPLAY_TURN_MISMATCH",
            format!("{path}.turn"),
            &expected.turn,
            &actual.turn,
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
            RpgCommandOutcome::ControlAccepted(expected),
            RpgCommandOutcome::ControlAccepted(actual),
        ) => {
            if expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_CONTROL_OUTCOME_MISMATCH",
                    path,
                    expected,
                    actual,
                ));
            }
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
            let mut portable_expected = expected.clone();
            if let (Some(expected_movement), Some(actual_movement)) = (
                portable_expected.request.movement.as_mut(),
                actual.request.movement.as_ref(),
            ) {
                expected_movement.session_binding_id = actual_movement.session_binding_id.clone();
                portable_expected.request.reaction_id = actual.request.reaction_id.clone();
            }
            if &portable_expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_PHASE_MISMATCH",
                    path,
                    &portable_expected,
                    actual,
                ));
            }
        }
        (
            RpgCommandOutcome::AwaitingTurnSave(expected),
            RpgCommandOutcome::AwaitingTurnSave(actual),
        ) => {
            if expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_PHASE_MISMATCH",
                    path,
                    expected,
                    actual,
                ));
            }
        }
        (
            RpgCommandOutcome::AwaitingForcedMovement(expected),
            RpgCommandOutcome::AwaitingForcedMovement(actual),
        ) => {
            if expected.random_evidence != actual.random_evidence {
                return Err(replay_mismatch(
                    "RPG_REPLAY_EVIDENCE_MISMATCH",
                    format!("{path}.result.randomEvidence"),
                    &expected.random_evidence,
                    &actual.random_evidence,
                ));
            }
            let mut portable_expected = expected.clone();
            for (expected_option, actual_option) in
                portable_expected.options.iter_mut().zip(&actual.options)
            {
                expected_option.session_binding_id = actual_option.session_binding_id.clone();
            }
            if &portable_expected != actual {
                return Err(replay_mismatch(
                    "RPG_REPLAY_PHASE_MISMATCH",
                    path,
                    &portable_expected,
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
    if path.contains("contentPacks") {
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
    use std::collections::BTreeSet;

    use rpg_compiler::{
        compile_prepared_play_bundle, materialized_definition_fingerprint, CompiledPlayBundle,
    };
    use rpg_core::{GridPosition, RpgDomainEvent, RpgIntent, RpgIntentCellTarget, Team};
    use rpg_ir::{
        ContentDefinitionProvenance, ContentExtensionPolicy, ContentPackDependencyLockEntry,
        ContentPackDependencyRelationship, ContentPackRequirements, ContentRelationshipKind,
        ContentRelationshipProvenance, ContentSourceLocation, MaterializedContentDefinition,
        MaterializedContentDefinitionKind, MaterializedContentVisibility, PlayBundleArtifactSchema,
        PreparedPlayBundle, ResolvedContentPack, RpgVersionedIdentity, Ruleset,
        RulesetActionEconomyModel, RulesetActivationBudget, RulesetActivationBudgetResetBoundary,
        RulesetActivationTiming, RulesetModels, RulesetNumericDomain, RulesetProvisions,
        RulesetSchema, RulesetValueContract, RulesetValueKind, RulesetValueSource,
        VersionedRpgRequirement, PLAY_BUNDLE_ARTIFACT_MAJOR, PREPARED_PLAY_BUNDLE_IDENTITY,
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
        assert_eq!(replayed.accepted_random_values(), 3);

        let RpgCommandOutcome::Accepted(receipt) = recorded_outcome else {
            panic!("reaction must commit");
        };
        assert_eq!(receipt.random_consumed, 3);
        assert_eq!(
            receipt
                .random_evidence
                .iter()
                .flat_map(|evidence| evidence.values.iter().copied())
                .collect::<Vec<_>>(),
            vec![12, 2, 2]
        );
        assert!(receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::DamagePacketApplied {
                bounded_vitality_delta: 1,
                ..
            }
        )));
        assert_eq!(reaction_entry.after.revision, 1);
        assert_eq!(reaction_entry.after.phase, RpgReplayPhase::Ready);
    }

    #[test]
    fn selected_cell_movement_replays_with_the_exact_board_binding() {
        let mut recorded = artifact_session();
        let initial = recorded.checkpoint().expect("initial checkpoint");
        let movement = RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.move".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["cell-2-0".to_owned()],
                cell_targets: vec![RpgIntentCellTarget {
                    id: "cell-2-0".to_owned(),
                    position: GridPosition { x: 2, y: 0 },
                    route_cell_ids: Vec::new(),
                    movement_cost: 0,
                }],
                item_binding: None,
            },
            random_values: Vec::new(),
        };
        let (outcome, entry) = recorded
            .submit_recorded(movement)
            .expect("movement records");
        assert!(matches!(outcome, RpgCommandOutcome::Accepted(_)));
        assert_eq!(
            recorded.state().entity("hero").unwrap().position(),
            GridPosition { x: 2, y: 0 }
        );

        let replayed = RpgAuthoritySession::replay(initial.clone(), std::slice::from_ref(&entry))
            .expect("movement replay matches");
        assert_eq!(
            replayed.state_hash().unwrap(),
            recorded.state_hash().unwrap()
        );

        let mut tampered = entry;
        let RpgReplayOperation::Submit { command } = &mut tampered.operation else {
            panic!("movement submit expected");
        };
        command.intent.cell_targets[0].position = GridPosition { x: 3, y: 0 };
        let failure = RpgAuthoritySession::replay(initial, &[tampered]).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "RPG_REPLAY_PHASE_MISMATCH");
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

        let mut setup_mismatch = initial.clone();
        setup_mismatch.scenario.board.width = setup_mismatch.scenario.board.width.saturating_add(1);
        let setup_error = target.replace_from_checkpoint(setup_mismatch).unwrap_err();
        assert_eq!(
            setup_error.diagnostics[0].code,
            "RPG_CHECKPOINT_SETUP_FINGERPRINT_MISMATCH"
        );
        assert_eq!(target.state_hash().unwrap(), target_hash);

        let mut lock_mismatch = initial.clone();
        lock_mismatch
            .artifact
            .dependency_lock
            .push(ContentPackDependencyLockEntry {
                requester: "replay.test@1.0.0".to_owned(),
                package_id: "support.test".to_owned(),
                requested_version: "^1.0.0".to_owned(),
                resolved_version: "1.0.0".to_owned(),
                source_fingerprint: "fnv1a64:2222222222222222".to_owned(),
                import_as: "support".to_owned(),
                relationship: ContentPackDependencyRelationship::DependsOn,
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
        package_mismatch.artifact.content_packs[0].version = "9.0.0".to_owned();
        let package_error = target
            .replace_from_checkpoint(package_mismatch)
            .unwrap_err();
        assert_eq!(
            package_error.diagnostics[0].code, "RPG_CHECKPOINT_CONTENT_PACK_MISMATCH",
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

    #[test]
    fn differently_shaped_setups_from_one_artifact_are_independent() {
        let bundle = artifact_bundle();
        let first_setup = standard_setup(&bundle);
        let mut second_setup = standard_setup(&bundle);
        second_setup.board.width = 8;
        second_setup.board.height = 6;
        second_setup.board.cells.push(crate::RpgCellSetup {
            id: "cell.high-ground".to_owned(),
            position: GridPosition { x: 7, y: 5 },
            capabilities: vec![
                crate::RpgCellCapabilitySetup {
                    id: "terrain.traversal".to_owned(),
                    version: 1,
                    definition_id: None,
                    value: crate::RpgCellCapabilityValue::Traversal {
                        passable: true,
                        movement_cost: 2,
                    },
                },
                crate::RpgCellCapabilitySetup {
                    id: "terrain.kind".to_owned(),
                    version: 1,
                    definition_id: Some("catalog.damage.force".to_owned()),
                    value: crate::RpgCellCapabilityValue::Identifier {
                        value_id: "terrain.high-ground".to_owned(),
                    },
                },
            ],
        });
        second_setup.participants.push(crate::RpgParticipantSetup {
            id: "scout".to_owned(),
            label: "Scout".to_owned(),
            team_id: Team::enemy(),
            position: GridPosition { x: 5, y: 2 },
            definition_ids: vec!["action.reactive".to_owned()],
            class_definition_id: None,
            feature_definition_ids: Vec::new(),
            items: Vec::new(),
            equipment: Vec::new(),
            capabilities: vec![
                crate::RpgInitialCapability::Vitality {
                    value: BoundedValue {
                        current: 14,
                        max: 14,
                    },
                },
                crate::RpgInitialCapability::Defense {
                    id: "guard".to_owned(),
                    value: 11,
                },
            ],
        });
        second_setup.turn.initiative_order.push("scout".to_owned());

        let mut first = RpgAuthoritySession::from_scenario(bundle.clone(), first_setup).unwrap();
        let second = RpgAuthoritySession::from_scenario(bundle, second_setup).unwrap();
        assert_eq!(first.encounter_view().participants.len(), 2);
        assert_eq!(second.encounter_view().participants.len(), 3);
        assert_eq!(second.encounter_view().board.cells.len(), 4);
        assert_eq!(
            first.artifact().unwrap().artifact_id,
            second.artifact().unwrap().artifact_id
        );

        assert!(matches!(
            first.submit(command()),
            RpgCommandOutcome::AwaitingReaction(_)
        ));
        assert!(matches!(
            first.react(reaction_command()),
            RpgCommandOutcome::Accepted(_)
        ));
        assert_eq!(first.state().revision(), 1);
        assert_eq!(first.turn().current_actor_id, "guardian");
        assert!(matches!(
            first.submit(RpgAuthorityCommand {
                expected_revision: 1,
                intent: RpgIntent {
                    action_id: "action.reactive".to_owned(),
                    actor_id: "guardian".to_owned(),
                    target_ids: vec!["hero".to_owned()],
                    cell_targets: Vec::new(),
                    item_binding: None,
                },
                random_values: vec![12],
            }),
            RpgCommandOutcome::AwaitingReaction(_)
        ));
        assert!(matches!(
            first.react(RpgReactionCommand {
                expected_revision: 1,
                reaction_id: "reaction.ward".to_owned(),
                option_id: Some("ward".to_owned()),
                additional_random_values: vec![2, 2],
            }),
            RpgCommandOutcome::Accepted(_)
        ));
        assert_eq!(first.state().revision(), 2);
        assert_eq!(first.turn().current_actor_id, "hero");
        assert_eq!(first.turn().round, 2);
        assert_eq!(first.encounter_view().log.len(), 2);
        assert_eq!(second.state().revision(), 0);
        assert_eq!(second.turn().current_actor_id, "hero");
    }

    #[test]
    fn accepted_turns_age_expire_checkpoint_and_replay_modifiers() {
        let mut session = artifact_session();
        let initial = session.checkpoint().unwrap();
        assert_eq!(initial.schemas.event, RPG_EVENT_SCHEMA_VERSION);

        let (pending, first_submit) = session.submit_recorded(command()).unwrap();
        assert!(matches!(pending, RpgCommandOutcome::AwaitingReaction(_)));
        let (accepted, first_reaction) = session
            .react_recorded(reaction_command())
            .expect("first turn records");
        let RpgCommandOutcome::Accepted(first_receipt) = accepted else {
            panic!("first turn must be accepted: {accepted:?}");
        };
        assert!(first_receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::ModifierDurationChanged {
                target_id,
                modifier_id,
                remaining_turns: 1,
                ..
            } if target_id == "hero" && modifier_id == "impeded"
        )));
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .modifier("impeded")
                .unwrap()
                .remaining_turns(),
            1
        );
        let mid_checkpoint = session.checkpoint().unwrap();
        let mid_restored = RpgAuthoritySession::restore_checkpoint(mid_checkpoint).unwrap();
        assert_eq!(
            mid_restored.state_hash().unwrap(),
            session.state_hash().unwrap()
        );

        let guardian_command = RpgAuthorityCommand {
            expected_revision: 1,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "guardian".to_owned(),
                target_ids: vec!["hero".to_owned()],
                cell_targets: Vec::new(),
                item_binding: None,
            },
            random_values: vec![12],
        };
        let (pending, second_submit) = session.submit_recorded(guardian_command).unwrap();
        assert!(matches!(pending, RpgCommandOutcome::AwaitingReaction(_)));
        let (accepted, second_reaction) = session
            .react_recorded(RpgReactionCommand {
                expected_revision: 1,
                reaction_id: "reaction.ward".to_owned(),
                option_id: Some("ward".to_owned()),
                additional_random_values: vec![2, 2],
            })
            .expect("second turn records");
        let RpgCommandOutcome::Accepted(second_receipt) = accepted else {
            panic!("second turn must be accepted: {accepted:?}");
        };
        assert!(second_receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::ModifierExpired {
                target_id,
                modifier_id,
                ..
            } if target_id == "hero" && modifier_id == "impeded"
        )));
        assert!(session
            .state()
            .entity("hero")
            .unwrap()
            .modifier("impeded")
            .is_none());

        let final_checkpoint = session.checkpoint().unwrap();
        let final_restored = RpgAuthoritySession::restore_checkpoint(final_checkpoint).unwrap();
        assert_eq!(
            final_restored.state_hash().unwrap(),
            session.state_hash().unwrap()
        );
        let replayed = RpgAuthoritySession::replay(
            initial,
            &[first_submit, first_reaction, second_submit, second_reaction],
        )
        .expect("modifier aging replays from recorded evidence");
        assert_eq!(
            replayed.state_hash().unwrap(),
            session.state_hash().unwrap()
        );
    }

    #[test]
    fn explicit_turn_controls_are_authoritative_atomic_and_replayable() {
        let mut recorded = artifact_session();
        let initial = recorded.checkpoint().expect("initial checkpoint");
        let initial_hash = recorded.state_hash().expect("initial hash");
        let control = recorded.encounter_view().controls[0].clone();
        assert_eq!(control.control, crate::RpgTurnControl::EndTurn);
        assert!(control.available);

        let rejected = recorded
            .control_recorded(crate::RpgTurnControlProposal {
                expected_revision: 0,
                actor_id: "guardian".to_owned(),
                control: crate::RpgTurnControl::EndTurn,
            })
            .expect("rejected controls are recorded");
        let RpgCommandOutcome::Rejected(rejection) = rejected.0 else {
            panic!("wrong actor must reject");
        };
        assert_eq!(rejection.code, "RPG_TURN_ACTOR_MISMATCH");
        assert_eq!(recorded.state_hash().unwrap(), initial_hash);

        let (first_outcome, first_entry) = recorded
            .control_recorded(crate::RpgTurnControlProposal {
                expected_revision: 0,
                actor_id: "hero".to_owned(),
                control: crate::RpgTurnControl::EndTurn,
            })
            .expect("first control records");
        let RpgCommandOutcome::ControlAccepted(first_receipt) = first_outcome else {
            panic!("end turn must be accepted");
        };
        assert_eq!(first_receipt.state_revision, 1);
        assert!(first_receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::ModifierDurationChanged {
                target_id,
                modifier_id,
                remaining_turns: 1,
                ..
            } if target_id == "hero" && modifier_id == "impeded"
        )));
        assert_eq!(recorded.turn().current_actor_id, "guardian");
        assert_eq!(
            recorded.encounter.log[0].action_id,
            crate::RPG_END_TURN_CONTROL_ID
        );

        let (second_outcome, second_entry) = recorded
            .control_recorded(crate::RpgTurnControlProposal {
                expected_revision: 1,
                actor_id: "guardian".to_owned(),
                control: crate::RpgTurnControl::EndTurn,
            })
            .expect("second control records");
        let RpgCommandOutcome::ControlAccepted(second_receipt) = second_outcome else {
            panic!("second end turn must be accepted");
        };
        assert_eq!(second_receipt.state_revision, 2);
        assert!(second_receipt.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::ModifierExpired {
                target_id,
                modifier_id,
                ..
            } if target_id == "hero" && modifier_id == "impeded"
        )));
        assert_eq!(recorded.turn().current_actor_id, "hero");
        assert_eq!(recorded.turn().round, 2);
        assert_eq!(recorded.turn().turn, 3);

        let replayed = RpgAuthoritySession::replay(initial, &[first_entry, second_entry])
            .expect("turn controls replay through the ordinary authority path");
        assert_eq!(
            replayed.state_hash().unwrap(),
            recorded.state_hash().unwrap()
        );
        assert_eq!(replayed.turn(), recorded.turn());
        assert_eq!(replayed.encounter_view().log, recorded.encounter_view().log);
    }

    #[test]
    fn portable_restore_rejects_modifier_tenure_above_the_runtime_bound() {
        let mut state = artifact_session().checkpoint().unwrap().state;
        let entity_index = state
            .entities
            .iter()
            .position(|entity| !entity.modifiers.is_empty())
            .expect("fixture has an active modifier");
        state.entities[entity_index].modifiers[0].remaining_turns =
            rpg_core::MAXIMUM_RPG_MODIFIER_TURNS + 1;
        let failure = restore_state(&state).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "RPG_CHECKPOINT_STATE_INVALID");
        assert_eq!(
            failure.diagnostics[0].path,
            format!("$.state.entities[{entity_index}].modifiers")
        );
    }

    #[test]
    fn invalid_scenario_reports_all_authority_diagnostics_before_session_exists() {
        let bundle = artifact_bundle();
        let mut invalid = standard_setup(&bundle);
        invalid.play_bundle_id = "different-artifact".to_owned();
        invalid.participants[0].definition_ids = vec!["action.missing".to_owned()];
        invalid.board.cells[0]
            .capabilities
            .push(crate::RpgCellCapabilitySetup {
                id: "terrain.traversal".to_owned(),
                version: 1,
                definition_id: None,
                value: crate::RpgCellCapabilityValue::Traversal {
                    passable: false,
                    movement_cost: 1,
                },
            });
        invalid.participants[1].position = GridPosition { x: 99, y: 99 };
        invalid.participants[1]
            .capabilities
            .push(crate::RpgInitialCapability::Resource {
                id: "focus".to_owned(),
                value: BoundedValue { current: 1, max: 1 },
            });
        invalid.participants[1]
            .capabilities
            .push(crate::RpgInitialCapability::Stat {
                id: "unknown-stat".to_owned(),
                value: 10,
            });
        for capability in &mut invalid.participants[0].capabilities {
            match capability {
                crate::RpgInitialCapability::Vitality { value } => value.current = 0,
                crate::RpgInitialCapability::Modifier {
                    remaining_turns, ..
                } => *remaining_turns = rpg_core::MAXIMUM_RPG_MODIFIER_TURNS + 1,
                crate::RpgInitialCapability::Defense { value, .. } => *value = 101,
                _ => {}
            }
        }
        invalid.turn.initiative_order = vec!["hero".to_owned(), "hero".to_owned()];

        let failure = RpgAuthoritySession::from_scenario(bundle.clone(), invalid).unwrap_err();
        let codes = failure
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("RPG_SCENARIO_PLAY_BUNDLE_MISMATCH"));
        assert!(codes.contains("RPG_SCENARIO_DEFINITION_UNKNOWN"));
        assert!(codes.contains("RPG_SCENARIO_POSITION_OUT_OF_BOUNDS"));
        assert!(codes.contains("RPG_SCENARIO_POSITION_BLOCKED"));
        assert!(codes.contains("RPG_SCENARIO_CAPABILITY_OWNER_INCOMPATIBLE"));
        assert!(codes.contains("RPG_SCENARIO_CONTENT_VALUE_UNKNOWN"));
        assert!(codes.contains("RPG_SCENARIO_RULESET_VALUE_UNKNOWN"));
        assert!(codes.contains("RPG_SCENARIO_RULESET_VALUE_OUT_OF_DOMAIN"));
        assert!(codes.contains("RPG_SCENARIO_MODIFIER_INVALID"));
        assert!(codes.contains("RPG_SCENARIO_CURRENT_ACTOR_INACTIVE"));
        assert!(codes.contains("RPG_SCENARIO_TURN_PARTICIPANT_DUPLICATE"));
        assert!(codes.contains("RPG_SCENARIO_TURN_ORDER_INCOMPLETE"));

        let valid = standard_setup(&bundle);
        let session = RpgAuthoritySession::from_scenario(bundle, valid).unwrap();
        assert_eq!(session.state().revision(), 0);
    }

    #[test]
    fn automatic_source_follows_the_selected_random_branch_and_replays_evidence() {
        let mut session = artifact_session();
        let initial = session.checkpoint().unwrap();
        let binding = session.scenario().random_source.clone();
        let check_request = required_request(session.submit(RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
                cell_targets: Vec::new(),
                item_binding: None,
            },
            random_values: Vec::new(),
        }));
        assert_eq!(
            check_request.kind,
            rpg_core::RpgRandomRequestKind::AttackCheck
        );
        let before_mismatch = session.state_hash().unwrap();
        let mut wrong_binding = binding.clone();
        wrong_binding.source_id = "test.other-source".to_owned();
        let mismatch = session
            .submit_with_random_source_recorded(
                crate::RpgActionProposal {
                    expected_revision: 0,
                    action_id: "action.reactive".to_owned(),
                    actor_id: "hero".to_owned(),
                    target_ids: vec!["guardian".to_owned()],
                    item_binding: None,
                },
                &mut crate::RpgRollTapeSource::new(wrong_binding, Vec::new()),
            )
            .unwrap_err();
        let crate::RpgAutomaticCommandFailure::RandomSource(mismatch) = mismatch else {
            panic!("binding mismatch must be a random source failure");
        };
        assert_eq!(mismatch.code, "RPG_RANDOM_SOURCE_BINDING_MISMATCH");
        assert_eq!(session.state_hash().unwrap(), before_mismatch);
        let mut submit_tape = crate::RpgRollTapeSource::new(
            binding.clone(),
            vec![crate::RpgRollTapeEntry {
                request: check_request,
                values: vec![12],
            }],
        );
        let (pending, submit_entry) = session
            .submit_with_random_source_recorded(
                crate::RpgActionProposal {
                    expected_revision: 0,
                    action_id: "action.reactive".to_owned(),
                    actor_id: "hero".to_owned(),
                    target_ids: vec!["guardian".to_owned()],
                    item_binding: None,
                },
                &mut submit_tape,
            )
            .unwrap();
        assert!(matches!(pending, RpgCommandOutcome::AwaitingReaction(_)));
        submit_tape.require_exhausted().unwrap();

        let branch_request = required_request(session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: Vec::new(),
        }));
        assert_eq!(
            branch_request.kind,
            rpg_core::RpgRandomRequestKind::FormulaDice
        );
        assert_eq!((branch_request.count, branch_request.sides), (2, 6));

        let mut tape = crate::RpgRollTapeSource::new(
            binding,
            vec![crate::RpgRollTapeEntry {
                request: branch_request,
                values: vec![2, 2],
            }],
        );
        let (accepted, reaction_entry) = session
            .react_with_random_source_recorded(
                crate::RpgReactionProposal {
                    expected_revision: 0,
                    reaction_id: "reaction.ward".to_owned(),
                    option_id: Some("ward".to_owned()),
                },
                &mut tape,
            )
            .unwrap();
        assert!(matches!(accepted, RpgCommandOutcome::Accepted(_)));
        tape.require_exhausted().unwrap();
        assert_eq!(tape.consumed_entries(), 1);
        assert_eq!(tape.consumed_values(), 2);
        assert_eq!(session.turn().current_actor_id, "guardian");

        let replayed = RpgAuthoritySession::replay(initial, &[submit_entry, reaction_entry])
            .expect("recorded source evidence replays without regenerating rolls");
        assert_eq!(
            replayed.state_hash().unwrap(),
            session.state_hash().unwrap()
        );
        assert_eq!(replayed.turn(), session.turn());
    }

    #[test]
    fn bounded_roll_tape_classifies_exhaustion_range_unused_and_order_failures() {
        use crate::RpgRandomSource as _;

        let binding = crate::RpgRandomSourceBinding {
            policy_id: "test.policy".to_owned(),
            policy_version: 1,
            source_id: "test.tape".to_owned(),
            source_version: 1,
        };
        let request = rpg_core::RpgRandomRequest {
            kind: rpg_core::RpgRandomRequestKind::FormulaDice,
            count: 2,
            sides: 6,
            path: "$.damage".to_owned(),
            heterogeneous_terms: Vec::new(),
        };
        let exhausted = crate::RpgRollTapeSource::new(binding.clone(), Vec::new())
            .draw(&request)
            .unwrap_err();
        assert_eq!(exhausted.code, "RPG_RANDOM_TAPE_EXHAUSTED");

        let different = rpg_core::RpgRandomRequest {
            path: "$.other".to_owned(),
            ..request.clone()
        };
        let order = crate::RpgRollTapeSource::new(
            binding.clone(),
            vec![crate::RpgRollTapeEntry {
                request: different,
                values: vec![1, 1],
            }],
        )
        .draw(&request)
        .unwrap_err();
        assert_eq!(order.code, "RPG_RANDOM_TAPE_REQUEST_ORDER_MISMATCH");
        assert!(order.expected_request.is_some());
        assert!(order.actual_request.is_some());

        let range = crate::RpgRollTapeSource::new(
            binding.clone(),
            vec![crate::RpgRollTapeEntry {
                request: request.clone(),
                values: vec![1, 7],
            }],
        )
        .draw(&request)
        .unwrap_err();
        assert_eq!(range.code, "RPG_RANDOM_TAPE_VALUE_OUT_OF_RANGE");

        let unused = crate::RpgRollTapeSource::new(
            binding,
            vec![crate::RpgRollTapeEntry {
                request: request.clone(),
                values: vec![1, 2, 3],
            }],
        )
        .draw(&request)
        .unwrap_err();
        assert_eq!(unused.code, "RPG_RANDOM_TAPE_UNUSED_EVIDENCE");
    }

    #[test]
    fn activation_budgets_stage_reactions_reset_exact_boundaries_and_replay() {
        let bundle = activation_reaction_bundle();
        let mut session =
            RpgAuthoritySession::from_scenario(bundle.clone(), standard_setup(&bundle)).unwrap();
        let initial = session.checkpoint().unwrap();

        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .activation_budget("normal"),
            Some(3)
        );
        let (awaiting, submit_entry) = session.submit_recorded(command()).unwrap();
        let RpgCommandOutcome::AwaitingReaction(pending) = awaiting else {
            panic!("activation transaction must await reaction");
        };
        assert_eq!(session.state().accepted_activations_this_turn(), 0);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .activation_budget("normal"),
            Some(3),
            "the staged action cost must roll back while the reaction is pending"
        );
        let ward = &pending.request.options[0];
        assert!(ward.unavailable.is_none());
        assert_eq!(ward.activation_costs[0].budget_id, "reaction");
        assert_eq!(ward.activation_costs[0].remaining, 1);

        let pending_checkpoint = session.checkpoint().unwrap();
        let restored_pending = RpgAuthoritySession::restore_checkpoint(pending_checkpoint).unwrap();
        assert_eq!(
            restored_pending
                .state()
                .entity("hero")
                .unwrap()
                .activation_budget("normal"),
            Some(3)
        );

        let before_invalid_reaction = session.state_hash().unwrap();
        let invalid_reaction = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2],
        });
        assert!(matches!(
            invalid_reaction,
            RpgCommandOutcome::Rejected(ref rejection)
                if rejection.code == "RPG_RANDOM_EXHAUSTED"
        ));
        assert_eq!(session.state_hash().unwrap(), before_invalid_reaction);
        assert!(session.pending_reaction().is_some());
        assert_eq!(session.state().accepted_activations_this_turn(), 0);
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .activation_budget("reaction"),
            Some(1)
        );

        let (accepted, reaction_entry) = session.react_recorded(reaction_command()).unwrap();
        let RpgCommandOutcome::Accepted(receipt) = accepted else {
            panic!("selected reaction must commit atomically");
        };
        assert_eq!(session.state().accepted_activations_this_turn(), 2);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .activation_budget("normal"),
            Some(2)
        );
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .activation_budget("reaction"),
            Some(0)
        );
        assert_eq!(
            receipt
                .events
                .iter()
                .filter(|event| matches!(event, RpgDomainEvent::ActivationBudgetSpent { .. }))
                .count(),
            2
        );

        let (hero_end, hero_end_entry) = session
            .control_recorded(crate::RpgTurnControlProposal {
                expected_revision: 1,
                actor_id: "hero".to_owned(),
                control: crate::RpgTurnControl::EndTurn,
            })
            .unwrap();
        let RpgCommandOutcome::ControlAccepted(hero_end) = hero_end else {
            panic!("hero end turn must commit");
        };
        assert_eq!(session.state().accepted_activations_this_turn(), 0);
        assert!(hero_end.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::ActivationBudgetReset {
                entity_id,
                budget_id,
                ..
            } if entity_id == "guardian" && budget_id == "normal"
        )));
        let hero_turn_index = hero_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::TurnTransitioned { .. }))
            .unwrap();
        let hero_modifier_index = hero_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::ModifierDurationChanged { .. }))
            .unwrap();
        let hero_reset_index = hero_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::ActivationBudgetReset { .. }))
            .unwrap();
        assert!(hero_turn_index < hero_modifier_index);
        assert!(hero_modifier_index < hero_reset_index);
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .activation_budget("reaction"),
            Some(0),
            "round-start budget must not reset at an owner-turn boundary"
        );

        let (guardian_end, guardian_end_entry) = session
            .control_recorded(crate::RpgTurnControlProposal {
                expected_revision: 2,
                actor_id: "guardian".to_owned(),
                control: crate::RpgTurnControl::EndTurn,
            })
            .unwrap();
        let RpgCommandOutcome::ControlAccepted(guardian_end) = guardian_end else {
            panic!("guardian end turn must commit");
        };
        assert!(guardian_end.events.iter().any(|event| matches!(
            event,
            RpgDomainEvent::RoundTransitioned {
                previous_round: 1,
                current_round: 2
            }
        )));
        let round_index = guardian_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::RoundTransitioned { .. }))
            .unwrap();
        let turn_index = guardian_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::TurnTransitioned { .. }))
            .unwrap();
        let modifier_index = guardian_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::ModifierExpired { .. }))
            .unwrap();
        let reset_index = guardian_end
            .events
            .iter()
            .position(|event| matches!(event, RpgDomainEvent::ActivationBudgetReset { .. }))
            .unwrap();
        assert!(round_index < turn_index);
        assert!(turn_index < modifier_index);
        assert!(modifier_index < reset_index);
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .activation_budget("reaction"),
            Some(1)
        );

        let replayed = RpgAuthoritySession::replay(
            initial,
            &[
                submit_entry,
                reaction_entry,
                hero_end_entry,
                guardian_end_entry,
            ],
        )
        .unwrap();
        assert_eq!(
            replayed.state_hash().unwrap(),
            session.state_hash().unwrap()
        );
        assert_eq!(replayed.turn(), session.turn());
    }

    fn required_request(outcome: RpgCommandOutcome) -> rpg_core::RpgRandomRequest {
        let RpgCommandOutcome::Rejected(rejection) = outcome else {
            panic!("expected a random request rejection: {outcome:?}");
        };
        *rejection
            .random_request
            .expect("authority rejection contains an exact random request")
    }

    fn artifact_bundle() -> CompiledPlayBundle {
        compile_prepared_play_bundle(artifact_prepared())
            .expect("prepared replay artifact compiles")
    }

    fn activation_reaction_bundle() -> CompiledPlayBundle {
        let mut prepared = artifact_prepared();
        prepared.ruleset.models.action_economy =
            RulesetActionEconomyModel::VariableActivationBudgets {
                version: 1,
                accepted_activation_ceiling: 4,
            };
        prepared
            .ruleset
            .provides
            .numeric_domains
            .push(RulesetNumericDomain {
                id: "activation".to_owned(),
                minimum: 0,
                maximum: 3,
            });
        prepared
            .ruleset
            .provides
            .numeric_domains
            .sort_by(|left, right| left.id.cmp(&right.id));
        prepared.ruleset.provides.activation_budgets = vec![
            RulesetActivationBudget {
                id: "normal".to_owned(),
                version: 1,
                label: "Normal activations".to_owned(),
                numeric_domain_id: "activation".to_owned(),
                timing: RulesetActivationTiming::Action,
                reset_boundary: RulesetActivationBudgetResetBoundary::OwnerTurnStart,
                initial_amount: 3,
            },
            RulesetActivationBudget {
                id: "reaction".to_owned(),
                version: 1,
                label: "Reaction activations".to_owned(),
                numeric_domain_id: "activation".to_owned(),
                timing: RulesetActivationTiming::Reaction,
                reset_boundary: RulesetActivationBudgetResetBoundary::RoundStart,
                initial_amount: 1,
            },
        ];
        let activation_capability = VersionedRpgRequirement {
            id: "capability.activation-budgets".to_owned(),
            version: 1,
        };
        prepared
            .ruleset
            .provides
            .capabilities
            .push(activation_capability.clone());
        prepared
            .ruleset
            .provides
            .capabilities
            .sort_by(|left, right| left.id.cmp(&right.id));
        prepared
            .content_requirements
            .capabilities
            .push(activation_capability);
        prepared
            .content_requirements
            .capabilities
            .sort_by(|left, right| left.id.cmp(&right.id));

        for definition in prepared
            .materialized_definitions
            .iter_mut()
            .filter(|definition| definition.kind == MaterializedContentDefinitionKind::Action)
        {
            let costs = if definition.id == "action.reactive" {
                json!([{
                    "budget": {"rulesetId": "replay.rules", "id": "normal"},
                    "amount": 1
                }])
            } else {
                json!([])
            };
            definition.semantic["action"]["activation"] = json!({
                "timing": "action",
                "costs": costs
            });
            if definition.id == "action.reactive" {
                definition.semantic["action"]["program"]["body"]["steps"][0]["operation"]
                    ["options"][0]["activation"] = json!({
                    "timing": "reaction",
                    "costs": [{
                        "budget": {"rulesetId": "replay.rules", "id": "reaction"},
                        "amount": 1
                    }]
                });
            }
            definition.fingerprint = materialized_definition_fingerprint(definition).unwrap();
        }
        compile_prepared_play_bundle(prepared)
            .expect("variable activation-budget replay artifact compiles")
    }

    fn artifact_prepared() -> PreparedPlayBundle {
        let source_location = ContentSourceLocation {
            module: "actions/replay.rs".to_owned(),
            declaration: "reactiveStrike".to_owned(),
        };
        let provenance = ContentDefinitionProvenance {
            definition_id: "action.reactive".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: source_location.clone(),
        };
        let mut action = MaterializedContentDefinition {
            id: "action.reactive".to_owned(),
            kind: MaterializedContentDefinitionKind::Action,
            visibility: MaterializedContentVisibility::Exported,
            extension_policy: ContentExtensionPolicy::Sealed,
            semantic: json!({
                "schema": {"identity": "asha.rpg.action-definition", "version": 1},
                "kind": "inline",
                "action": {
                    "id": "action.reactive",
                    "name": "Reactive strike",
                    "sourcePath": "actions/replay.rs#reactiveStrike",
                    "targets": {"team": "hostile", "maximumRange": 3, "maximumTargets": 1},
                    "check": {"kind": "attack", "modifier": {"kind": "constant", "value": 0}, "defenseId": "catalog.defense.guard"},
                    "rollScope": "shared",
                    "costs": [],
                    "program": {"kind": "atomic", "body": {"kind": "sequence", "steps": [
                        {"kind": "operation", "operation": {"kind": "openReaction", "reactionId": "reaction.ward", "options": [
                            {"id": "ward", "label": "Raise ward", "damageReduction": 3}
                        ]}},
                        {"kind": "onCheck",
                          "hit": {"kind": "operation", "operation": {"kind": "damage", "parts": [
                              {"id": "damage", "amount": {"kind": "dice", "count": 2, "sides": 6, "bonus": 0}, "damageType": "catalog.damage.force", "tags": []}
                          ]}},
                          "miss": {"kind": "operation", "operation": {"kind": "damage", "parts": [
                              {"id": "damage", "amount": {"kind": "dice", "count": 1, "sides": 4, "bonus": 0}, "damageType": "catalog.damage.force", "tags": []}
                          ]}}
                        }
                    ]}}
                }
            }),
            presentation: json!({"label": "Reactive strike"}),
            references: vec![
                "catalog.damage.force".to_owned(),
                "catalog.defense.guard".to_owned(),
            ],
            provenance: provenance.clone(),
            fingerprint: String::new(),
        };
        action.fingerprint = materialized_definition_fingerprint(&action).unwrap();

        let movement_provenance = ContentDefinitionProvenance {
            definition_id: "action.move".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: ContentSourceLocation {
                module: "actions/replay.rs".to_owned(),
                declaration: "moveAction".to_owned(),
            },
        };
        let mut movement = MaterializedContentDefinition {
            id: "action.move".to_owned(),
            kind: MaterializedContentDefinitionKind::Action,
            visibility: MaterializedContentVisibility::Exported,
            extension_policy: ContentExtensionPolicy::Sealed,
            semantic: json!({
                "schema": {"identity": "asha.rpg.action-definition", "version": 1},
                "kind": "inline",
                "action": {
                    "id": "action.move",
                    "name": "Move",
                    "sourcePath": "actions/replay.rs#moveAction",
                    "targets": {"kind": "cell", "team": "any", "maximumRange": 2, "maximumTargets": 1},
                    "check": {"kind": "noRoll"},
                    "rollScope": "none",
                    "costs": [],
                    "program": {"kind": "atomic", "body": {"kind": "onCheck", "noRoll": {
                        "kind": "operation", "operation": {"kind": "moveToCell", "maximumDistance": 2, "provokes": true}
                    }}}
                }
            }),
            presentation: json!({"label": "Move"}),
            references: Vec::new(),
            provenance: movement_provenance.clone(),
            fingerprint: String::new(),
        };
        movement.fingerprint = materialized_definition_fingerprint(&movement).unwrap();

        let support_provenance = ContentDefinitionProvenance {
            definition_id: "catalog.damage.force".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: ContentSourceLocation {
                module: "catalogs/replay.rs".to_owned(),
                declaration: "force".to_owned(),
            },
        };
        let mut support = MaterializedContentDefinition {
            id: "catalog.damage.force".to_owned(),
            kind: MaterializedContentDefinitionKind::Support,
            visibility: MaterializedContentVisibility::Exported,
            extension_policy: ContentExtensionPolicy::Sealed,
            semantic: json!({"catalog": "damageType", "id": "force"}),
            presentation: json!({"label": "Force"}),
            references: Vec::new(),
            provenance: support_provenance.clone(),
            fingerprint: String::new(),
        };
        support.fingerprint = materialized_definition_fingerprint(&support).unwrap();

        let guard_provenance = ContentDefinitionProvenance {
            definition_id: "catalog.defense.guard".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: ContentSourceLocation {
                module: "catalogs/replay.rs".to_owned(),
                declaration: "guard".to_owned(),
            },
        };
        let mut guard = MaterializedContentDefinition {
            id: "catalog.defense.guard".to_owned(),
            kind: MaterializedContentDefinitionKind::Support,
            visibility: MaterializedContentVisibility::Exported,
            extension_policy: ContentExtensionPolicy::Sealed,
            semantic: json!({"catalog": "defense", "id": "guard"}),
            presentation: json!({"label": "Guard"}),
            references: Vec::new(),
            provenance: guard_provenance.clone(),
            fingerprint: String::new(),
        };
        guard.fingerprint = materialized_definition_fingerprint(&guard).unwrap();

        let modifier_provenance = ContentDefinitionProvenance {
            definition_id: "catalog.modifier.impeded".to_owned(),
            package_id: "replay.test".to_owned(),
            package_version: "1.0.0".to_owned(),
            source: ContentSourceLocation {
                module: "catalogs/replay.rs".to_owned(),
                declaration: "impeded".to_owned(),
            },
        };
        let mut modifier = MaterializedContentDefinition {
            id: "catalog.modifier.impeded".to_owned(),
            kind: MaterializedContentDefinitionKind::Support,
            visibility: MaterializedContentVisibility::Exported,
            extension_policy: ContentExtensionPolicy::Sealed,
            semantic: json!({"catalog": "modifier", "id": "impeded"}),
            presentation: json!({"label": "Impeded"}),
            references: Vec::new(),
            provenance: modifier_provenance.clone(),
            fingerprint: String::new(),
        };
        modifier.fingerprint = materialized_definition_fingerprint(&modifier).unwrap();

        let operations = vec![
            VersionedRpgRequirement {
                id: "operation.damage".to_owned(),
                version: 2,
            },
            VersionedRpgRequirement {
                id: "operation.moveToCell".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "operation.openReaction".to_owned(),
                version: 1,
            },
        ];
        let capabilities = vec![
            VersionedRpgRequirement {
                id: "capability.defenses".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "capability.modifiers".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "capability.position".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "capability.random".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "capability.reactions".to_owned(),
                version: 1,
            },
            VersionedRpgRequirement {
                id: "capability.vitality".to_owned(),
                version: 1,
            },
        ];
        let package_identity = "replay.test@1.0.0".to_owned();
        PreparedPlayBundle {
            schema: PlayBundleArtifactSchema {
                identity: PREPARED_PLAY_BUNDLE_IDENTITY.to_owned(),
                major: PLAY_BUNDLE_ARTIFACT_MAJOR,
            },
            play_bundle_identity: RpgVersionedIdentity {
                id: "replay.test".to_owned(),
                version: "1.0.0".to_owned(),
            },
            ruleset: Ruleset {
                schema: RulesetSchema {
                    identity: "asha.rpg.ruleset".to_owned(),
                    major: 1,
                },
                identity: RpgVersionedIdentity {
                    id: "replay.rules".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                language: RpgVersionedIdentity {
                    id: "asha-rpg".to_owned(),
                    version: "1.0.0".to_owned(),
                },
                models: RulesetModels {
                    checks: VersionedRpgRequirement {
                        id: "check.d20-roll-over".to_owned(),
                        version: 1,
                    },
                    turns: VersionedRpgRequirement {
                        id: "turn.ordered-one-action".to_owned(),
                        version: 1,
                    },
                    initiative: VersionedRpgRequirement {
                        id: "initiative.scenario-ordered".to_owned(),
                        version: 1,
                    },
                    reactions: VersionedRpgRequirement {
                        id: "reaction.before-damage-choice".to_owned(),
                        version: 1,
                    },
                    action_economy: RulesetActionEconomyModel::OneActionPlusReaction { version: 1 },
                    line_of_effect: None,
                },
                provides: RulesetProvisions {
                    operations: operations.clone(),
                    capabilities: capabilities.clone(),
                    values: vec![
                        RulesetValueContract {
                            kind: RulesetValueKind::Defense,
                            id: "catalog.defense.guard".to_owned(),
                            label: "Guard action reference".to_owned(),
                            numeric_domain_id: "defense".to_owned(),
                            source: RulesetValueSource::Input,
                        },
                        RulesetValueContract {
                            kind: RulesetValueKind::Defense,
                            id: "guard".to_owned(),
                            label: "Guard".to_owned(),
                            numeric_domain_id: "defense".to_owned(),
                            source: RulesetValueSource::Input,
                        },
                    ],
                    numeric_domains: vec![RulesetNumericDomain {
                        id: "defense".to_owned(),
                        minimum: 0,
                        maximum: 100,
                    }],
                    calculation_selectors: Vec::new(),
                    contribution_stacking_groups: Vec::new(),
                    scalar_test_profiles: Vec::new(),
                    activation_budgets: Vec::new(),
                    movement_allowance_budget_id: None,
                    heterogeneous_pool_profiles: Vec::new(),
                },
            },
            content_packs: vec![ResolvedContentPack {
                id: "replay.test".to_owned(),
                version: "1.0.0".to_owned(),
                source_fingerprint: "fnv1a64:1111111111111111".to_owned(),
            }],
            dependency_lock: Vec::new(),
            content_requirements: ContentPackRequirements {
                operations,
                capabilities,
                values: vec![rpg_ir::ContentValueRequirement {
                    kind: RulesetValueKind::Defense,
                    id: "catalog.defense.guard".to_owned(),
                }],
                numeric_domains: Vec::new(),
            },
            exported_roots: vec![
                "action.move".to_owned(),
                "action.reactive".to_owned(),
                "catalog.damage.force".to_owned(),
                "catalog.defense.guard".to_owned(),
                "catalog.modifier.impeded".to_owned(),
            ],
            materialized_definitions: vec![movement, action, support, guard, modifier],
            compiled_policy_bindings: Vec::new(),
            definition_provenance: vec![
                movement_provenance,
                provenance,
                support_provenance,
                guard_provenance,
                modifier_provenance,
            ],
            definition_commitments: Vec::new(),
            relationships: vec![
                ContentRelationshipProvenance {
                    kind: ContentRelationshipKind::Exports,
                    source: package_identity.clone(),
                    target: "action.move".to_owned(),
                    order: 0,
                },
                ContentRelationshipProvenance {
                    kind: ContentRelationshipKind::Exports,
                    source: package_identity.clone(),
                    target: "action.reactive".to_owned(),
                    order: 1,
                },
                ContentRelationshipProvenance {
                    kind: ContentRelationshipKind::Exports,
                    source: package_identity.clone(),
                    target: "catalog.damage.force".to_owned(),
                    order: 2,
                },
                ContentRelationshipProvenance {
                    kind: ContentRelationshipKind::Exports,
                    source: package_identity.clone(),
                    target: "catalog.defense.guard".to_owned(),
                    order: 3,
                },
                ContentRelationshipProvenance {
                    kind: ContentRelationshipKind::Exports,
                    source: package_identity,
                    target: "catalog.modifier.impeded".to_owned(),
                    order: 4,
                },
            ],
            derivation_provenance: Vec::new(),
            overlay_provenance: Vec::new(),
        }
    }

    fn artifact_session() -> RpgAuthoritySession {
        let bundle = artifact_bundle();
        let scenario = standard_setup(&bundle);
        RpgAuthoritySession::from_scenario(bundle, scenario).expect("replay scenario is valid")
    }

    fn standard_setup(bundle: &CompiledPlayBundle) -> RpgScenario {
        RpgScenario {
            schema: RpgScenario::schema(),
            play_bundle_id: bundle.artifact().artifact_id.clone(),
            board: crate::RpgBoardSetup {
                width: 4,
                height: 4,
                cells: vec![
                    crate::RpgCellSetup {
                        id: "cell-0-0".to_owned(),
                        position: GridPosition { x: 0, y: 0 },
                        capabilities: Vec::new(),
                    },
                    crate::RpgCellSetup {
                        id: "cell-1-0".to_owned(),
                        position: GridPosition { x: 1, y: 0 },
                        capabilities: Vec::new(),
                    },
                    crate::RpgCellSetup {
                        id: "cell-2-0".to_owned(),
                        position: GridPosition { x: 2, y: 0 },
                        capabilities: Vec::new(),
                    },
                ],
            },
            participants: vec![
                crate::RpgParticipantSetup {
                    id: "hero".to_owned(),
                    label: "Hero".to_owned(),
                    team_id: Team::ally(),
                    position: GridPosition { x: 0, y: 0 },
                    definition_ids: vec!["action.reactive".to_owned(), "action.move".to_owned()],
                    class_definition_id: None,
                    feature_definition_ids: Vec::new(),
                    items: Vec::new(),
                    equipment: Vec::new(),
                    capabilities: vec![
                        crate::RpgInitialCapability::Vitality {
                            value: BoundedValue {
                                current: 20,
                                max: 20,
                            },
                        },
                        crate::RpgInitialCapability::Defense {
                            id: "guard".to_owned(),
                            value: 10,
                        },
                        crate::RpgInitialCapability::Modifier {
                            stacking_group: "movement-control".to_owned(),
                            id: "impeded".to_owned(),
                            value: -2,
                            remaining_turns: 2,
                        },
                    ],
                },
                crate::RpgParticipantSetup {
                    id: "guardian".to_owned(),
                    label: "Guardian".to_owned(),
                    team_id: Team::enemy(),
                    position: GridPosition { x: 1, y: 1 },
                    definition_ids: vec!["action.reactive".to_owned(), "action.move".to_owned()],
                    class_definition_id: None,
                    feature_definition_ids: Vec::new(),
                    items: Vec::new(),
                    equipment: Vec::new(),
                    capabilities: vec![
                        crate::RpgInitialCapability::Vitality {
                            value: BoundedValue {
                                current: 20,
                                max: 20,
                            },
                        },
                        crate::RpgInitialCapability::Defense {
                            id: "guard".to_owned(),
                            value: 10,
                        },
                    ],
                },
            ],
            turn: crate::RpgTurnInitialization {
                initiative_order: vec!["hero".to_owned(), "guardian".to_owned()],
                current_actor_id: "hero".to_owned(),
                round: 1,
                turn: 1,
            },
            random_source: crate::RpgRandomSourceBinding {
                policy_id: "test.recorded-evidence".to_owned(),
                policy_version: 1,
                source_id: "test.roll-tape".to_owned(),
                source_version: 1,
            },
        }
    }

    fn command() -> RpgAuthorityCommand {
        RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
                cell_targets: Vec::new(),
                item_binding: None,
            },
            random_values: vec![12],
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
