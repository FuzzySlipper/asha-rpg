use std::collections::VecDeque;
use std::io::{self, Read};

use asha_rpg::{
    compile_prepared_play_bundle_json, materialized_definition_fingerprint, BoundedValue,
    GridPosition, MaterializedContentDefinition, RpgActionProposal, RpgAuthoritySession,
    RpgBoardSetup, RpgCommandOutcome, RpgContributionDisposition, RpgDomainEvent,
    RpgEquipmentSlotSetup, RpgInitialCapability, RpgIntentItemBinding, RpgItemInstanceSetup,
    RpgNaturalDieEffect, RpgParticipantSetup, RpgRandomRequest, RpgRandomRequestKind,
    RpgRandomSource, RpgRandomSourceBinding, RpgRandomSourceFailure, RpgScenario, RpgTeamId,
    RpgTurnInitialization,
};
use serde_json::Value;

const ACTOR_ID: &str = "vanguard";
const FIRST_TARGET_ID: &str = "raider-a";
const SECOND_TARGET_ID: &str = "raider-b";

fn main() {
    let prepared_source = read_stdin();
    prove_prepared_input_rejects_tampering(&prepared_source);

    let bundle = compile_prepared_play_bundle_json(&prepared_source)
        .expect("compile the exact TypeScript-authored prepared bundle");
    let scenario = scenario(&bundle, 5);
    let mut session =
        RpgAuthoritySession::from_scenario(bundle.clone(), scenario.clone()).expect("scenario");
    let initial_checkpoint = session.checkpoint().expect("initial checkpoint");

    let view = session.encounter_view();
    assert_eq!(view.state_revision, 0);
    assert_eq!(budget_remaining(&view, "standard"), 1);
    assert_eq!(budget_remaining(&view, "bonus"), 1);
    assert_eq!(budget_remaining(&view, "reaction"), 1);
    let bindings = view
        .actions
        .iter()
        .filter(|action| action.definition_id == "action.core-attack")
        .map(|action| {
            action
                .item_binding
                .clone()
                .expect("bound core attack exposes exact item identity")
        })
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.item_definition_id.as_str())
            .collect::<Vec<_>>(),
        ["item.long-spear", "item.short-blade"]
    );
    let spear_binding = bindings
        .iter()
        .find(|binding| binding.item_definition_id == "item.long-spear")
        .cloned()
        .expect("long spear binding");

    let mut expose_source = ScriptedSource::new(&session, []);
    let (expose_outcome, expose_replay) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.expose".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut expose_source,
        )
        .expect("unopposed expose action");
    let expose_receipt = accepted(expose_outcome);
    assert!(expose_receipt.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::EffectApplied {
            target_id,
            definition_id,
            ..
        } if target_id == FIRST_TARGET_ID && definition_id == "effect.exposed"
    )));
    assert_eq!(expose_source.requests.len(), 0);

    let mut attack_source =
        ScriptedSource::new(&session, [vec![20], vec![5], vec![4], vec![5]]);
    let (attack_outcome, attack_replay) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 1,
                action_id: "action.core-attack".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![
                    FIRST_TARGET_ID.to_owned(),
                    SECOND_TARGET_ID.to_owned(),
                ],
                item_binding: Some(spear_binding.clone()),
            },
            &mut attack_source,
        )
        .expect("bound multi-target attack");
    let attack_receipt = accepted(attack_outcome);
    assert_eq!(
        attack_source
            .requests
            .iter()
            .map(|request| (request.kind, request.count, request.sides))
            .collect::<Vec<_>>(),
        [
            (RpgRandomRequestKind::ScalarTest, 1, 20),
            (RpgRandomRequestKind::ScalarTest, 1, 20),
            (RpgRandomRequestKind::FormulaDice, 1, 8),
            (RpgRandomRequestKind::FormulaDice, 1, 8),
        ]
    );

    let scalar_events = attack_receipt
        .events
        .iter()
        .filter_map(|event| match event {
            RpgDomainEvent::ScalarTestResolved {
                target_id,
                natural_die_resolution,
                final_band_id,
                contribution_ledger,
                ..
            } => Some((
                target_id.as_str(),
                natural_die_resolution,
                final_band_id.as_str(),
                contribution_ledger,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scalar_events
            .iter()
            .map(|(target, _, band, _)| (*target, *band))
            .collect::<Vec<_>>(),
        [(FIRST_TARGET_ID, "critical"), (SECOND_TARGET_ID, "miss")]
    );
    assert!(matches!(
        scalar_events[0].1.as_ref().map(|resolution| &resolution.effect),
        Some(RpgNaturalDieEffect::SetBand { band_id }) if band_id == "critical"
    ));

    let first_ledger = scalar_events[0].3;
    assert!(first_ledger.candidates.iter().any(|decision| {
        decision.source_definition_id == "effect.exposed"
            && decision.contribution_id == "exposed-opening"
            && decision.applied_value == 1
            && matches!(decision.disposition, RpgContributionDisposition::Applied)
    }));
    assert!(first_ledger.candidates.iter().any(|decision| {
        decision.source_definition_id == "feature.tactical-training"
            && decision.contribution_id == "training"
            && decision.applied_value == 2
            && matches!(decision.disposition, RpgContributionDisposition::Applied)
    }));
    assert!(first_ledger.candidates.iter().any(|decision| {
        decision.source_definition_id == "feature.tactical-training"
            && decision.contribution_id == "surrounded-resolve"
            && matches!(
                decision.disposition,
                RpgContributionDisposition::Inapplicable { .. }
            )
    }));
    assert!(first_ledger.candidates.iter().any(|decision| {
        decision.source_definition_id == "item.long-spear"
            && decision.contribution_id == "weapon-accuracy"
            && matches!(
                decision.disposition,
                RpgContributionDisposition::Suppressed { .. }
            )
    }));

    let damage_targets = attack_receipt
        .events
        .iter()
        .filter_map(|event| match event {
            RpgDomainEvent::DamagePacketApplied {
                target_id,
                actual_vitality_delta,
                ..
            } => Some((target_id.as_str(), *actual_vitality_delta)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        damage_targets,
        [
            (FIRST_TARGET_ID, 4),
            (FIRST_TARGET_ID, 5),
            (SECOND_TARGET_ID, 1),
        ]
    );
    assert_eq!(
        attack_receipt
            .events
            .iter()
            .filter(|event| matches!(
                event,
                RpgDomainEvent::ResourceSpent {
                    resource_id,
                    amount: 2,
                    ..
                } if resource_id == "focus"
            ))
            .count(),
        1,
        "multi-target action pays Focus once"
    );
    assert_eq!(
        attack_receipt
            .events
            .iter()
            .filter(|event| matches!(
                event,
                RpgDomainEvent::ActivationBudgetSpent {
                    budget_id,
                    amount: 1,
                    ..
                } if budget_id == "standard"
            ))
            .count(),
        1,
        "multi-target action pays its Standard activation once"
    );

    let final_view = session.encounter_view();
    assert_eq!(final_view.state_revision, 2);
    assert_eq!(budget_remaining(&final_view, "standard"), 0);
    assert_eq!(budget_remaining(&final_view, "bonus"), 0);
    assert_eq!(budget_remaining(&final_view, "reaction"), 1);
    assert_eq!(resource_current(&final_view, ACTOR_ID, "focus"), 3);
    assert_eq!(vitality(&final_view, FIRST_TARGET_ID), 21);
    assert_eq!(vitality(&final_view, SECOND_TARGET_ID), 29);
    assert_eq!(final_view.log.len(), 2);

    prove_stale_and_binding_rejections_are_atomic(&mut session, &spear_binding);
    prove_unaffordable_action_is_atomic(bundle.clone());
    prove_malformed_random_evidence_is_atomic(bundle.clone());

    let replayed = RpgAuthoritySession::replay(
        initial_checkpoint,
        &[expose_replay, attack_replay],
    )
    .expect("accepted sequence replays through ordinary authority paths");
    assert_eq!(replayed.state(), session.state());
    assert_eq!(
        replayed.state_hash().expect("replay hash"),
        session.state_hash().expect("recorded hash")
    );
    assert_eq!(
        replayed.accepted_random_values(),
        session.accepted_random_values()
    );
    assert_eq!(replayed.encounter_view().log, session.encounter_view().log);

    println!(
        "core witness compiled, rejected tampering, resolved exact bindings and ledgers, and replayed at {}",
        session.state_hash().expect("final hash").value
    );
}

fn prove_prepared_input_rejects_tampering(prepared_source: &[u8]) {
    let mut unknown_band: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    unknown_band["ruleset"]["provides"]["scalarTestProfiles"][0]["naturalDieRules"][0]["effect"]
        ["bandId"] = Value::String("missing-band".to_owned());
    assert_compile_rejects(&unknown_band, "RULESET_SCALAR_TEST_NATURAL_RULE_INVALID");

    let mut duplicate_definition: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let duplicate = duplicate_definition["materializedDefinitions"][0].clone();
    duplicate_definition["materializedDefinitions"]
        .as_array_mut()
        .expect("definition array")
        .push(duplicate);
    assert_compile_rejects(
        &duplicate_definition,
        "CONTENT_PACK_DUPLICATE_MATERIALIZED_DEFINITION",
    );

    let mut incompatible_lock: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    incompatible_lock["dependencyLock"][0]["resolvedVersion"] =
        Value::String("2.0.0".to_owned());
    assert_compile_rejects(&incompatible_lock, "CONTENT_PACK_LOCK_SOURCE_MISMATCH");

    let mut invalid_domain: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let item = invalid_domain["materializedDefinitions"]
        .as_array_mut()
        .expect("definition array")
        .iter_mut()
        .find(|definition| definition["id"] == "item.long-spear")
        .expect("long spear definition");
    item["semantic"]["contributions"][0]["value"]["value"] =
        Value::Number(2_147_483_647_i64.into());
    let typed_item: MaterializedContentDefinition =
        serde_json::from_value(item.clone()).expect("decode tampered item definition");
    item["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_item)
            .expect("fingerprint tampered definition"),
    );
    assert_compile_rejects(&invalid_domain, "SCALAR_CONTRIBUTION_VALUE_OUT_OF_DOMAIN");
}

fn assert_compile_rejects(prepared: &Value, code: &str) {
    let failure = compile_prepared_play_bundle_json(
        &serde_json::to_vec(prepared).expect("serialize tampered prepared bundle"),
    )
    .expect_err("tampered prepared bundle must fail closed");
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "expected {code}, got {:?}",
        failure.diagnostics
    );
}

fn prove_stale_and_binding_rejections_are_atomic(
    session: &mut RpgAuthoritySession,
    spear_binding: &RpgIntentItemBinding,
) {
    let before = session.checkpoint().expect("checkpoint before rejection");
    let mut no_random = ScriptedSource::new(session, []);
    let (stale, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 1,
                action_id: "action.core-attack".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: Some(spear_binding.clone()),
            },
            &mut no_random,
        )
        .expect("stale command is an authority outcome");
    assert!(matches!(stale, RpgCommandOutcome::Rejected(_)));
    assert_eq!(session.checkpoint().expect("checkpoint after stale"), before);

    let mut wrong_definition = spear_binding.clone();
    wrong_definition.item_definition_id = "item.short-blade".to_owned();
    let mut no_random = ScriptedSource::new(session, []);
    let (rejected, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 2,
                action_id: "action.core-attack".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: Some(wrong_definition),
            },
            &mut no_random,
        )
        .expect("wrong exact item identity is an authority outcome");
    assert!(matches!(rejected, RpgCommandOutcome::Rejected(_)));
    assert_eq!(
        session.checkpoint().expect("checkpoint after binding rejection"),
        before
    );
}

fn prove_unaffordable_action_is_atomic(bundle: asha_rpg::CompiledPlayBundle) {
    let scenario = scenario(&bundle, 1);
    let mut session = RpgAuthoritySession::from_scenario(bundle, scenario).expect("scenario");
    let binding = session
        .encounter_view()
        .actions
        .iter()
        .find(|action| {
            action.definition_id == "action.core-attack"
                && action.item_binding.as_ref().is_some_and(|binding| {
                    binding.item_definition_id == "item.long-spear"
                })
        })
        .and_then(|action| action.item_binding.clone())
        .expect("bound action");
    let before = session.checkpoint().expect("checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let (rejected, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.core-attack".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: Some(binding),
            },
            &mut no_random,
        )
        .expect("unaffordable action is an authority outcome");
    assert!(matches!(rejected, RpgCommandOutcome::Rejected(_)));
    assert_eq!(session.checkpoint().expect("unchanged checkpoint"), before);
}

fn prove_malformed_random_evidence_is_atomic(bundle: asha_rpg::CompiledPlayBundle) {
    let scenario = scenario(&bundle, 5);
    let mut session = RpgAuthoritySession::from_scenario(bundle, scenario).expect("scenario");
    let binding = session
        .encounter_view()
        .actions
        .iter()
        .find(|action| {
            action.definition_id == "action.core-attack"
                && action.item_binding.as_ref().is_some_and(|binding| {
                    binding.item_definition_id == "item.long-spear"
                })
        })
        .and_then(|action| action.item_binding.clone())
        .expect("bound action");
    let before = session.checkpoint().expect("checkpoint");
    let mut malformed = ScriptedSource::new(&session, [vec![0]]);
    let failure = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.core-attack".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: Some(binding),
            },
            &mut malformed,
        )
        .expect_err("out-of-range evidence fails before authority commit");
    assert!(matches!(
        failure,
        asha_rpg::RpgAutomaticCommandFailure::RandomSource(
            asha_rpg::RpgRandomSourceFailure { ref code, .. }
        ) if code == "RPG_RANDOM_SOURCE_VALUE_OUT_OF_RANGE"
    ));
    assert_eq!(session.checkpoint().expect("unchanged checkpoint"), before);
}

fn accepted(outcome: RpgCommandOutcome) -> asha_rpg::RpgResolutionReceipt {
    match outcome {
        RpgCommandOutcome::Accepted(receipt) => receipt,
        other => panic!("expected accepted command, got {other:?}"),
    }
}

fn read_stdin() -> Vec<u8> {
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .expect("read prepared PlayBundle from stdin");
    source
}

fn scenario(bundle: &asha_rpg::CompiledPlayBundle, focus: i32) -> RpgScenario {
    RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 5,
            height: 3,
            cells: Vec::new(),
        },
        participants: vec![
            actor(focus),
            participant(
                FIRST_TARGET_ID,
                "Raider A",
                RpgTeamId::enemy(),
                GridPosition { x: 2, y: 1 },
            ),
            participant(
                SECOND_TARGET_ID,
                "Raider B",
                RpgTeamId::enemy(),
                GridPosition { x: 3, y: 1 },
            ),
        ],
        turn: RpgTurnInitialization {
            initiative_order: vec![
                ACTOR_ID.to_owned(),
                FIRST_TARGET_ID.to_owned(),
                SECOND_TARGET_ID.to_owned(),
            ],
            current_actor_id: ACTOR_ID.to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "core-witness.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "core-witness.scripted".to_owned(),
            source_version: 1,
        },
    }
}

fn actor(focus: i32) -> RpgParticipantSetup {
    let mut actor = participant(
        ACTOR_ID,
        "Vanguard",
        RpgTeamId::ally(),
        GridPosition { x: 1, y: 1 },
    );
    actor.definition_ids = vec![
        "action.core-attack".to_owned(),
        "action.expose".to_owned(),
        "action.rally".to_owned(),
    ];
    actor.class_definition_id = Some("class.vanguard".to_owned());
    actor.feature_definition_ids = vec!["feature.tactical-training".to_owned()];
    actor.items = vec![
        RpgItemInstanceSetup {
            id: "long-spear".to_owned(),
            definition_id: "item.long-spear".to_owned(),
        },
        RpgItemInstanceSetup {
            id: "short-blade".to_owned(),
            definition_id: "item.short-blade".to_owned(),
        },
    ];
    actor.equipment = vec![
        RpgEquipmentSlotSetup {
            slot_id: "hand.main".to_owned(),
            item_instance_id: "long-spear".to_owned(),
        },
        RpgEquipmentSlotSetup {
            slot_id: "hand.off".to_owned(),
            item_instance_id: "short-blade".to_owned(),
        },
    ];
    actor.capabilities.push(RpgInitialCapability::Resource {
        id: "focus".to_owned(),
        value: BoundedValue {
            current: focus,
            max: 5,
        },
    });
    actor
}

fn participant(
    id: &str,
    label: &str,
    team_id: RpgTeamId,
    position: GridPosition,
) -> RpgParticipantSetup {
    let mut capabilities = vec![RpgInitialCapability::Vitality {
        value: BoundedValue {
            current: 30,
            max: 30,
        },
    }];
    for (id, value) in [
        ("acuity", 3),
        ("conviction", 3),
        ("finesse", 3),
        ("intellect", 3),
        ("might", 5),
        ("spirit", 3),
    ] {
        capabilities.push(RpgInitialCapability::Stat {
            id: id.to_owned(),
            value,
        });
    }
    for (id, value) in [
        ("armor", 18),
        ("grit", 16),
        ("nerve", 15),
        ("wits", 15),
    ] {
        capabilities.push(RpgInitialCapability::Defense {
            id: id.to_owned(),
            value,
        });
    }
    RpgParticipantSetup {
        id: id.to_owned(),
        label: label.to_owned(),
        team_id,
        position,
        definition_ids: vec!["action.expose".to_owned()],
        class_definition_id: None,
        feature_definition_ids: Vec::new(),
        items: Vec::new(),
        equipment: Vec::new(),
        capabilities,
    }
}

fn budget_remaining(view: &asha_rpg::RpgEncounterView, budget_id: &str) -> i32 {
    view.participants
        .iter()
        .find(|participant| participant.id == ACTOR_ID)
        .and_then(|participant| {
            participant
                .activation_budgets
                .iter()
                .find(|budget| budget.id == budget_id)
        })
        .map(|budget| budget.remaining)
        .expect("budget readback")
}

fn resource_current(view: &asha_rpg::RpgEncounterView, actor_id: &str, resource_id: &str) -> i32 {
    view.participants
        .iter()
        .find(|participant| participant.id == actor_id)
        .and_then(|participant| {
            participant
                .resources
                .iter()
                .find(|resource| resource.id == resource_id)
        })
        .map(|resource| resource.value.current)
        .expect("resource readback")
}

fn vitality(view: &asha_rpg::RpgEncounterView, participant_id: &str) -> i32 {
    view.participants
        .iter()
        .find(|participant| participant.id == participant_id)
        .map(|participant| participant.vitality.current)
        .expect("participant vitality")
}

struct ScriptedSource {
    binding: RpgRandomSourceBinding,
    values: VecDeque<Vec<u32>>,
    requests: Vec<RpgRandomRequest>,
}

impl ScriptedSource {
    fn new<const N: usize>(
        session: &RpgAuthoritySession,
        values: [Vec<u32>; N],
    ) -> Self {
        Self {
            binding: session.scenario().random_source.clone(),
            values: values.into(),
            requests: Vec::new(),
        }
    }
}

impl RpgRandomSource for ScriptedSource {
    fn binding(&self) -> &RpgRandomSourceBinding {
        &self.binding
    }

    fn draw(&mut self, request: &RpgRandomRequest) -> Result<Vec<u32>, RpgRandomSourceFailure> {
        self.requests.push(request.clone());
        Ok(self.values.pop_front().unwrap_or_default())
    }
}
