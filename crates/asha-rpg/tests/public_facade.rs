use asha_rpg::{
    compile_normalized_rpg_json, DeterministicRandomStream, GridPosition, RpgAuthoritySession,
    RpgCapabilityState, RpgEntityState, RpgIntent, Team,
};

#[test]
fn public_facade_compiles_and_executes_a_no_roll_action() {
    let source = br#"{
      "schema":{"identity":"asha.rpg.ir","major":1},
      "package":{"id":"consumer.package","version":"1.0.0"},
      "catalogs":{"capabilities":["capability.vitality"]},
      "requirements":[
        {"kind":"operation","id":"operation.heal","version":1},
        {"kind":"capability","id":"capability.vitality","version":1}
      ],
      "actions":[{
        "id":"action.heal","name":"action.heal","sourcePath":"actions/heal",
        "targets":{"team":"ally","maximumRange":3,"maximumTargets":1},
        "check":{"kind":"noRoll"},"rollScope":"none","costs":[],
        "program":{"kind":"atomic","body":{"kind":"onCheck","noRoll":
          {"kind":"operation","operation":{"kind":"heal","amount":{"kind":"constant","value":4}}}
        }}
      }]
    }"#;
    let ruleset = compile_normalized_rpg_json(source).unwrap();
    assert_eq!(
        ruleset.required_capabilities().collect::<Vec<_>>(),
        vec![("capability.vitality", 1)]
    );

    let actor = RpgEntityState::new("actor", Team::Ally, GridPosition { x: 0, y: 0 }, 20);
    let target = RpgEntityState::new("target", Team::Ally, GridPosition { x: 1, y: 0 }, 20);
    let mut state = RpgCapabilityState::default();
    state.insert_entity(actor);
    state.insert_entity(target);
    state.vitality_owner().apply_damage("target", 7).unwrap();
    let mut session =
        RpgAuthoritySession::new(ruleset, state, DeterministicRandomStream::new(Vec::new()));

    let receipt = session
        .submit(&RpgIntent {
            action_id: "action.heal".to_owned(),
            actor_id: "actor".to_owned(),
            target_ids: vec!["target".to_owned()],
        })
        .unwrap();

    assert_eq!(receipt.random_consumed, 0);
    assert_eq!(
        session.state().entity("target").unwrap().vitality().current,
        17
    );
}
