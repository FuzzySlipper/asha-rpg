//! Rpg's real downstream gameplay module and public ASHA host adapter.

#![forbid(unsafe_code)]

use asha_gameplay_module_sdk::*;
use asha_runtime_session_composition::{
    BundleArtifacts, ComposedGameplayOwner, ComposedGameplayOwnerCheckpoint,
    ComposedGameplayOwnerOutput, ComposedGameplayRuntime, ComposedRuntimeSessionCheckpoint,
    GameplayBindingEntityTargets, GameplayDecisionMoment, GameplayDecisionReceipt,
    GameplayDecisionStatus, GameplayOperationWorkspace, GameplayRuntimeDeclaredReadPlan,
    GameplayRuntimeProjectInput, GameplayRuntimeSchedulerDefinition, LoadPlan, LoadStep,
    RuntimeSessionId, SceneId, StaticRuntimeSessionBuilder,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const MODULE_ID: &str = "rpg.pre-effect-reaction";
const PROVIDER_ID: &str = "provider.rpg.pre-effect-reaction";
const OWNER_ID: &str = "authority.rpg.combat";
const STATE_READ_ID: &str = "reaction-state";

/// Exact ASHA Git revision resolved by Cargo for the governed public facade.
///
/// The build script reads Cargo.lock so capability evidence cannot drift from
/// the dependency that actually compiled this crate.
pub const GOVERNED_ASHA_REVISION: &str = env!("ASHA_RPG_GOVERNED_ASHA_REVISION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreEffectWorkspace {
    pub decision_id: String,
    pub actor_id: String,
    pub target_id: String,
    pub action_id: String,
    pub damage_amount: u32,
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionFabricConfig {
    pub accepted_reaction_damage_reduction: u32,
}

impl Default for ReactionFabricConfig {
    fn default() -> Self {
        Self {
            accepted_reaction_damage_reduction: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionFabricState {
    pub revision: u64,
    pub opened_windows: u64,
    pub resolved_windows: u64,
    pub accepted_reactions: u64,
    pub last_decision_id: Option<String>,
    pub last_option_id: Option<String>,
    pub last_resolution_accepted: bool,
    pub accepted_reaction_damage_reduction: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ReactionFabricFact {
    Opened {
        decision_id: String,
    },
    Resolved {
        decision_id: String,
        accepted: bool,
        option_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReactionOpenedEvent {
    decision_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReactionResolvedEvent {
    decision_id: String,
    accepted: bool,
    option_id: Option<String>,
}

struct PreEffectReactionBehavior;

impl GameplayModuleBehavior for PreEffectReactionBehavior {
    fn invoke(
        &self,
        context: &GameplayModuleContext<'_>,
    ) -> Result<GameplayModuleActions, GameplayModuleError> {
        if context.event_contract() == Some(&contract("reaction-opened")) {
            let event: ReactionOpenedEvent = context.event_payload()?;
            return record_fact(
                context,
                ReactionFabricFact::Opened {
                    decision_id: event.decision_id,
                },
            );
        }
        if context.event_contract() == Some(&contract("reaction-resolved")) {
            let event: ReactionResolvedEvent = context.event_payload()?;
            return record_fact(
                context,
                ReactionFabricFact::Resolved {
                    decision_id: event.decision_id,
                    accepted: event.accepted,
                    option_id: event.option_id,
                },
            );
        }

        let _state: ReactionFabricState = context.named_view(STATE_READ_ID)?;
        let workspace: PreEffectWorkspace = context.decision_workspace()?;
        let mut actions = context.actions();
        match context.invocation_id() {
            "rpg.pre-effect.transform" => {
                actions.transform_workspace_json(
                    contract("pre-effect-workspace"),
                    context
                        .decision_workspace_hash()
                        .expect("Transform has a Workspace hash"),
                    &workspace,
                )?;
            }
            "rpg.pre-effect.react" if context.decision_resume_token().is_none() => {
                actions.react(
                    GameplayReactionDisposition::Suspend {
                        token: format!("rpg-window:{}", workspace.decision_id),
                    },
                    None,
                );
            }
            "rpg.pre-effect.react" => {
                actions.react(GameplayReactionDisposition::Continue, None);
            }
            _ => {
                return Err(GameplayModuleError {
                    code: "unexpectedRpgInvocation".to_owned(),
                    message: context.invocation_id().to_owned(),
                });
            }
        }
        Ok(actions)
    }
}

fn record_fact(
    context: &GameplayModuleContext<'_>,
    fact: ReactionFabricFact,
) -> Result<GameplayModuleActions, GameplayModuleError> {
    let state: ReactionFabricState = context.named_view(STATE_READ_ID)?;
    let mut actions = context.actions();
    actions.record_local_fact_json(
        contract("reaction-fact"),
        contract("reaction-state"),
        GameplayModuleStateScope::Session,
        state.revision,
        &fact,
    )?;
    Ok(actions)
}

struct ReactionStateAdapter;

impl GameplayTypedModuleStateAdapter for ReactionStateAdapter {
    type Config = ReactionFabricConfig;
    type State = ReactionFabricState;
    type Fact = ReactionFabricFact;
    type View = ReactionFabricState;

    fn module_id(&self) -> &str {
        MODULE_ID
    }

    fn state_schema(&self) -> &GameplayContractRef {
        static VALUE: std::sync::OnceLock<GameplayContractRef> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| contract("reaction-state"))
    }

    fn fact_schema(&self) -> &GameplayContractRef {
        static VALUE: std::sync::OnceLock<GameplayContractRef> = std::sync::OnceLock::new();
        VALUE.get_or_init(|| contract("reaction-fact"))
    }

    fn owner(&self) -> &GameplayOwnerRef {
        static VALUE: std::sync::OnceLock<GameplayOwnerRef> = std::sync::OnceLock::new();
        VALUE.get_or_init(state_owner)
    }

    fn decode_config(&self, value: &[u8]) -> Result<Self::Config, String> {
        serde_json::from_slice(value).map_err(|error| error.to_string())
    }

    fn initialize(&self, config: &Self::Config) -> Result<Self::State, String> {
        Ok(ReactionFabricState {
            revision: 0,
            opened_windows: 0,
            resolved_windows: 0,
            accepted_reactions: 0,
            last_decision_id: None,
            last_option_id: None,
            last_resolution_accepted: false,
            accepted_reaction_damage_reduction: config.accepted_reaction_damage_reduction,
        })
    }

    fn decode_state(&self, value: &[u8]) -> Result<Self::State, String> {
        serde_json::from_slice(value).map_err(|error| error.to_string())
    }

    fn encode_state(&self, state: &Self::State) -> Result<Vec<u8>, String> {
        serde_json::to_vec(state).map_err(|error| error.to_string())
    }

    fn decode_fact(&self, value: &[u8]) -> Result<Self::Fact, String> {
        serde_json::from_slice(value).map_err(|error| error.to_string())
    }

    fn apply_fact(&self, state: &Self::State, fact: &Self::Fact) -> Result<Self::State, String> {
        let mut next = state.clone();
        next.revision = next.revision.saturating_add(1);
        match fact {
            ReactionFabricFact::Opened { decision_id } => {
                next.opened_windows = next.opened_windows.saturating_add(1);
                next.last_decision_id = Some(decision_id.clone());
                next.last_option_id = None;
                next.last_resolution_accepted = false;
            }
            ReactionFabricFact::Resolved {
                decision_id,
                accepted,
                option_id,
            } => {
                next.opened_windows = next.opened_windows.saturating_add(1);
                next.resolved_windows = next.resolved_windows.saturating_add(1);
                next.accepted_reactions =
                    next.accepted_reactions.saturating_add(u64::from(*accepted));
                next.last_decision_id = Some(decision_id.clone());
                next.last_option_id.clone_from(option_id);
                next.last_resolution_accepted = *accepted;
            }
        }
        Ok(next)
    }

    fn migrate(&self, _from_version: u32, state: &Self::State) -> Result<Self::State, String> {
        Ok(state.clone())
    }

    fn view_schema(&self) -> Option<&GameplayContractRef> {
        static VALUE: std::sync::OnceLock<GameplayContractRef> = std::sync::OnceLock::new();
        Some(VALUE.get_or_init(|| contract("reaction-state-view")))
    }

    fn project_view(&self, state: &Self::State) -> Result<Self::View, String> {
        Ok(state.clone())
    }

    fn encode_view(&self, view: &Self::View) -> Result<Vec<u8>, String> {
        serde_json::to_vec(view).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgGameplayContinuation {
    pub decision_id: String,
    pub operation: GameplayProposalEnvelope,
    pub expected_owner_revision: String,
    pub workspace: GameplayOperationWorkspace,
    pub resume_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgGameplayDecisionEvidence {
    pub decision_id: String,
    pub status: String,
    pub receipt_hash: String,
    pub initial_workspace_hash: String,
    pub final_workspace_hash: String,
    pub declared_read_hashes: Vec<String>,
    pub invocation_output_hashes: Vec<String>,
    pub routing_hash: Option<String>,
    pub diagnostic_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgGameplayFabricReadout {
    pub registry_digest: String,
    pub binding_registry_hash: String,
    pub module_state_hash: String,
    pub runtime_host_hash: String,
    pub reaction_frame_hashes: Vec<String>,
    pub decisions: Vec<RpgGameplayDecisionEvidence>,
    pub pending_decision_count: u32,
}

pub trait RpgPreEffectOwner {
    fn revision_hash(&self) -> String;
    fn validate_commit(&self, workspace: &PreEffectWorkspace) -> Result<(), Vec<String>>;
    fn commit(&mut self, workspace: &PreEffectWorkspace) -> Vec<String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComposedOwnerState {
    revision: String,
    accepted_reaction_damage_reduction: u32,
    response_accepted: Option<bool>,
    response_option_id: Option<String>,
    committed_workspace: Option<PreEffectWorkspace>,
}

impl Default for ComposedOwnerState {
    fn default() -> Self {
        Self {
            revision: "unbound".to_owned(),
            accepted_reaction_damage_reduction: ReactionFabricConfig::default()
                .accepted_reaction_damage_reduction,
            response_accepted: None,
            response_option_id: None,
            committed_workspace: None,
        }
    }
}

struct RpgComposedOwner {
    owner: GameplayOwnerRef,
    state: Arc<Mutex<ComposedOwnerState>>,
}

pub struct RpgGameplayFabric {
    runtime: ComposedGameplayRuntime<RpgComposedOwner>,
    owner_state: Arc<Mutex<ComposedOwnerState>>,
    readout: RpgGameplayFabricReadout,
}

impl core::fmt::Debug for RpgGameplayFabric {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RpgGameplayFabric")
            .field("readout", &self.readout())
            .finish()
    }
}

impl RpgGameplayFabric {
    pub fn new() -> Self {
        let owner_state = Arc::new(Mutex::new(ComposedOwnerState::default()));
        let owner = RpgComposedOwner {
            owner: combat_owner(),
            state: Arc::clone(&owner_state),
        };
        let runtime = StaticRuntimeSessionBuilder::activate_project(project_input())
            .expect("static Rpg gameplay composition is valid")
            .with_gameplay_owner(owner)
            .expect("static Rpg combat owner is valid")
            .build()
            .expect("static Rpg RuntimeSession composition is valid");
        let mut fabric = Self {
            runtime,
            owner_state,
            readout: empty_fabric_readout(),
        };
        fabric.refresh_readout().expect("composed Rpg readout");
        fabric
    }

    pub fn restore(snapshot: &RpgGameplayFabricSnapshot) -> Result<Self, String> {
        let owner_state = Arc::new(Mutex::new(ComposedOwnerState::default()));
        let owner = RpgComposedOwner {
            owner: combat_owner(),
            state: Arc::clone(&owner_state),
        };
        let runtime =
            StaticRuntimeSessionBuilder::restore_project(project_input(), &snapshot.checkpoint)
                .map_err(|error| error.to_string())?
                .with_gameplay_owner(owner)
                .map_err(|error| error.to_string())?
                .build()
                .map_err(|error| error.to_string())?;
        let mut fabric = Self {
            runtime,
            owner_state,
            readout: snapshot.readout.clone(),
        };
        fabric.refresh_readout()?;
        Ok(fabric)
    }

    pub fn snapshot(&mut self) -> RpgGameplayFabricSnapshot {
        RpgGameplayFabricSnapshot {
            checkpoint: self
                .runtime
                .checkpoint_composed_runtime_session()
                .expect("Rpg composed RuntimeSession checkpoint"),
            readout: self.readout.clone(),
        }
    }

    pub fn begin_before_effect(
        &mut self,
        workspace: PreEffectWorkspace,
        expected_owner_revision: String,
    ) -> Result<RpgGameplayContinuation, String> {
        let moment = decision_moment(workspace, expected_owner_revision.clone());
        let operation = moment.operation.clone();
        let owner_state_before = {
            let mut state = self.lock_owner_state()?;
            let owner_state_before = state.clone();
            state.revision.clone_from(&expected_owner_revision);
            state.response_accepted = None;
            state.response_option_id = None;
            state.committed_workspace = None;
            owner_state_before
        };
        let transaction = match self.runtime.transact_composed_gameplay_owner(moment) {
            Ok(transaction) => transaction,
            Err(error) => {
                *self.lock_owner_state()? = owner_state_before;
                self.refresh_readout()?;
                return Err(error.to_string());
            }
        };
        let reaction_frame_hashes = transaction.reaction_frame_hashes;
        let receipt = transaction.decision;
        if receipt.status != GameplayDecisionStatus::Suspended {
            *self.lock_owner_state()? = owner_state_before;
            self.refresh_readout()?;
            return Err(format!(
                "decision did not suspend: {:?}",
                receipt.diagnostics
            ));
        }
        let evidence = decision_evidence(&receipt);
        let decision_id = receipt.decision_id.clone();
        let continuation = receipt
            .continuation
            .ok_or_else(|| "suspended decision omitted continuation".to_owned())?;
        self.readout.decisions.push(evidence);
        self.readout
            .reaction_frame_hashes
            .extend(reaction_frame_hashes);
        self.refresh_readout()?;
        Ok(RpgGameplayContinuation {
            decision_id,
            operation,
            expected_owner_revision,
            workspace: continuation.workspace,
            resume_token: continuation.token,
        })
    }

    pub fn resolve_before_effect(
        &mut self,
        pending: &RpgGameplayContinuation,
        accepted: bool,
        option_id: Option<String>,
        owner: &mut dyn RpgPreEffectOwner,
    ) -> Result<GameplayDecisionReceipt, String> {
        let actual_owner_revision = owner.revision_hash();
        if actual_owner_revision != pending.expected_owner_revision {
            return Err(format!(
                "stale Rpg owner revision: expected {}, found {actual_owner_revision}",
                pending.expected_owner_revision
            ));
        }
        let mut predicted_workspace: PreEffectWorkspace =
            serde_json::from_slice(&pending.workspace.canonical_payload)
                .map_err(|error| error.to_string())?;
        if accepted {
            predicted_workspace.damage_amount = predicted_workspace
                .damage_amount
                .saturating_sub(self.lock_owner_state()?.accepted_reaction_damage_reduction);
        }
        owner
            .validate_commit(&predicted_workspace)
            .map_err(|diagnostics| diagnostics.join(", "))?;
        let owner_state_before = {
            let mut state = self.lock_owner_state()?;
            let owner_state_before = state.clone();
            state.response_accepted = Some(accepted);
            state.response_option_id.clone_from(&option_id);
            state.committed_workspace = None;
            owner_state_before
        };
        let moment = GameplayDecisionMoment {
            decision_id: pending.decision_id.clone(),
            operation: pending.operation.clone(),
            expected_owner_revision: pending.expected_owner_revision.clone(),
            workspace: pending.workspace.clone(),
            resume_token: Some(pending.resume_token.clone()),
        };
        let transaction = match self.runtime.transact_composed_gameplay_owner(moment) {
            Ok(transaction) => transaction,
            Err(error) => {
                *self.lock_owner_state()? = owner_state_before;
                self.refresh_readout()?;
                return Err(error.to_string());
            }
        };
        let reaction_frame_hashes = transaction.reaction_frame_hashes;
        let receipt = transaction.decision;
        if receipt.accepted() {
            let committed_workspace = self
                .lock_owner_state()?
                .committed_workspace
                .clone()
                .expect("accepted composed owner retains its workspace");
            assert_eq!(
                committed_workspace, predicted_workspace,
                "composed owner must commit the prevalidated Rpg workspace"
            );
            let _fact_hashes = owner.commit(&committed_workspace);
            self.readout.decisions.push(decision_evidence(&receipt));
            self.readout
                .reaction_frame_hashes
                .extend(reaction_frame_hashes);
        } else {
            *self.lock_owner_state()? = owner_state_before;
        }
        self.refresh_readout()?;
        Ok(receipt)
    }

    pub fn readout(&self) -> RpgGameplayFabricReadout {
        self.readout.clone()
    }

    fn lock_owner_state(&self) -> Result<std::sync::MutexGuard<'_, ComposedOwnerState>, String> {
        self.owner_state
            .lock()
            .map_err(|_| "Rpg composed owner state lock poisoned".to_owned())
    }

    fn refresh_readout(&mut self) -> Result<(), String> {
        let composed = self
            .runtime
            .read_composed_runtime_session()
            .map_err(|error| error.to_string())?;
        self.readout.registry_digest = composed.gameplay.gameplay_registry_digest;
        self.readout.binding_registry_hash = composed.gameplay.binding_registry_hash;
        self.readout.module_state_hash = composed.gameplay.module_state_hash;
        self.readout.runtime_host_hash = composed.gameplay.runtime_host_hash;
        self.readout.pending_decision_count = composed.gameplay.pending_decision_count;
        Ok(())
    }
}

pub struct RpgGameplayFabricSnapshot {
    checkpoint: ComposedRuntimeSessionCheckpoint,
    readout: RpgGameplayFabricReadout,
}

fn empty_fabric_readout() -> RpgGameplayFabricReadout {
    RpgGameplayFabricReadout {
        registry_digest: String::new(),
        binding_registry_hash: String::new(),
        module_state_hash: String::new(),
        runtime_host_hash: String::new(),
        reaction_frame_hashes: Vec::new(),
        decisions: Vec::new(),
        pending_decision_count: 0,
    }
}

impl Default for RpgGameplayFabric {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposedGameplayOwner for RpgComposedOwner {
    fn owner(&self) -> &GameplayOwnerRef {
        &self.owner
    }

    fn revision_hash(&self) -> String {
        self.state
            .lock()
            .map(|state| state.revision.clone())
            .unwrap_or_else(|_| "rpg-owner-lock-poisoned".to_owned())
    }

    fn checkpoint(&self) -> Result<ComposedGameplayOwnerCheckpoint, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Rpg composed owner state lock poisoned".to_owned())?;
        let canonical_state = serde_json::to_vec(&*state).map_err(|error| error.to_string())?;
        let replay_hash = gameplay_module_payload_hash(&canonical_state);
        ComposedGameplayOwnerCheckpoint::new(self.owner.clone(), canonical_state, replay_hash)
    }

    fn restore(&mut self, checkpoint: &ComposedGameplayOwnerCheckpoint) -> Result<(), String> {
        if checkpoint.owner() != &self.owner {
            return Err("Rpg composed owner identity mismatch".to_owned());
        }
        let restored: ComposedOwnerState = serde_json::from_slice(checkpoint.canonical_state())
            .map_err(|error| error.to_string())?;
        *self
            .state
            .lock()
            .map_err(|_| "Rpg composed owner state lock poisoned".to_owned())? = restored;
        Ok(())
    }

    fn route_precommit(
        &mut self,
        operation: &GameplayProposalEnvelope,
    ) -> ComposedGameplayOwnerOutput {
        let mut workspace: PreEffectWorkspace =
            match serde_json::from_slice(&operation.canonical_payload) {
                Ok(workspace) => workspace,
                Err(_) => {
                    return ComposedGameplayOwnerOutput {
                        diagnostic_codes: vec!["rpgWorkspaceDecodeFailed".to_owned()],
                        ..ComposedGameplayOwnerOutput::default()
                    };
                }
            };
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return ComposedGameplayOwnerOutput {
                    diagnostic_codes: vec!["rpgOwnerLockPoisoned".to_owned()],
                    ..ComposedGameplayOwnerOutput::default()
                };
            }
        };
        let Some(accepted) = state.response_accepted else {
            return ComposedGameplayOwnerOutput {
                diagnostic_codes: vec!["missingRpgReactionResponse".to_owned()],
                ..ComposedGameplayOwnerOutput::default()
            };
        };
        if accepted {
            workspace.damage_amount = workspace
                .damage_amount
                .saturating_sub(state.accepted_reaction_damage_reduction);
        }
        state.committed_workspace = Some(workspace.clone());
        let option_id = state.response_option_id.clone();
        let committed_payload = match serde_json::to_vec(&workspace) {
            Ok(payload) => payload,
            Err(_) => {
                return ComposedGameplayOwnerOutput {
                    diagnostic_codes: vec!["rpgWorkspaceEncodeFailed".to_owned()],
                    ..ComposedGameplayOwnerOutput::default()
                };
            }
        };
        let events = match reaction_events(operation, &workspace.decision_id, accepted, option_id) {
            Ok(events) => events,
            Err(code) => {
                return ComposedGameplayOwnerOutput {
                    diagnostic_codes: vec![code],
                    ..ComposedGameplayOwnerOutput::default()
                };
            }
        };
        ComposedGameplayOwnerOutput {
            accepted: true,
            fact_hashes: vec![gameplay_module_payload_hash(&committed_payload)],
            diagnostic_codes: Vec::new(),
            events,
        }
    }
}

fn reaction_events(
    operation: &GameplayProposalEnvelope,
    decision_id: &str,
    accepted: bool,
    option_id: Option<String>,
) -> Result<Vec<GameplayEventEnvelope>, String> {
    Ok(vec![
        owner_event(
            operation,
            0,
            contract("reaction-opened"),
            &ReactionOpenedEvent {
                decision_id: decision_id.to_owned(),
            },
        )?,
        owner_event(
            operation,
            1,
            contract("reaction-resolved"),
            &ReactionResolvedEvent {
                decision_id: decision_id.to_owned(),
                accepted,
                option_id,
            },
        )?,
    ])
}

fn owner_event<T: Serialize>(
    operation: &GameplayProposalEnvelope,
    event_sequence: u32,
    event: GameplayContractRef,
    payload: &T,
) -> Result<GameplayEventEnvelope, String> {
    let canonical_payload = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
    Ok(GameplayEventEnvelope {
        event_id: format!("{}/reaction/{event_sequence}", operation.proposal_id),
        event,
        tick: operation.tick,
        root_sequence: operation.root_sequence,
        wave: operation.wave.saturating_add(1),
        event_sequence,
        phase: GameplayEventPhase::PostCommit,
        emitter: GameplayEmitterRef::Owner {
            owner_id: OWNER_ID.to_owned(),
        },
        causation: operation.causation.clone(),
        source: operation.source.clone(),
        subjects: Vec::new(),
        targets: operation.targets.clone(),
        scope: Some("rpg.reaction-window".to_owned()),
        tags: vec!["pre-effect".to_owned()],
        payload_hash: gameplay_canonical_payload_hash(&canonical_payload),
        canonical_payload,
    })
}

fn provider() -> GameplayStaticModuleProvider {
    let owner = state_owner();
    let proposal = contract("pre-effect-operation");
    let state_view = contract("reaction-state-view");
    let configuration_metadata = GameplayConfigurationSchemaMetadata {
        module_id: MODULE_ID.to_owned(),
        configuration: contract("configuration"),
        codec_id: gameplay_canonical_codec_id(&contract("configuration").schema_hash),
        fields: vec![GameplayConfigurationFieldMetadata {
            name: "acceptedReactionDamageReduction".to_owned(),
            value_type: "u32".to_owned(),
            required: true,
        }],
    };
    let manifest = GameplayModuleManifest {
        module_ref: module_ref(),
        published_events: vec![
            event_schema("reaction-opened"),
            event_schema("reaction-resolved"),
        ],
        subscriptions: vec![subscription("reaction-resolved")],
        invocations: vec![
            observe_invocation("reaction-resolved"),
            GameplayInvocationDescriptor {
                invocation_id: "rpg.pre-effect.transform".to_owned(),
                family: GameplayInvocationFamily::Transform,
                input_contract: proposal.clone(),
                output_contract: contract("pre-effect-workspace"),
                read_requirements: state_read_requirements(),
                max_outputs: 1,
                max_payload_bytes: 8_192,
            },
            GameplayInvocationDescriptor {
                invocation_id: "rpg.pre-effect.react".to_owned(),
                family: GameplayInvocationFamily::React,
                input_contract: proposal.clone(),
                output_contract: contract("pre-effect-workspace"),
                read_requirements: state_read_requirements(),
                max_outputs: 1,
                max_payload_bytes: 8_192,
            },
        ],
        read_views: vec![GameplayReadViewRequirement {
            view: state_view.clone(),
            provider_id: PROVIDER_ID.to_owned(),
            kind: GameplayReadViewKind::ModuleNamed,
            fields: vec![
                "acceptedReactionDamageReduction".to_owned(),
                "lastDecisionId".to_owned(),
                "lastResolutionAccepted".to_owned(),
                "revision".to_owned(),
            ],
            selector_capabilities: vec![GameplayReadSelectorCapability::ModuleStateScope],
            max_items: 1,
        }],
        proposal_kinds: vec![GameplayProposalDeclaration {
            proposal: proposal.clone(),
            owner: combat_owner(),
        }],
        state_schemas: vec![GameplayOwnedSchemaDeclaration {
            schema: contract("reaction-state"),
            owner: owner.clone(),
        }],
        fact_schemas: vec![GameplayOwnedSchemaDeclaration {
            schema: contract("reaction-fact"),
            owner: owner.clone(),
        }],
        ordering: Vec::new(),
        budget: GameplayExecutionBudget {
            max_waves: 4,
            max_events_per_root: 8,
            max_proposals_per_root: 2,
            max_invocations_per_root: 12,
            max_payload_bytes_per_root: 32_768,
        },
        deterministic_requirements: vec!["canonical-json".to_owned()],
        source_hash: "sha256:rpg-pre-effect-source-v1".to_owned(),
    };
    let provenance = build_provenance();
    let mut manifest = manifest;
    provenance.apply_to_manifest::<PreEffectReactionBehavior>(&mut manifest);
    GameplayStaticModuleProvider::linked_from_manifest(
        manifest,
        &provenance,
        PreEffectReactionBehavior,
    )
    .event_codec(codec::<ReactionOpenedEvent>(contract("reaction-opened")))
    .event_codec(codec::<ReactionResolvedEvent>(contract(
        "reaction-resolved",
    )))
    .proposal_codec(codec::<PreEffectWorkspace>(proposal.clone()))
    .proposal_owner(GameplayProposalOwnerRegistration {
        proposal,
        owner: combat_owner(),
    })
    .read_view_provider(GameplayReadViewProviderRegistration {
        view: state_view,
        provider_id: PROVIDER_ID.to_owned(),
        kind: GameplayReadViewKind::ModuleNamed,
        fields: vec![
            "acceptedReactionDamageReduction".to_owned(),
            "lastDecisionId".to_owned(),
            "lastResolutionAccepted".to_owned(),
            "revision".to_owned(),
        ],
        selector_capabilities: vec![GameplayReadSelectorCapability::ModuleStateScope],
        max_items: 1,
        ordering: "singleton".to_owned(),
    })
    .state_owner(GameplayStateOwnerRegistration {
        schema: contract("reaction-state"),
        owner: owner.clone(),
    })
    .state_owner(GameplayStateOwnerRegistration {
        schema: contract("reaction-fact"),
        owner,
    })
    .state_adapter(GameplayModuleStateRegistration::typed(ReactionStateAdapter))
    .configuration_schema(configuration_metadata.clone())
    .configuration_codec(GameplayConfigurationCodecRegistration::typed::<
        ReactionFabricConfig,
    >(configuration_metadata))
}

fn project_input() -> GameplayRuntimeProjectInput {
    let provider = provider();
    let linked_module = provider.manifest.module_ref.clone();
    let mut composition = GameplayStaticCompositionBuilder::new();
    composition.add_provider(provider);
    GameplayRuntimeProjectInput {
        load_plan: LoadPlan {
            steps: vec![
                LoadStep::ValidateVersions {
                    bundle_schema_version: 1,
                    protocol_version: 1,
                },
                LoadStep::LoadAssetLock {
                    artifact: "assets/lock.json".to_owned(),
                    asset_count: 0,
                },
                LoadStep::LoadSceneDocument {
                    artifact: "scene/scene.json".to_owned(),
                    scene: SceneId::new(1),
                },
                LoadStep::BootstrapScene {
                    scene: SceneId::new(1),
                    runtime_session: RuntimeSessionId::new(1),
                },
                LoadStep::ValidateFinalState,
            ],
        },
        artifacts: BundleArtifacts::new()
            .with_artifact("assets/lock.json", "{ \"entries\": [] }\n")
            .with_artifact("scene/scene.json", SCENE_JSON),
        composition: composition.build().expect("Rpg composition"),
        bindings: binding_registry(linked_module),
        entity_targets: GameplayBindingEntityTargets::new(),
        spatial_entities: Vec::new(),
        declared_reads: declared_reads(),
        triggers: Vec::new(),
        scheduler: GameplayRuntimeSchedulerDefinition::new(
            GameplayOwnerRef {
                owner_id: "authority.rpg.scheduler".to_owned(),
                provider_id: PROVIDER_ID.to_owned(),
            },
            Vec::new(),
            Vec::new(),
        ),
    }
}

fn binding_registry(module: GameplayModuleRef) -> GameplayModuleBindingRegistry {
    let canonical_config = serde_json::to_vec(&ReactionFabricConfig::default())
        .expect("Rpg reaction config serializes");
    let configuration = GameplayModuleConfiguration {
        configuration_id: "rpg.pre-effect.default".to_owned(),
        module,
        configuration: contract("configuration"),
        codec_id: gameplay_canonical_codec_id(&contract("configuration").schema_hash),
        config_hash: gameplay_module_payload_hash(&canonical_config),
        canonical_config,
    };
    let binding = GameplayModuleBinding {
        binding_id: "rpg.pre-effect.session".to_owned(),
        module_id: MODULE_ID.to_owned(),
        configuration_id: configuration.configuration_id.clone(),
        state_schema: contract("reaction-state"),
        target: GameplayModuleBindingTarget::Session,
        required_reads: vec![GameplayReadViewRequirement {
            view: contract("reaction-state-view"),
            provider_id: PROVIDER_ID.to_owned(),
            kind: GameplayReadViewKind::ModuleNamed,
            fields: vec![
                "acceptedReactionDamageReduction".to_owned(),
                "lastDecisionId".to_owned(),
                "lastResolutionAccepted".to_owned(),
                "revision".to_owned(),
            ],
            selector_capabilities: vec![GameplayReadSelectorCapability::ModuleStateScope],
            max_items: 1,
        }],
        output_contracts: vec![contract("reaction-resolved")],
        enabled: true,
    };
    let mut builder = GameplayModuleBindingRegistryBuilder::new();
    builder.configuration(configuration).binding(binding);
    builder.build()
}

fn declared_reads() -> Vec<GameplayRuntimeDeclaredReadPlan> {
    [
        "rpg.reaction-opened.observe",
        "rpg.reaction-resolved.observe",
        "rpg.pre-effect.transform",
        "rpg.pre-effect.react",
    ]
    .into_iter()
    .map(|invocation_id| GameplayRuntimeDeclaredReadPlan {
        module_id: MODULE_ID.to_owned(),
        invocation_id: invocation_id.to_owned(),
        requests: vec![GameplayReadRequest {
            request_id: STATE_READ_ID.to_owned(),
            view: contract("reaction-state-view"),
            fields: vec![
                "acceptedReactionDamageReduction".to_owned(),
                "lastDecisionId".to_owned(),
                "lastResolutionAccepted".to_owned(),
                "revision".to_owned(),
            ],
            selector: GameplayReadSelector::ModuleNamed {
                scope: GameplayModuleStateScope::Session,
            },
        }],
    })
    .collect()
}

fn decision_moment(
    workspace: PreEffectWorkspace,
    expected_owner_revision: String,
) -> GameplayDecisionMoment {
    let decision_id = workspace.decision_id.clone();
    let canonical_payload = serde_json::to_vec(&workspace).expect("Workspace serializes");
    GameplayDecisionMoment {
        decision_id: decision_id.clone(),
        operation: GameplayProposalEnvelope {
            proposal_id: format!("{decision_id}/operation"),
            proposal: contract("pre-effect-operation"),
            tick: 0,
            root_sequence: 0,
            wave: 0,
            proposal_sequence: 0,
            emitter: GameplayEmitterRef::Owner {
                owner_id: OWNER_ID.to_owned(),
            },
            causation: GameplayCausationRef {
                root_id: decision_id.clone(),
                parent_event_id: None,
                decision_id: Some(decision_id),
            },
            originating_event_id: None,
            source: None,
            targets: Vec::new(),
            payload_hash: gameplay_canonical_payload_hash(&canonical_payload),
            canonical_payload: canonical_payload.clone(),
        },
        expected_owner_revision,
        workspace: GameplayOperationWorkspace::from_payload(
            contract("pre-effect-workspace"),
            canonical_payload,
        ),
        resume_token: None,
    }
}

fn decision_evidence(receipt: &GameplayDecisionReceipt) -> RpgGameplayDecisionEvidence {
    RpgGameplayDecisionEvidence {
        decision_id: receipt.decision_id.clone(),
        status: format!("{:?}", receipt.status),
        receipt_hash: receipt.receipt_hash.clone(),
        initial_workspace_hash: receipt.initial_workspace_hash.clone(),
        final_workspace_hash: receipt.final_workspace_hash.clone(),
        declared_read_hashes: receipt
            .invocations
            .iter()
            .filter_map(|invocation| invocation.declared_read_set_hash.clone())
            .collect(),
        invocation_output_hashes: receipt
            .invocations
            .iter()
            .map(|invocation| invocation.output_hash.clone())
            .collect(),
        routing_hash: receipt
            .routing
            .as_ref()
            .map(|routing| routing.routing_hash.clone()),
        diagnostic_codes: receipt
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{:?}", diagnostic.code))
            .collect(),
    }
}

fn contract(name: &str) -> GameplayContractRef {
    gameplay_contract("rpg.pre-effect", name, 1, schema_descriptor(name))
}

fn schema_descriptor(name: &str) -> &'static str {
    match name {
        "configuration" => {
            "rpg.pre-effect.configuration.v1:{acceptedReactionDamageReduction:u32};canonical-json-v1"
        }
        "pre-effect-operation" => {
            "rpg.pre-effect.operation.v1:{decisionId:string,actorId:string,targetId:string,actionId:string,damageAmount:u32,damageType:string};canonical-json-v1"
        }
        "pre-effect-workspace" => {
            "rpg.pre-effect.workspace.v1:{decisionId:string,actorId:string,targetId:string,actionId:string,damageAmount:u32,damageType:string};canonical-json-v1"
        }
        "reaction-opened" => {
            "rpg.pre-effect.reaction-opened.v1:{decisionId:string};canonical-json-v1"
        }
        "reaction-resolved" => {
            "rpg.pre-effect.reaction-resolved.v1:{decisionId:string,accepted:boolean,optionId:string?};canonical-json-v1"
        }
        "reaction-fact" => {
            "rpg.pre-effect.reaction-fact.v1:opened{decisionId:string}|resolved{decisionId:string,accepted:boolean,optionId:string?};canonical-json-v1"
        }
        "reaction-state" | "reaction-state-view" => {
            "rpg.pre-effect.reaction-state.v1:{revision:u64,openedWindows:u64,resolvedWindows:u64,acceptedReactions:u64,lastDecisionId:string?,lastOptionId:string?,lastResolutionAccepted:boolean,acceptedReactionDamageReduction:u32};canonical-json-v1"
        }
        _ => panic!("unknown Rpg gameplay contract: {name}"),
    }
}

fn build_provenance() -> GameplayModuleBuildProvenance {
    GameplayModuleBuildProvenance::from_build_inputs(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        &[include_bytes!("lib.rs")],
        include_bytes!("../../../Cargo.lock"),
        &[],
    )
}

fn module_ref() -> GameplayModuleRef {
    GameplayModuleRef {
        module_id: MODULE_ID.to_owned(),
        namespace: "rpg.pre-effect".to_owned(),
        version: "1.0.0".to_owned(),
        sdk_hash: "sha256:gameplay-sdk-v1".to_owned(),
        contract_hash: "sha256:rpg-pre-effect-contract-v1".to_owned(),
        artifact_hash: "sha256:rpg-pre-effect-artifact-v1".to_owned(),
        provider_id: PROVIDER_ID.to_owned(),
    }
}

fn state_owner() -> GameplayOwnerRef {
    GameplayOwnerRef {
        owner_id: "authority.rpg.pre-effect-state".to_owned(),
        provider_id: PROVIDER_ID.to_owned(),
    }
}

fn combat_owner() -> GameplayOwnerRef {
    GameplayOwnerRef {
        owner_id: OWNER_ID.to_owned(),
        provider_id: PROVIDER_ID.to_owned(),
    }
}

fn event_schema(name: &str) -> GameplayEventSchemaDeclaration {
    let event = contract(name);
    GameplayEventSchemaDeclaration {
        codec_id: gameplay_canonical_codec_id(&event.schema_hash),
        event,
    }
}

fn subscription(name: &str) -> GameplaySubscriptionDeclaration {
    GameplaySubscriptionDeclaration {
        subscription_id: format!("rpg.{name}.subscribe"),
        event: contract(name),
        invocation_id: format!("rpg.{name}.observe"),
        selector: GameplayHeaderSelector {
            source: None,
            target: None,
            scope: None,
            required_tags: Vec::new(),
        },
        max_deliveries_per_root: 2,
    }
}

fn observe_invocation(name: &str) -> GameplayInvocationDescriptor {
    GameplayInvocationDescriptor {
        invocation_id: format!("rpg.{name}.observe"),
        family: GameplayInvocationFamily::Observe,
        input_contract: contract(name),
        output_contract: contract(name),
        read_requirements: state_read_requirements(),
        max_outputs: 1,
        max_payload_bytes: 4_096,
    }
}

fn state_read_requirements() -> Vec<GameplayInvocationReadRequirement> {
    vec![GameplayInvocationReadRequirement {
        request_id: STATE_READ_ID.to_owned(),
        view: contract("reaction-state-view"),
    }]
}

fn codec<T>(event: GameplayContractRef) -> GameplayEventCodecRegistration
where
    T: Serialize + for<'de> Deserialize<'de> + 'static,
{
    let descriptor = schema_descriptor(&event.name);
    gameplay_serde_json_codec_registration::<T>(event, descriptor)
}

const SCENE_JSON: &str = r#"{
  "schemaVersion": 1,
  "id": 1,
  "metadata": { "name": "rpg-gameplay-host", "authoringFormatVersion": 1 },
  "dependencies": [],
  "nodes": [
    { "id": 1, "parent": null, "childOrder": 0, "label": null, "tags": [], "transform": { "translation": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1] }, "kind": { "kind": "emptyGroup" } }
  ]
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Owner {
        revision: u64,
        commits: Vec<PreEffectWorkspace>,
    }

    impl RpgPreEffectOwner for Owner {
        fn revision_hash(&self) -> String {
            format!("rpg:{:016x}", self.revision)
        }

        fn validate_commit(&self, _workspace: &PreEffectWorkspace) -> Result<(), Vec<String>> {
            Ok(())
        }

        fn commit(&mut self, workspace: &PreEffectWorkspace) -> Vec<String> {
            self.commits.push(workspace.clone());
            self.revision = self.revision.saturating_add(1);
            vec![gameplay_module_payload_hash(
                &serde_json::to_vec(workspace).unwrap(),
            )]
        }
    }

    #[test]
    fn real_module_suspends_records_state_transforms_and_restores() {
        let mut fabric = RpgGameplayFabric::new();
        let mut owner = Owner::default();
        let pending = fabric
            .begin_before_effect(
                PreEffectWorkspace {
                    decision_id: "step-1".to_owned(),
                    actor_id: "adept".to_owned(),
                    target_id: "raider".to_owned(),
                    action_id: "hexing-bolt".to_owned(),
                    damage_amount: 9,
                    damage_type: "psychic".to_owned(),
                },
                owner.revision_hash(),
            )
            .unwrap();
        assert_eq!(fabric.readout().pending_decision_count, 1);
        assert!(fabric.readout().reaction_frame_hashes.is_empty());

        let snapshot = fabric.snapshot();
        let mut restored = RpgGameplayFabric::restore(&snapshot).unwrap();
        let receipt = restored
            .resolve_before_effect(&pending, true, Some("raider.ward".to_owned()), &mut owner)
            .unwrap();
        assert_eq!(receipt.status, GameplayDecisionStatus::Accepted);
        assert_eq!(owner.commits.len(), 1);
        assert_eq!(owner.commits[0].damage_amount, 7);
        assert_eq!(restored.readout().pending_decision_count, 0);
        assert_eq!(restored.readout().reaction_frame_hashes.len(), 1);
        assert!(restored.readout().decisions[1].routing_hash.is_some());

        let final_snapshot = restored.snapshot();
        let final_restored = RpgGameplayFabric::restore(&final_snapshot).unwrap();
        assert_eq!(final_restored.readout(), restored.readout());
    }

    #[test]
    fn stale_owner_rejects_before_recording_resolution_state() {
        let mut fabric = RpgGameplayFabric::new();
        let mut owner = Owner::default();
        let pending = fabric
            .begin_before_effect(
                PreEffectWorkspace {
                    decision_id: "stale-step".to_owned(),
                    actor_id: "adept".to_owned(),
                    target_id: "raider".to_owned(),
                    action_id: "hexing-bolt".to_owned(),
                    damage_amount: 9,
                    damage_type: "psychic".to_owned(),
                },
                owner.revision_hash(),
            )
            .unwrap();
        owner.revision = 1;
        let before = fabric.readout();

        let error = fabric
            .resolve_before_effect(&pending, true, Some("raider.ward".to_owned()), &mut owner)
            .expect_err("stale owner must fail before observing resolution");

        assert!(error.contains("stale Rpg owner revision"));
        assert_eq!(fabric.readout(), before);
        assert!(owner.commits.is_empty());
    }

    #[test]
    fn consumed_resume_token_cannot_mutate_state_frames_or_readout() {
        let mut fabric = RpgGameplayFabric::new();
        let mut owner = Owner::default();
        let pending = fabric
            .begin_before_effect(
                PreEffectWorkspace {
                    decision_id: "replayed-step".to_owned(),
                    actor_id: "adept".to_owned(),
                    target_id: "raider".to_owned(),
                    action_id: "hexing-bolt".to_owned(),
                    damage_amount: 9,
                    damage_type: "psychic".to_owned(),
                },
                owner.revision_hash(),
            )
            .unwrap();
        let first = fabric
            .resolve_before_effect(&pending, true, Some("raider.ward".to_owned()), &mut owner)
            .unwrap();
        assert!(first.accepted());
        assert_eq!(owner.commits.len(), 1);

        owner.revision = 0;
        let before_readout = fabric.readout();
        let before_snapshot = fabric.snapshot();
        let replay = fabric
            .resolve_before_effect(&pending, true, Some("raider.ward".to_owned()), &mut owner)
            .unwrap();

        assert!(!replay.accepted());
        assert_eq!(owner.commits.len(), 1);
        assert_eq!(fabric.readout(), before_readout);
        let after_snapshot = fabric.snapshot();
        assert_eq!(
            after_snapshot.checkpoint.readout().runtime_session_hash,
            before_snapshot.checkpoint.readout().runtime_session_hash,
        );
    }
}
