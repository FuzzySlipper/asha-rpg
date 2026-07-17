use rpg_compiler::CompiledRpgRuleset;
use rpg_core::{
    DeterministicRandomStream, RpgCapabilityState, RpgIntent, RpgResolutionReceipt,
    RpgResolutionRejection,
};

/// Owner of one compiled ruleset's private capability and random state.
#[derive(Debug, Clone)]
pub struct RpgAuthoritySession {
    ruleset: CompiledRpgRuleset,
    state: RpgCapabilityState,
    random: DeterministicRandomStream,
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

    pub fn submit(
        &mut self,
        intent: &RpgIntent,
    ) -> Result<RpgResolutionReceipt, RpgResolutionRejection> {
        self.ruleset
            .resolve(&mut self.state, &mut self.random, intent)
    }
}
