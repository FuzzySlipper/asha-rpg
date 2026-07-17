use asha_rpg::{PreEffectWorkspace, RpgGameplayFabric, RpgPreEffectOwner};

#[derive(Default)]
struct GameAuthority {
    revision: u64,
    committed_damage: Option<u32>,
}

impl RpgPreEffectOwner for GameAuthority {
    fn revision_hash(&self) -> String {
        format!("minimal-game:{:016x}", self.revision)
    }

    fn validate_commit(&self, workspace: &PreEffectWorkspace) -> Result<(), Vec<String>> {
        (workspace.damage_amount <= 20)
            .then_some(())
            .ok_or_else(|| vec!["damageOutOfConsumerRange".to_owned()])
    }

    fn commit(&mut self, workspace: &PreEffectWorkspace) -> Vec<String> {
        self.committed_damage = Some(workspace.damage_amount);
        self.revision = self.revision.saturating_add(1);
        vec![format!("minimal-game-fact:damage={}", workspace.damage_amount)]
    }
}

fn main() {
    let mut game = GameAuthority::default();
    let mut fabric = RpgGameplayFabric::new();
    let continuation = fabric
        .begin_before_effect(
            PreEffectWorkspace {
                decision_id: "turn-1".to_owned(),
                actor_id: "hero".to_owned(),
                target_id: "guardian".to_owned(),
                action_id: "arc-bolt".to_owned(),
                damage_amount: 9,
                damage_type: "arcane".to_owned(),
            },
            game.revision_hash(),
        )
        .expect("the portable authority loop suspends at its reaction point");

    let receipt = fabric
        .resolve_before_effect(
            &continuation,
            true,
            Some("guardian.ward".to_owned()),
            &mut game,
        )
        .expect("the consumer-owned commit is accepted");

    assert!(receipt.accepted());
    assert_eq!(game.committed_damage, Some(7));
    assert_eq!(fabric.readout().pending_decision_count, 0);
    println!("minimal consumer accepted damage=7");
}
