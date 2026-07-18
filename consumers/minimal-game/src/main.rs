use asha_rpg::{
    compile_normalized_rpg_json, GridPosition, RpgAuthorityCommand, RpgAuthoritySession,
    RpgCapabilityState, RpgCommandOutcome, RpgEntityState, RpgIntent, RpgReactionCommand, Team,
};

fn main() {
    run_authority_session();
    println!("minimal consumer resumed one staged reaction and committed damage=5");
}

fn run_authority_session() {
    let source = br#"{
      "schema":{"identity":"asha.rpg.ir","major":1},
      "package":{"id":"minimal.game","version":"1.0.0"},
      "catalogs":{
        "defenses":["guard"],
        "capabilities":["capability.vitality","capability.defenses","capability.random","capability.reactions"]
      },
      "requirements":[
        {"kind":"operation","id":"operation.damage","version":1},
        {"kind":"operation","id":"operation.openReaction","version":1},
        {"kind":"capability","id":"capability.vitality","version":1},
        {"kind":"capability","id":"capability.defenses","version":1},
        {"kind":"capability","id":"capability.random","version":1},
        {"kind":"capability","id":"capability.reactions","version":1}
      ],
      "actions":[{
        "id":"minimal.strike","name":"Minimal Strike","sourcePath":"actions/minimal-strike",
        "targets":{"team":"hostile","maximumRange":3,"maximumTargets":1},
        "check":{"kind":"attack","modifier":{"kind":"constant","value":2},"defenseId":"guard"},
        "rollScope":"perTarget","costs":[],
        "program":{"kind":"atomic","body":{"kind":"onCheck","hit":
          {"kind":"sequence","steps":[
            {"kind":"operation","operation":{"kind":"openReaction","reactionId":"minimal.ward","options":[
              {"id":"ward","label":"Raise ward","damageReduction":2}
            ]}},
            {"kind":"operation","operation":{"kind":"damage","amount":{"kind":"constant","value":7},"damageType":"force"}}
          ]}
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
    let mut session = RpgAuthoritySession::new(ruleset, state);

    let suspended = session.submit(RpgAuthorityCommand {
        expected_revision: 0,
        intent: RpgIntent {
            action_id: "minimal.strike".to_owned(),
            actor_id: "hero".to_owned(),
            target_ids: vec!["guardian".to_owned()],
        },
        random_values: vec![10],
    });
    let RpgCommandOutcome::AwaitingReaction(pending) = suspended else {
        panic!("consumer command should suspend: {suspended:?}");
    };
    assert_eq!(session.state().revision(), 0);

    let resumed = session.react(RpgReactionCommand {
        expected_revision: 0,
        reaction_id: pending.request.reaction_id,
        option_id: Some("ward".to_owned()),
        additional_random_values: Vec::new(),
    });
    let RpgCommandOutcome::Accepted(receipt) = resumed else {
        panic!("consumer reaction should commit: {resumed:?}");
    };
    assert_eq!(receipt.random_consumed, 1);
    assert_eq!(
        session.state().entity("guardian").unwrap().vitality().current,
        15
    );
}
