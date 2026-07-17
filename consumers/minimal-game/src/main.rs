use asha_rpg::{
    compile_normalized_rpg_json, DeterministicRandomStream, GridPosition, PreEffectWorkspace,
    RpgAuthoritySession, RpgCapabilityState, RpgEntityState, RpgIntent, RpgPreEffectOwner, Team,
};

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
        vec![format!(
            "minimal-game-fact:damage={}",
            workspace.damage_amount
        )]
    }
}

fn main() {
    run_authority_session();
    println!("minimal consumer accepted semantic damage=7 and reaction damage=7");
}

fn run_authority_session() {
    let source = br#"{
      "schema":{"identity":"asha.rpg.ir","major":1},
      "package":{"id":"minimal.game","version":"1.0.0"},
      "catalogs":{
        "defenses":["guard"],
        "capabilities":["capability.vitality","capability.defenses","capability.random"]
      },
      "requirements":[
        {"kind":"operation","id":"operation.damage","version":1},
        {"kind":"capability","id":"capability.vitality","version":1},
        {"kind":"capability","id":"capability.defenses","version":1},
        {"kind":"capability","id":"capability.random","version":1}
      ],
      "actions":[{
        "id":"minimal.strike","name":"Minimal Strike","sourcePath":"actions/minimal-strike",
        "targets":{"team":"hostile","maximumRange":3,"maximumTargets":1},
        "check":{"kind":"attack","modifier":{"kind":"constant","value":2},"defenseId":"guard"},
        "rollScope":"perTarget","costs":[],
        "program":{"kind":"atomic","body":{"kind":"onCheck","hit":
          {"kind":"operation","operation":{"kind":"damage","amount":{"kind":"constant","value":7},"damageType":"force"}}
        }}
      }]
    }"#;
    let ruleset = compile_normalized_rpg_json(source).expect("consumer RPG IR compiles");
    let actor = RpgEntityState::new("hero", Team::Ally, GridPosition { x: 0, y: 0 }, 20);
    let target = RpgEntityState::new("guardian", Team::Enemy, GridPosition { x: 1, y: 0 }, 20)
        .with_defense("guard", 12);
    let mut state = RpgCapabilityState::default();
    state.insert_entity(actor);
    state.insert_entity(target);
    let mut session =
        RpgAuthoritySession::new(ruleset, state, DeterministicRandomStream::new(vec![10]));
    let receipt = session
        .submit(&RpgIntent {
            action_id: "minimal.strike".to_owned(),
            actor_id: "hero".to_owned(),
            target_ids: vec!["guardian".to_owned()],
        })
        .expect("consumer intent resolves");

    assert_eq!(receipt.random_consumed, 1);
    assert_eq!(
        session
            .state()
            .entity("guardian")
            .expect("target view remains available")
            .vitality()
            .current,
        13
    );
    let mut game = GameAuthority::default();
    let continuation = session
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

    let receipt = session
        .resolve_before_effect(
            &continuation,
            true,
            Some("guardian.ward".to_owned()),
            &mut game,
        )
        .expect("the consumer-owned commit is accepted");

    assert!(receipt.accepted());
    assert_eq!(game.committed_damage, Some(7));
    assert_eq!(session.gameplay_fabric_readout().pending_decision_count, 0);
}
