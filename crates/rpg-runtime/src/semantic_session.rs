use rpg_compiler::CompiledRpgRuleset;
use rpg_core::{
    DeterministicRandomStream, RpgCapabilityState, RpgIntent, RpgResolutionReceipt,
    RpgResolutionRejection,
};

use crate::{
    PreEffectWorkspace, RpgGameplayContinuation, RpgGameplayFabric, RpgGameplayFabricReadout,
    RpgPreEffectOwner,
};
use asha_runtime_session_composition::GameplayDecisionReceipt;

/// Owner of one compiled ruleset's private capability and random state.
#[derive(Debug)]
pub struct RpgAuthoritySession {
    ruleset: CompiledRpgRuleset,
    state: RpgCapabilityState,
    random: DeterministicRandomStream,
    gameplay_fabric: RpgGameplayFabric,
}

impl RpgAuthoritySession {
    pub fn new(
        ruleset: CompiledRpgRuleset,
        initial_state: RpgCapabilityState,
        random: DeterministicRandomStream,
    ) -> Self {
        Self {
            ruleset,
            state: initial_state,
            random,
            gameplay_fabric: RpgGameplayFabric::new(),
        }
    }

    pub fn ruleset(&self) -> &CompiledRpgRuleset {
        &self.ruleset
    }

    pub fn state(&self) -> &RpgCapabilityState {
        &self.state
    }

    pub fn random_consumed(&self) -> usize {
        self.random.consumed()
    }

    pub fn supply_random(&mut self, values: impl IntoIterator<Item = u32>) {
        self.random.extend(values);
    }

    /// Resolve against cloned capability/random workspaces without committing.
    pub fn preview(
        &self,
        intent: &RpgIntent,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        let mut state = self.state.clone();
        let mut random = self.random.clone();
        self.ruleset.resolve(&mut state, &mut random, intent)
    }

    pub fn preview_with_random(
        &self,
        intent: &RpgIntent,
        values: impl IntoIterator<Item = u32>,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        let mut state = self.state.clone();
        let mut random = DeterministicRandomStream::new(values.into_iter().collect());
        self.ruleset.resolve(&mut state, &mut random, intent)
    }

    pub fn submit(
        &mut self,
        intent: &RpgIntent,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.ruleset
            .resolve(&mut self.state, &mut self.random, intent)
    }

    pub fn submit_with_random(
        &mut self,
        intent: &RpgIntent,
        values: impl IntoIterator<Item = u32>,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        let mut random = DeterministicRandomStream::new(values.into_iter().collect());
        self.ruleset.resolve(&mut self.state, &mut random, intent)
    }

    pub fn begin_before_effect(
        &mut self,
        workspace: PreEffectWorkspace,
        expected_owner_revision: String,
    ) -> Result<RpgGameplayContinuation, String> {
        self.gameplay_fabric
            .begin_before_effect(workspace, expected_owner_revision)
    }

    pub fn resolve_before_effect(
        &mut self,
        pending: &RpgGameplayContinuation,
        accepted: bool,
        option_id: Option<String>,
        owner: &mut dyn RpgPreEffectOwner,
    ) -> Result<GameplayDecisionReceipt, String> {
        self.gameplay_fabric
            .resolve_before_effect(pending, accepted, option_id, owner)
    }

    pub fn gameplay_fabric_readout(&self) -> RpgGameplayFabricReadout {
        self.gameplay_fabric.readout()
    }
}
