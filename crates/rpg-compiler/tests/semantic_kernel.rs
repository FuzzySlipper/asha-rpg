use rpg_compiler::{compile_normalized_rpg_json, RpgDiagnosticStage};
use rpg_core::{
    DeterministicRandomStream, GridPosition, RpgCapabilityState, RpgDomainEvent, RpgEntityState,
    RpgIntent, Team,
};

#[test]
fn attack_resolution_is_deterministic_and_owner_mutated() {
    let ruleset = compile_normalized_rpg_json(single_target_source().as_bytes()).unwrap();
    let initial_state = single_target_state();
    let intent = RpgIntent {
        action_id: "action.attack".to_owned(),
        actor_id: "actor".to_owned(),
        target_ids: vec!["target".to_owned()],
    };

    let mut first_state = initial_state.clone();
    let mut first_random = DeterministicRandomStream::new(vec![12, 3, 4]);
    let first = ruleset
        .resolve(&mut first_state, &mut first_random, &intent)
        .unwrap();

    let mut second_state = initial_state;
    let mut second_random = DeterministicRandomStream::new(vec![12, 3, 4]);
    let second = ruleset
        .resolve(&mut second_state, &mut second_random, &intent)
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(first_state, second_state);
    assert_eq!(first.random_consumed, 3);
    assert_eq!(first.state_revision, 1);
    assert_eq!(
        first_state
            .entity("actor")
            .unwrap()
            .resource("focus")
            .unwrap()
            .current,
        1
    );
    assert_eq!(first_state.entity("target").unwrap().vitality().current, 12);
    let modifier = first_state
        .entity("target")
        .unwrap()
        .modifier("impeded")
        .unwrap();
    assert_eq!(modifier.value(), -2);
    assert_eq!(modifier.remaining_turns(), 2);
    assert!(matches!(
        first.events[0],
        RpgDomainEvent::ResourceSpent { .. }
    ));
    assert!(matches!(
        first.events[1],
        RpgDomainEvent::AttackResolved { hit: true, .. }
    ));
    assert!(matches!(
        first.events[2],
        RpgDomainEvent::DamageApplied { amount: 8, .. }
    ));
    assert!(matches!(
        first.events[3],
        RpgDomainEvent::ModifierApplied { .. }
    ));
}

#[test]
fn multi_target_saves_select_independent_bounded_branches() {
    let ruleset = compile_normalized_rpg_json(multi_target_source().as_bytes()).unwrap();
    let mut state = multi_target_state();
    let mut random = DeterministicRandomStream::new(vec![10, 15]);
    let receipt = ruleset
        .resolve(
            &mut state,
            &mut random,
            &RpgIntent {
                action_id: "action.save".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target-b".to_owned(), "target-a".to_owned()],
            },
        )
        .unwrap();

    assert_eq!(receipt.target_ids, vec!["target-a", "target-b"]);
    assert_eq!(state.entity("target-a").unwrap().vitality().current, 15);
    assert_eq!(state.entity("target-b").unwrap().vitality().current, 18);
    assert_eq!(
        state
            .entity("actor")
            .unwrap()
            .resource("charge")
            .unwrap()
            .current,
        1
    );
    assert_eq!(receipt.random_consumed, 2);
}

#[test]
fn compiler_rejects_unknown_references_before_execution() {
    let invalid =
        single_target_source().replacen("\"statId\":\"power\"", "\"statId\":\"missing\"", 1);
    let failure = compile_normalized_rpg_json(invalid.as_bytes()).unwrap_err();

    assert!(failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.stage == RpgDiagnosticStage::References
            && diagnostic.code == "RPG_IR_REFERENCE_UNKNOWN"
            && diagnostic.message.contains("missing")
    }));
}

#[test]
fn compiler_rejects_non_atomic_programs_and_incompatible_requirements() {
    let failure = compile_normalized_rpg_json(non_atomic_source().as_bytes()).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "RPG_IR_ATOMIC_ROOT_REQUIRED"));

    let incompatible = single_target_source().replacen(
        "\"id\":\"operation.damage\",\"version\":1",
        "\"id\":\"operation.damage\",\"version\":99",
        1,
    );
    let failure = compile_normalized_rpg_json(incompatible.as_bytes()).unwrap_err();
    assert!(failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.stage == RpgDiagnosticStage::Requirements
            && diagnostic.code == "RPG_IR_REQUIREMENT_UNSUPPORTED"
            && diagnostic.requirement.as_deref() == Some("operation.damage@99")
    }));
}

#[test]
fn late_random_failure_rolls_back_state_and_random_together() {
    let ruleset = compile_normalized_rpg_json(single_target_source().as_bytes()).unwrap();
    let mut state = single_target_state();
    let original_state = state.clone();
    let mut random = DeterministicRandomStream::new(vec![12, 7]);
    let intent = RpgIntent {
        action_id: "action.attack".to_owned(),
        actor_id: "actor".to_owned(),
        target_ids: vec!["target".to_owned()],
    };

    let rejection = ruleset
        .resolve(&mut state, &mut random, &intent)
        .unwrap_err();

    assert_eq!(rejection.code, "RPG_RANDOM_VALUE_OUT_OF_RANGE");
    assert_eq!(rejection.random_attempted, 2);
    assert_eq!(state, original_state);
    assert_eq!(random.consumed(), 0);
}

fn single_target_state() -> RpgCapabilityState {
    let actor = RpgEntityState::new("actor", Team::Ally, GridPosition { x: 0, y: 0 }, 20)
        .with_stat("power", 4)
        .with_resource("focus", 2, 2);
    let target = RpgEntityState::new("target", Team::Enemy, GridPosition { x: 2, y: 0 }, 20)
        .with_defense("guard", 15);
    let mut state = RpgCapabilityState::default();
    state.insert_entity(actor);
    state.insert_entity(target);
    state
}

fn multi_target_state() -> RpgCapabilityState {
    let actor = RpgEntityState::new("actor", Team::Ally, GridPosition { x: 0, y: 0 }, 20)
        .with_resource("charge", 3, 3);
    let target_a = RpgEntityState::new("target-a", Team::Enemy, GridPosition { x: 1, y: 0 }, 20)
        .with_defense("resolve", 2);
    let target_b = RpgEntityState::new("target-b", Team::Enemy, GridPosition { x: 2, y: 0 }, 20)
        .with_defense("resolve", 2);
    let mut state = RpgCapabilityState::default();
    state.insert_entity(actor);
    state.insert_entity(target_a);
    state.insert_entity(target_b);
    state
}

fn single_target_source() -> String {
    r#"{
      "schema":{"identity":"asha.rpg.ir","major":1},
      "package":{"id":"consumer.package","version":"1.0.0"},
      "catalogs":{
        "stats":["power"],"defenses":["guard"],"resources":["focus"],
        "modifiers":["impeded"],
        "capabilities":["capability.vitality","capability.stats","capability.defenses","capability.resources","capability.modifiers","capability.random"]
      },
      "requirements":[
        {"kind":"operation","id":"operation.damage","version":1},
        {"kind":"operation","id":"operation.applyModifier","version":1},
        {"kind":"capability","id":"capability.vitality","version":1},
        {"kind":"capability","id":"capability.stats","version":1},
        {"kind":"capability","id":"capability.defenses","version":1},
        {"kind":"capability","id":"capability.resources","version":1},
        {"kind":"capability","id":"capability.modifiers","version":1},
        {"kind":"capability","id":"capability.random","version":1}
      ],
      "actions":[{
        "id":"action.attack","name":"action.attack","sourcePath":"actions/attack",
        "targets":{"team":"hostile","maximumRange":5,"maximumTargets":1},
        "check":{"kind":"attack","modifier":{"kind":"readStat","subject":"actor","statId":"power"},"defenseId":"guard"},
        "rollScope":"perTarget","costs":[{"resourceId":"focus","amount":1}],
        "program":{"kind":"atomic","body":{"kind":"onCheck","hit":{"kind":"sequence","steps":[
          {"kind":"operation","operation":{"kind":"damage","amount":{"kind":"dice","count":2,"sides":6,"bonus":1},"damageType":"force"}},
          {"kind":"when","predicate":{"kind":"compare","left":{"kind":"readStat","subject":"actor","statId":"power"},"comparison":"greaterThanOrEqual","right":{"kind":"constant","value":4}},"then":
            {"kind":"operation","operation":{"kind":"applyModifier","modifierId":"impeded","stackingGroup":"movement-control","stacking":"refresh","value":{"kind":"constant","value":-2},"durationTurns":2}}
          }
        ]}}}
      }]
    }"#
    .to_owned()
}

fn multi_target_source() -> String {
    r#"{
      "schema":{"identity":"asha.rpg.ir","major":1},
      "package":{"id":"consumer.package","version":"1.0.0"},
      "catalogs":{
        "defenses":["resolve"],"resources":["charge"],
        "capabilities":["capability.vitality","capability.defenses","capability.resources","capability.random"]
      },
      "requirements":[
        {"kind":"operation","id":"operation.damage","version":1},
        {"kind":"operation","id":"operation.changeResource","version":1},
        {"kind":"capability","id":"capability.vitality","version":1},
        {"kind":"capability","id":"capability.defenses","version":1},
        {"kind":"capability","id":"capability.resources","version":1},
        {"kind":"capability","id":"capability.random","version":1}
      ],
      "actions":[{
        "id":"action.save","name":"action.save","sourcePath":"actions/save",
        "targets":{"team":"hostile","maximumRange":5,"maximumTargets":2},
        "check":{"kind":"savingThrow","difficulty":{"kind":"constant","value":14},"defenseId":"resolve"},
        "rollScope":"perTarget","costs":[],
        "program":{"kind":"atomic","body":{"kind":"forEachTarget","maximum":2,"body":{"kind":"sequence","steps":[
          {"kind":"onCheck",
            "saved":{"kind":"operation","operation":{"kind":"damage","amount":{"kind":"half","value":{"kind":"constant","value":5}},"damageType":"force"}},
            "failed":{"kind":"operation","operation":{"kind":"damage","amount":{"kind":"constant","value":5},"damageType":"force"}}
          },
          {"kind":"operation","operation":{"kind":"changeResource","subject":"actor","resourceId":"charge","delta":{"kind":"constant","value":-1}}}
        ]}}}
      }]
    }"#
    .to_owned()
}

fn non_atomic_source() -> String {
    r#"{
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
        "program":{"kind":"operation","operation":{"kind":"heal","amount":{"kind":"constant","value":4}}}
      }]
    }"#
    .to_owned()
}
