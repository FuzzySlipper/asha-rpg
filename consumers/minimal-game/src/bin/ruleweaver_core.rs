use std::collections::VecDeque;
use std::io::{self, Read};

use asha_rpg::{
    compile_prepared_play_bundle_json, materialized_definition_fingerprint, BoundedValue,
    GridPosition, MaterializedContentDefinition, RpgActionProposal, RpgAuthoritySession,
    RpgBoardSetup, RpgBoundActionProposal, RpgCellCapabilitySetup, RpgCellCapabilityValue,
    RpgCellSetup, RpgCommandOutcome, RpgContributionDisposition, RpgDomainEvent,
    RpgEquipmentSlotSetup, RpgInitialCapability, RpgIntentItemBinding, RpgItemInstanceSetup,
    RpgMovementKind, RpgNaturalDieEffect, RpgParticipantSetup, RpgRandomRequest,
    RpgRandomRequestKind, RpgRandomSource, RpgRandomSourceBinding, RpgRandomSourceFailure,
    RpgReactionProposal, RpgScenario, RpgTeamId, RpgTurnControl, RpgTurnControlProposal,
    RpgTurnInitialization,
    RPG_LINE_OF_EFFECT_OBSTRUCTION_ID,
    RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION,
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
    prove_fixed_spatial_source_lifecycle(bundle.clone());
    prove_movement_allowance_forced_choices_and_reactions(bundle.clone());
    prove_line_of_effect_projection_staleness_and_atomicity(bundle.clone());
    prove_condition_lanes_tenure_restrictions_and_replay(bundle.clone());
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

    let expose_binding = view
        .actions
        .iter()
        .find(|action| action.definition_id == "action.expose")
        .expect("expose action")
        .options
        .binding
        .clone();
    let mut expose_source = ScriptedSource::new(&session, []);
    let expose_submission = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: expose_binding,
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
            },
            &mut expose_source,
        )
        .expect("unopposed expose action");
    let expose_outcome = expose_submission.outcome;
    let expose_replay = expose_submission
        .replay_entry
        .expect("accepted bound action has replay entry");
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

fn prove_condition_lanes_tenure_restrictions_and_replay(
    bundle: asha_rpg::CompiledPlayBundle,
) {
    let mut setup = scenario(&bundle, 5);
    setup.participants[0].definition_ids.extend([
        "action.apply-condition".to_owned(),
        "action.condition-probe".to_owned(),
        "action.restricted".to_owned(),
    ]);
    for participant in &mut setup.participants[1..] {
        participant.definition_ids.extend([
            "action.condition-probe".to_owned(),
            "action.restricted".to_owned(),
            "action.shift".to_owned(),
        ]);
        participant.definition_ids.sort();
    }
    setup.participants[0].definition_ids.sort();
    let mut session =
        RpgAuthoritySession::from_scenario(bundle, setup).expect("condition scenario");
    let initial = session.checkpoint().expect("condition initial checkpoint");
    let mut replay_entries = Vec::new();

    let mut no_random = ScriptedSource::new(&session, []);
    let (applied, apply_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.apply-condition".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("apply save-ends effects");
    let applied = accepted(applied);
    assert_eq!(
        applied
            .events
            .iter()
            .filter(|event| matches!(event, RpgDomainEvent::EffectApplied { .. }))
            .count(),
        2
    );
    replay_entries.push(apply_entry);
    let mut duplicate_effect_identity =
        session.checkpoint().expect("checkpoint with active effects");
    let target_effects = &mut duplicate_effect_identity
        .state
        .entities
        .iter_mut()
        .find(|entity| entity.id == FIRST_TARGET_ID)
        .expect("condition target")
        .effects;
    target_effects.push(target_effects[0].clone());
    let duplicate_failure = RpgAuthoritySession::restore_checkpoint(duplicate_effect_identity)
        .expect_err("duplicate active effect identity fails closed");
    assert!(duplicate_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "RPG_CHECKPOINT_STATE_INVALID"
    }), "unexpected duplicate-effect diagnostics: {:?}", duplicate_failure.diagnostics);

    let mut no_random = ScriptedSource::new(&session, []);
    let (exposed, expose_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 1,
                action_id: "action.expose".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("apply fixed-tenure effect");
    accepted(exposed);
    replay_entries.push(expose_entry);

    let mut probe_source = ScriptedSource::new(&session, [vec![12]]);
    let (target_lane, target_lane_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 2,
                action_id: "action.condition-probe".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut probe_source,
        )
        .expect("target-lane condition probe");
    let target_lane = accepted(target_lane);
    let target_ledger = target_lane
        .events
        .iter()
        .find_map(|event| match event {
            RpgDomainEvent::ScalarTestResolved {
                contribution_ledger,
                ..
            } => Some(contribution_ledger),
            _ => None,
        })
        .expect("target-lane ledger");
    assert!(target_ledger.candidates.iter().any(|candidate| {
        candidate.source_definition_id == "effect.save-ends-restricted"
            && candidate.contribution_id == "target-opening"
    }));
    assert!(target_ledger.candidates.iter().any(|candidate| {
        candidate.source_definition_id == "effect.save-ends-auxiliary"
            && candidate.contribution_id == "auxiliary-opening"
    }));
    assert!(!target_ledger
        .candidates
        .iter()
        .any(|candidate| candidate.contribution_id == "actor-pressure"));
    replay_entries.push(target_lane_entry);

    let (hero_ended, hero_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 3,
            actor_id: ACTOR_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("end hero turn");
    assert!(matches!(
        hero_ended,
        RpgCommandOutcome::ControlAccepted(_)
    ));
    replay_entries.push(hero_end_entry);
    let first_target_turn = session.encounter_view();
    let first_target = first_target_turn
        .participants
        .iter()
        .find(|participant| participant.id == FIRST_TARGET_ID)
        .expect("conditioned target");
    let exposed = first_target
        .effects
        .iter()
        .find(|effect| effect.definition_id == "effect.exposed")
        .expect("fixed effect remains after first target-turn boundary");
    assert_eq!(exposed.remaining_count, 1);
    assert!(matches!(
        exposed.tenure,
        asha_rpg::RpgEffectTenure::Fixed {
            anchor: asha_rpg::RpgEffectDurationAnchor::TargetTurnStart,
            count: 2
        }
    ));
    let restricted_effect = first_target
        .effects
        .iter()
        .find(|effect| effect.definition_id == "effect.save-ends-restricted")
        .expect("save-ends effect readback");
    assert!(matches!(
        restricted_effect.tenure,
        asha_rpg::RpgEffectTenure::TargetTurnEndSave {}
    ));
    assert_eq!(
        restricted_effect
            .condition
            .as_ref()
            .expect("typed condition")
            .clauses
            .len(),
        2
    );
    let restricted = first_target_turn
        .actions
        .iter()
        .find(|action| action.definition_id == "action.restricted")
        .expect("restricted action readback");
    assert!(!restricted.available);
    assert_eq!(
        restricted
            .unavailable
            .as_ref()
            .expect("restriction reason")
            .code,
        "RPG_CONDITION_ACTION_RESTRICTED"
    );
    let unavailable_source = restricted
        .unavailable
        .as_ref()
        .and_then(|reason| reason.unavailable_source.as_ref())
        .expect("restriction source");
    assert_eq!(
        unavailable_source.source_definition_id,
        "effect.save-ends-restricted"
    );
    let stale_restricted_binding = restricted.options.binding.clone();
    let movement = first_target_turn
        .actions
        .iter()
        .find(|action| action.definition_id == "action.shift")
        .expect("movement readback");
    assert!(!movement.available);
    assert_eq!(
        movement
            .unavailable
            .as_ref()
            .expect("movement restriction reason")
            .code,
        "RPG_CONDITION_MOVEMENT_RESTRICTED"
    );
    let restricted_checkpoint = session.checkpoint().expect("restricted checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let rejected = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: restricted.options.binding.clone(),
                target_ids: vec![ACTOR_ID.to_owned()],
            },
            &mut no_random,
        )
        .expect("restricted action submission is an authority outcome");
    assert!(matches!(
        rejected.outcome,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_CONDITION_ACTION_RESTRICTED"
    ));
    let mut no_random = ScriptedSource::new(&session, []);
    let rejected = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: movement.options.binding.clone(),
                target_ids: vec!["cell-0-0".to_owned()],
            },
            &mut no_random,
        )
        .expect("restricted movement submission is an authority outcome");
    assert!(matches!(
        rejected.outcome,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_CONDITION_MOVEMENT_RESTRICTED"
    ));
    assert_eq!(
        session.checkpoint().expect("condition rejections are atomic"),
        restricted_checkpoint
    );

    let mut probe_source = ScriptedSource::new(&session, [vec![12]]);
    let (actor_lane, actor_lane_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 4,
                action_id: "action.condition-probe".to_owned(),
                actor_id: FIRST_TARGET_ID.to_owned(),
                target_ids: vec![ACTOR_ID.to_owned()],
                item_binding: None,
            },
            &mut probe_source,
        )
        .expect("actor-lane condition probe");
    let actor_lane = accepted(actor_lane);
    let actor_ledger = actor_lane
        .events
        .iter()
        .find_map(|event| match event {
            RpgDomainEvent::ScalarTestResolved {
                contribution_ledger,
                ..
            } => Some(contribution_ledger),
            _ => None,
        })
        .expect("actor-lane ledger");
    assert!(actor_ledger.candidates.iter().any(|candidate| {
        candidate.source_definition_id == "effect.save-ends-restricted"
            && candidate.contribution_id == "actor-pressure"
    }));
    assert!(!actor_ledger
        .candidates
        .iter()
        .any(|candidate| candidate.contribution_id == "target-opening"));
    replay_entries.push(actor_lane_entry);

    let mut fail_source = ScriptedSource::new(&session, [vec![9], vec![9]]);
    let (failed_saves, failed_save_entries) = session
        .control_with_random_source_recorded(
            RpgTurnControlProposal {
                expected_revision: 5,
                actor_id: FIRST_TARGET_ID.to_owned(),
                control: RpgTurnControl::EndTurn,
            },
            &mut fail_source,
        )
        .expect("resolve failed save-ends evidence");
    let RpgCommandOutcome::ControlAccepted(failed_saves) = failed_saves else {
        panic!("failed saves still advance the turn");
    };
    assert_eq!(
        failed_saves
            .events
            .iter()
            .filter_map(|event| match event {
                RpgDomainEvent::EffectSaveResolved {
                    definition_id,
                    roll: 9,
                    saved: false,
                    ..
                } => Some(definition_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            "effect.save-ends-auxiliary",
            "effect.save-ends-restricted"
        ]
    );
    assert_eq!(
        fail_source
            .requests
            .iter()
            .map(|request| (request.kind, request.count, request.sides))
            .collect::<Vec<_>>(),
        [
            (RpgRandomRequestKind::EffectSave, 1, 20),
            (RpgRandomRequestKind::EffectSave, 1, 20)
        ]
    );
    replay_entries.extend(failed_save_entries);

    let (second_target_ended, second_target_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 6,
            actor_id: SECOND_TARGET_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("advance second target");
    assert!(matches!(
        second_target_ended,
        RpgCommandOutcome::ControlAccepted(_)
    ));
    replay_entries.push(second_target_end_entry);
    let (hero_ended, hero_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 7,
            actor_id: ACTOR_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("advance hero to second target turn");
    assert!(matches!(
        hero_ended,
        RpgCommandOutcome::ControlAccepted(_)
    ));
    replay_entries.push(hero_end_entry);
    let second_target_turn = session.encounter_view();
    let first_target = second_target_turn
        .participants
        .iter()
        .find(|participant| participant.id == FIRST_TARGET_ID)
        .expect("conditioned target");
    assert!(first_target
        .effects
        .iter()
        .all(|effect| effect.definition_id != "effect.exposed"));

    let mut success_source = ScriptedSource::new(&session, [vec![10], vec![10]]);
    let (saved, saved_entries) = session
        .control_with_random_source_recorded(
            RpgTurnControlProposal {
                expected_revision: 8,
                actor_id: FIRST_TARGET_ID.to_owned(),
                control: RpgTurnControl::EndTurn,
            },
            &mut success_source,
        )
        .expect("resolve successful save-ends evidence");
    let RpgCommandOutcome::ControlAccepted(saved) = saved else {
        panic!("successful saves should advance the turn");
    };
    assert_eq!(
        saved
            .events
            .iter()
            .filter_map(|event| match event {
                RpgDomainEvent::EffectSaveResolved {
                    definition_id,
                    roll: 10,
                    saved: true,
                    ..
                } => Some(definition_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            "effect.save-ends-auxiliary",
            "effect.save-ends-restricted"
        ]
    );
    replay_entries.extend(saved_entries);
    let after_save = session.encounter_view();
    let target = after_save
        .participants
        .iter()
        .find(|participant| participant.id == FIRST_TARGET_ID)
        .expect("saved target");
    assert!(target.effects.is_empty());

    let before_stale = session.checkpoint().expect("post-save checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let stale = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: stale_restricted_binding,
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
            },
            &mut no_random,
        )
        .expect("expired-source binding is stale");
    assert!(matches!(
        stale.outcome,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_ACTION_OPTION_STALE"
    ));
    assert_eq!(
        session.checkpoint().expect("stale binding is atomic"),
        before_stale
    );

    let replayed =
        RpgAuthoritySession::replay(initial, &replay_entries).expect("condition replay");
    assert_eq!(replayed.state(), session.state());
    assert_eq!(
        replayed.state_hash().expect("condition replay hash"),
        session.state_hash().expect("condition authority hash")
    );
    assert_eq!(
        replayed.accepted_random_values(),
        session.accepted_random_values()
    );
    assert_eq!(replayed.encounter_view().log, session.encounter_view().log);
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

    let mut incompatible_line_model: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    incompatible_line_model["ruleset"]["models"]["lineOfEffect"]["version"] =
        Value::Number(99_u64.into());
    assert_compile_rejects(&incompatible_line_model, "RULESET_MODEL_UNSUPPORTED");

    let mut missing_line_model: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    missing_line_model["ruleset"]["models"]
        .as_object_mut()
        .expect("ruleset models")
        .remove("lineOfEffect");
    assert_compile_rejects(
        &missing_line_model,
        "ACTION_LINE_OF_EFFECT_MODEL_REQUIRED",
    );

    let mut missing_selector_requirement: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let definition = missing_selector_requirement["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "action.expose")
        .expect("expose definition");
    definition["semantic"]["action"]["targets"]
        .as_object_mut()
        .expect("target selector")
        .remove("lineOfEffect");
    let typed_definition: MaterializedContentDefinition =
        serde_json::from_value(definition.clone()).expect("decode selector tamper");
    definition["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_definition)
            .expect("fingerprint selector tamper"),
    );
    assert_compile_rejects(
        &missing_selector_requirement,
        "RPG_IR_LINE_OF_EFFECT_REQUIREMENT_MISSING",
    );

    let mut duplicate_condition: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let condition = duplicate_condition["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "effect.save-ends-restricted")
        .expect("save-ends condition");
    let duplicate_clause = condition["semantic"]["condition"]["clauses"][0].clone();
    condition["semantic"]["condition"]["clauses"]
        .as_array_mut()
        .expect("condition clauses")
        .insert(1, duplicate_clause);
    let typed_condition: MaterializedContentDefinition =
        serde_json::from_value(condition.clone()).expect("decode condition definition");
    condition["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_condition)
            .expect("fingerprint duplicate condition"),
    );
    assert_compile_rejects(
        &duplicate_condition,
        "EFFECT_CONDITION_CLAUSES_NOT_CANONICAL",
    );

    let mut tampered_tenure: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let condition = tampered_tenure["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "effect.save-ends-restricted")
        .expect("save-ends condition");
    condition["semantic"]["tenure"]["successMinimum"] = Value::Number(9_u64.into());
    let typed_condition: MaterializedContentDefinition =
        serde_json::from_value(condition.clone()).expect("decode tenure definition");
    condition["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_condition)
            .expect("fingerprint tampered tenure"),
    );
    assert_compile_rejects(&tampered_tenure, "EFFECT_SEMANTIC_DECODE_FAILED");

    let mut invalid_spatial_shape: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let spatial_source = invalid_spatial_shape["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "spatial-source.fixed-hazard")
        .expect("spatial-source definition");
    spatial_source["semantic"]["shape"]["radius"] = Value::Number(9_u64.into());
    let typed_spatial_source: MaterializedContentDefinition =
        serde_json::from_value(spatial_source.clone()).expect("decode spatial source");
    spatial_source["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_spatial_source)
            .expect("fingerprint spatial-source shape tamper"),
    );
    assert_compile_rejects(
        &invalid_spatial_shape,
        "SPATIAL_SOURCE_SHAPE_INVALID",
    );

    let mut duplicate_spatial_trigger: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let spatial_source = duplicate_spatial_trigger["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "spatial-source.fixed-hazard")
        .expect("spatial-source definition");
    let duplicate = spatial_source["semantic"]["triggers"][0].clone();
    spatial_source["semantic"]["triggers"]
        .as_array_mut()
        .expect("spatial-source triggers")
        .insert(1, duplicate);
    let typed_spatial_source: MaterializedContentDefinition =
        serde_json::from_value(spatial_source.clone()).expect("decode spatial source");
    spatial_source["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_spatial_source)
            .expect("fingerprint duplicate spatial-source trigger"),
    );
    assert_compile_rejects(
        &duplicate_spatial_trigger,
        "SPATIAL_SOURCE_TRIGGERS_NOT_CANONICAL",
    );

    let mut random_spatial_trigger: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let procedure = random_spatial_trigger["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "procedure.hazard-pulse")
        .expect("spatial trigger procedure");
    procedure["semantic"]["implementation"]["template"]["program"]["body"]["noRoll"]
        ["operation"]["parts"][0]["amount"] =
        serde_json::json!({"kind": "dice", "count": 1, "sides": 6, "bonus": 0});
    let typed_procedure: MaterializedContentDefinition =
        serde_json::from_value(procedure.clone()).expect("decode trigger procedure");
    procedure["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_procedure)
            .expect("fingerprint random spatial trigger"),
    );
    assert_compile_rejects(
        &random_spatial_trigger,
        "SPATIAL_SOURCE_TRIGGER_SEMANTICS_UNSUPPORTED",
    );

    let mut foreign_catalog_reference: Value =
        serde_json::from_slice(prepared_source).expect("decode prepared witness JSON");
    let procedure = foreign_catalog_reference["materializedDefinitions"]
        .as_array_mut()
        .expect("definitions")
        .iter_mut()
        .find(|definition| definition["id"] == "procedure.hazard-pulse")
        .expect("spatial trigger procedure");
    procedure["semantic"]["implementation"]["template"]["program"]["body"]["noRoll"]
        ["operation"]["parts"][0]["damageType"]["packageId"] =
        Value::String("foreign.content".to_owned());
    let typed_procedure: MaterializedContentDefinition =
        serde_json::from_value(procedure.clone()).expect("decode trigger procedure");
    procedure["fingerprint"] = Value::String(
        materialized_definition_fingerprint(&typed_procedure)
            .expect("fingerprint foreign trigger catalog reference"),
    );
    assert_compile_rejects(
        &foreign_catalog_reference,
        "ACTION_PROCEDURE_CATALOG_REFERENCE_OWNER_MISMATCH",
    );
}

fn prove_fixed_spatial_source_lifecycle(bundle: asha_rpg::CompiledPlayBundle) {
    let mut setup = scenario(&bundle, 5);
    setup
        .participants
        .iter_mut()
        .find(|participant| participant.id == SECOND_TARGET_ID)
        .expect("second spatial target")
        .position = GridPosition { x: 4, y: 0 };
    for participant in &mut setup.participants {
        for definition_id in ["action.create-fixed-hazard", "action.shift"] {
            if !participant
                .definition_ids
                .iter()
                .any(|candidate| candidate == definition_id)
            {
                participant.definition_ids.push(definition_id.to_owned());
            }
        }
        participant.definition_ids.sort();
    }
    let mut session =
        RpgAuthoritySession::from_scenario(bundle.clone(), setup.clone()).expect("spatial setup");
    let initial = session.checkpoint().expect("spatial initial checkpoint");
    let mut replay_entries = Vec::new();

    let mut no_random = ScriptedSource::new(&session, []);
    let (created, created_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.create-fixed-hazard".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![ACTOR_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("create fixed spatial source");
    let created = accepted(created);
    assert!(created.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceCreated {
            owner_id,
            source_id,
            instance_id,
            definition_id,
            included_cell_ids,
            remaining_count: 3,
            application_revision: 1,
            ..
        } if owner_id == ACTOR_ID
            && source_id == ACTOR_ID
            && instance_id == "fixed-hazard"
            && definition_id == "spatial-source.fixed-hazard"
            && included_cell_ids
                == &[
                    "cell-0-1",
                    "cell-1-0",
                    "cell-1-1",
                    "cell-1-2",
                    "cell-2-1",
                ]
    )));
    assert!(!created.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Applied,
            ..
        }
    )));
    replay_entries.push(created_entry);
    let created_view = session.encounter_view();
    assert_eq!(created_view.spatial_sources.len(), 1);
    assert_eq!(
        created_view.spatial_sources[0].trigger_boundaries,
        [
            asha_rpg::RpgSpatialSourceBoundary::Enter,
            asha_rpg::RpgSpatialSourceBoundary::StartTurn,
            asha_rpg::RpgSpatialSourceBoundary::EndTurn,
            asha_rpg::RpgSpatialSourceBoundary::Exit,
        ]
    );
    assert!(created_view.spatial_sources[0].trigger_evidence.is_empty());
    let restored_created =
        RpgAuthoritySession::restore_checkpoint(session.checkpoint().expect("spatial checkpoint"))
            .expect("restore active spatial source");
    assert_eq!(
        restored_created.state_hash().expect("restored spatial hash"),
        session.state_hash().expect("active spatial hash")
    );

    let stale_before = session.checkpoint().expect("pre-stale spatial checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let (stale, _stale_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.create-fixed-hazard".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![ACTOR_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("stale spatial creation is an authority outcome");
    assert!(matches!(
        stale,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_SESSION_REVISION_MISMATCH"
    ));
    assert_eq!(
        session.checkpoint().expect("post-stale spatial checkpoint"),
        stale_before
    );

    let (actor_ended, actor_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 1,
            actor_id: ACTOR_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("advance into fixed source");
    let RpgCommandOutcome::ControlAccepted(actor_ended) = actor_ended else {
        panic!("actor turn should advance");
    };
    assert!(actor_ended.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::StartTurn,
            participant_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Applied,
            ..
        } if participant_id == FIRST_TARGET_ID
    )));
    assert!(actor_ended.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::EndTurn,
            participant_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Inapplicable { reason },
            ..
        } if participant_id == ACTOR_ID && reason == "targetFilter"
    )));
    assert_eq!(vitality(&session.encounter_view(), FIRST_TARGET_ID), 29);
    replay_entries.push(actor_end_entry);

    let mut no_random = ScriptedSource::new(&session, []);
    let (exited, exited_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 2,
                action_id: "action.shift".to_owned(),
                actor_id: FIRST_TARGET_ID.to_owned(),
                target_ids: vec!["cell-3-1".to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("leave fixed source");
    let exited = accepted(exited);
    assert!(exited.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::Exit,
            participant_id,
            cell_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Applied,
            ..
        } if participant_id == FIRST_TARGET_ID && cell_id == "cell-2-1"
    )));
    assert_eq!(vitality(&session.encounter_view(), FIRST_TARGET_ID), 28);
    replay_entries.push(exited_entry);

    let (first_ended, first_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 3,
            actor_id: FIRST_TARGET_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("advance to second target");
    assert!(matches!(
        first_ended,
        RpgCommandOutcome::ControlAccepted(_)
    ));
    replay_entries.push(first_end_entry);

    let mut no_random = ScriptedSource::new(&session, []);
    let (entered, entered_entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 4,
                action_id: "action.shift".to_owned(),
                actor_id: SECOND_TARGET_ID.to_owned(),
                target_ids: vec!["cell-2-1".to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("enter fixed source through intermediary cells");
    let entered = accepted(entered);
    assert!(entered.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementTransition {
            route_cell_ids,
            ..
        } if route_cell_ids.last().is_some_and(|cell_id| cell_id == "cell-2-1")
    )));
    assert!(entered.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::Enter,
            participant_id,
            cell_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Applied,
            ..
        } if participant_id == SECOND_TARGET_ID && cell_id == "cell-2-1"
    )));
    assert_eq!(vitality(&session.encounter_view(), SECOND_TARGET_ID), 29);
    replay_entries.push(entered_entry);

    let (second_ended, second_end_entry) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 5,
            actor_id: SECOND_TARGET_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("expire fixed source");
    let RpgCommandOutcome::ControlAccepted(second_ended) = second_ended else {
        panic!("second target turn should advance");
    };
    assert!(second_ended.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::EndTurn,
            participant_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Applied,
            ..
        } if participant_id == SECOND_TARGET_ID
    )));
    assert!(second_ended.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceExpired {
            instance_id,
            ..
        } if instance_id == "fixed-hazard"
    )));
    assert_eq!(vitality(&session.encounter_view(), SECOND_TARGET_ID), 28);
    assert!(session.encounter_view().spatial_sources.is_empty());
    replay_entries.push(second_end_entry);

    let replayed =
        RpgAuthoritySession::replay(initial, &replay_entries).expect("spatial-source replay");
    assert_eq!(replayed.state(), session.state());
    assert_eq!(
        replayed.state_hash().expect("spatial replay hash"),
        session.state_hash().expect("spatial authority hash")
    );

    let mut overlap = RpgAuthoritySession::from_scenario(bundle.clone(), setup.clone())
        .expect("overlap spatial setup");
    let mut no_random = ScriptedSource::new(&overlap, []);
    let (first_created, _) = overlap
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.create-fixed-hazard".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![ACTOR_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("first overlapping source");
    accepted(first_created);
    let (advanced, _) = overlap
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 1,
            actor_id: ACTOR_ID.to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .expect("advance overlap source");
    assert!(matches!(advanced, RpgCommandOutcome::ControlAccepted(_)));
    let mut no_random = ScriptedSource::new(&overlap, []);
    let (second_created, _) = overlap
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 2,
                action_id: "action.create-fixed-hazard".to_owned(),
                actor_id: FIRST_TARGET_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("second overlapping source");
    accepted(second_created);
    let overlap_view = overlap.encounter_view();
    assert_eq!(overlap_view.spatial_sources.len(), 2);
    assert_eq!(
        overlap_view
            .spatial_sources
            .iter()
            .map(|source| (
                source.instance_id.as_str(),
                source.source_entity_id.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("fixed-hazard", FIRST_TARGET_ID),
            ("fixed-hazard", ACTOR_ID)
        ]
    );

    let mut application_revision =
        RpgAuthoritySession::from_scenario(bundle, setup).expect("application-revision setup");
    let initial = application_revision
        .checkpoint()
        .expect("application-revision initial");
    let target_vitality =
        vitality(&application_revision.encounter_view(), FIRST_TARGET_ID);
    let mut no_random = ScriptedSource::new(&application_revision, []);
    let (created_and_moved, replay_entry) = application_revision
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.create-and-leave-hazard".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("create and leave source atomically");
    let created_and_moved = accepted(created_and_moved);
    assert!(created_and_moved.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::SpatialSourceTriggerEvaluated {
            boundary: asha_rpg::RpgSpatialSourceBoundary::Exit,
            participant_id,
            disposition: asha_rpg::RpgSpatialSourceTriggerDisposition::Suppressed { reason },
            ..
        } if participant_id == FIRST_TARGET_ID && reason == "applicationRevision"
    )));
    assert_eq!(
        vitality(&application_revision.encounter_view(), FIRST_TARGET_ID),
        target_vitality
    );
    let replayed = RpgAuthoritySession::replay(initial, &[replay_entry])
        .expect("application-revision suppression replay");
    assert_eq!(
        replayed.state_hash().expect("suppression replay hash"),
        application_revision
            .state_hash()
            .expect("suppression authority hash")
    );
}

fn prove_line_of_effect_projection_staleness_and_atomicity(
    bundle: asha_rpg::CompiledPlayBundle,
) {
    let mut wrong_fact = scenario(&bundle, 5);
    wrong_fact.board.cells[0].capabilities[0].value =
        RpgCellCapabilityValue::Flag { value: true };
    let wrong_fact_failure = RpgAuthoritySession::from_scenario(bundle.clone(), wrong_fact)
        .expect_err("reserved obstruction identity rejects a generic flag");
    assert!(wrong_fact_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "RPG_SCENARIO_LINE_OF_EFFECT_OBSTRUCTION_INVALID"
            && diagnostic.path == "$.board.cells[0].capabilities[0]"
    }));

    let mut duplicate_fact = scenario(&bundle, 5);
    let duplicate = duplicate_fact.board.cells[0].capabilities[0].clone();
    duplicate_fact.board.cells[0].capabilities.push(duplicate);
    let duplicate_failure = RpgAuthoritySession::from_scenario(bundle.clone(), duplicate_fact)
        .expect_err("duplicate obstruction facts fail before session state");
    assert!(duplicate_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "RPG_SCENARIO_CELL_CAPABILITY_ID_INVALID"
            && diagnostic.path == "$.board.cells[0].capabilities[2].id"
    }));

    let mut blocked_scenario = scenario(&bundle, 5);
    set_line_blocker(&mut blocked_scenario, 2, 1, true);
    let session = RpgAuthoritySession::from_scenario(bundle.clone(), blocked_scenario.clone())
        .expect("blocked setup");
    let expose = session
        .encounter_view()
        .actions
        .into_iter()
        .find(|action| action.definition_id == "action.expose")
        .expect("line-of-effect action");
    assert_eq!(expose.options.participant_ids, [FIRST_TARGET_ID]);
    assert!(expose.options.filtered_participants.iter().any(|candidate| {
        candidate.participant_id == SECOND_TARGET_ID
            && candidate.reason == "lineOfEffectBlocked"
            && candidate.blocking_cell_ids == ["cell-2-1"]
    }));

    let binding = expose.options.binding;
    let mut clone = session.clone();
    let before_clone = clone.checkpoint().expect("clone checkpoint");
    let mut no_random = ScriptedSource::new(&clone, []);
    let stale = clone
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: binding.clone(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
            },
            &mut no_random,
        )
        .expect("stale binding is an outcome");
    assert!(matches!(stale.outcome, RpgCommandOutcome::Rejected(_)));
    assert!(stale.replay_entry.is_none());
    assert_eq!(clone.checkpoint().expect("unchanged clone"), before_clone);

    let mut session = session;
    let before_blocked = session.checkpoint().expect("blocked checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let blocked = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding,
                target_ids: vec![SECOND_TARGET_ID.to_owned()],
            },
            &mut no_random,
        )
        .expect("blocked target is an authority outcome");
    assert!(matches!(
        blocked.outcome,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_LINE_OF_EFFECT_BLOCKED"
    ));
    assert_eq!(
        session.checkpoint().expect("blocked submission unchanged"),
        before_blocked
    );
    let shift = session
        .encounter_view()
        .actions
        .into_iter()
        .find(|action| action.definition_id == "action.shift")
        .expect("line-of-effect cell action");
    assert!(shift
        .options
        .cell_paths
        .iter()
        .any(|path| path.destination_cell_id == "cell-1-2"));
    assert!(shift.options.filtered_cells.iter().any(|candidate| {
        candidate.cell_id == "cell-4-1"
            && candidate.reason == "lineOfEffectBlocked"
            && candidate.blocking_cell_ids == ["cell-2-1"]
    }));
    let before_cell = session.checkpoint().expect("cell checkpoint");
    let mut no_random = ScriptedSource::new(&session, []);
    let blocked_cell = session
        .submit_bound_with_random_source_recorded(
            RpgBoundActionProposal {
                binding: shift.options.binding,
                target_ids: vec!["cell-4-1".to_owned()],
            },
            &mut no_random,
        )
        .expect("blocked cell is an authority outcome");
    assert!(matches!(
        blocked_cell.outcome,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_LINE_OF_EFFECT_BLOCKED"
    ));
    assert_eq!(
        session.checkpoint().expect("blocked cell unchanged"),
        before_cell
    );

    let mut area_session = RpgAuthoritySession::from_scenario(bundle.clone(), blocked_scenario)
        .expect("blocked area setup");
    let area_option = area_session
        .encounter_view()
        .actions
        .into_iter()
        .find(|action| action.definition_id == "action.burst")
        .and_then(|action| {
            action
                .options
                .area_options
                .into_iter()
                .find(|option| option.anchor_cell_id == "cell-1-1")
        })
        .expect("actor-anchored burst option");
    assert_eq!(area_option.included_participant_ids, [FIRST_TARGET_ID]);
    assert!(area_option.filtered_participants.iter().any(|participant| {
        participant.participant_id == SECOND_TARGET_ID
            && participant.reason == "lineOfEffectBlocked"
            && participant.blocking_cell_ids == ["cell-2-1"]
    }));
    let initial_area = area_session.checkpoint().expect("initial area checkpoint");
    let mut no_random = ScriptedSource::new(&area_session, []);
    let area_submission = area_session
        .submit_area_option_with_random_source_recorded(area_option, &mut no_random)
        .expect("complete area option");
    let area_receipt = accepted(area_submission.outcome);
    assert!(area_receipt.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::AreaTargetsDerived {
            included_participant_ids,
            filtered_participants,
            ..
        } if included_participant_ids == &[FIRST_TARGET_ID]
            && filtered_participants.iter().any(|participant| {
                participant.participant_id == SECOND_TARGET_ID
                    && participant.blocking_cell_ids == ["cell-2-1"]
            })
    )));
    let replayed_area = RpgAuthoritySession::replay(
        initial_area,
        &[area_submission
            .replay_entry
            .expect("accepted area has replay entry")],
    )
    .expect("area replay");
    assert_eq!(replayed_area.state(), area_session.state());
    assert_eq!(
        replayed_area.state_hash().expect("area replay hash"),
        area_session.state_hash().expect("area authority hash")
    );

    let mut diagonal = scenario(&bundle, 5);
    diagonal.participants[0].position = GridPosition { x: 0, y: 0 };
    diagonal.participants[1].position = GridPosition { x: 2, y: 2 };
    set_line_blocker(&mut diagonal, 1, 0, true);
    set_line_blocker(&mut diagonal, 0, 1, true);
    let diagonal = RpgAuthoritySession::from_scenario(bundle.clone(), diagonal)
        .expect("diagonal tie setup")
        .encounter_view();
    let diagonal_filter = diagonal
        .actions
        .iter()
        .find(|action| action.definition_id == "action.expose")
        .and_then(|action| {
            action
                .options
                .filtered_participants
                .iter()
                .find(|candidate| candidate.participant_id == FIRST_TARGET_ID)
        })
        .expect("diagonal target is filtered");
    assert_eq!(
        diagonal_filter.blocking_cell_ids,
        ["cell-1-0", "cell-0-1"]
    );

    let mut missing = scenario(&bundle, 5);
    missing
        .board
        .cells
        .retain(|cell| cell.position != GridPosition { x: 2, y: 1 });
    missing.participants[1].position = GridPosition { x: 3, y: 1 };
    missing.participants[2].position = GridPosition { x: 4, y: 1 };
    let missing = RpgAuthoritySession::from_scenario(bundle, missing)
        .expect("sparse boundary setup")
        .encounter_view();
    assert!(missing
        .actions
        .iter()
        .find(|action| action.definition_id == "action.expose")
        .expect("expose action")
        .options
        .filtered_participants
        .iter()
        .any(|candidate| {
            candidate.participant_id == FIRST_TARGET_ID
                && candidate.reason == "lineOfEffectCellMissing"
        }));
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

fn prove_movement_allowance_forced_choices_and_reactions(
    bundle: asha_rpg::CompiledPlayBundle,
) {
    let mut setup = scenario(&bundle, 5);
    setup
        .participants
        .iter_mut()
        .find(|participant| participant.id == ACTOR_ID)
        .expect("movement actor")
        .position = GridPosition { x: 0, y: 1 };
    let actor_vitality = setup
        .participants
        .iter_mut()
        .find(|participant| participant.id == ACTOR_ID)
        .and_then(|participant| {
            participant
                .capabilities
                .iter_mut()
                .find_map(|capability| match capability {
                    RpgInitialCapability::Vitality { value } => Some(value),
                    _ => None,
                })
        })
        .expect("actor vitality");
    actor_vitality.current = 20;
    let responder = setup
        .participants
        .iter_mut()
        .find(|participant| participant.id == FIRST_TARGET_ID)
        .expect("movement responder");
    responder.position = GridPosition { x: 1, y: 1 };
    responder.definition_ids = vec!["action.leave-response".to_owned()];
    responder.class_definition_id = Some("class.vanguard".to_owned());
    responder.feature_definition_ids = vec!["feature.tactical-training".to_owned()];
    responder.items = vec![RpgItemInstanceSetup {
        id: "response-blade".to_owned(),
        definition_id: "item.short-blade".to_owned(),
    }];
    responder.equipment = vec![RpgEquipmentSlotSetup {
        slot_id: "hand.main".to_owned(),
        item_instance_id: "response-blade".to_owned(),
    }];
    setup
        .participants
        .iter_mut()
        .find(|participant| participant.id == SECOND_TARGET_ID)
        .expect("movement blocker")
        .position = GridPosition { x: 4, y: 1 };
    set_traversal(&mut setup, 0, 2, true, 2);
    set_traversal(&mut setup, 3, 1, false, 1);

    let mut movement =
        RpgAuthoritySession::from_scenario(bundle.clone(), setup.clone()).expect("movement setup");
    let movement_initial = movement.checkpoint().expect("movement initial checkpoint");
    let view = movement.encounter_view();
    let actor = view
        .participants
        .iter()
        .find(|participant| participant.id == ACTOR_ID)
        .expect("movement actor view");
    assert_eq!(
        actor
            .movement_allowance
            .as_ref()
            .map(|allowance| allowance.remaining),
        Some(5)
    );
    let shift = view
        .actions
        .iter()
        .find(|action| action.definition_id == "action.shift")
        .expect("shift readback");
    let route = shift
        .options
        .cell_paths
        .iter()
        .find(|path| path.destination_cell_id == "cell-0-2")
        .expect("weighted movement route");
    assert_eq!(route.cell_ids, ["cell-0-2"]);
    assert_eq!(route.movement_cost, 2);

    let mut no_random = ScriptedSource::new(&movement, []);
    let (pending, move_entry) = movement
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.shift".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec!["cell-0-2".to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("movement submission");
    let RpgCommandOutcome::AwaitingReaction(pending) = pending else {
        panic!("voluntary movement should await the registered response");
    };
    let context = pending
        .request
        .movement
        .as_ref()
        .expect("movement reaction context");
    assert_eq!(context.owner_id, FIRST_TARGET_ID);
    assert_eq!(context.source_definition_id, "feature.tactical-training");
    assert_eq!(context.response_action_id, "action.leave-response");
    let response_binding = context
        .response_item_binding
        .as_ref()
        .expect("captured response item binding");
    assert_eq!(response_binding.item_instance_id, "response-blade");
    assert_eq!(response_binding.item_definition_id, "item.short-blade");
    assert_eq!(response_binding.slot_id, "hand.main");
    assert_eq!(context.trigger_start, GridPosition { x: 0, y: 1 });
    assert_eq!(context.trigger_end, GridPosition { x: 0, y: 2 });
    assert_eq!(
        movement
            .state()
            .entity(ACTOR_ID)
            .expect("staged actor")
            .position(),
        GridPosition { x: 0, y: 1 }
    );
    let pending_hash = movement.state_hash().expect("pending hash");
    let mut no_random = ScriptedSource::new(&movement, []);
    let (responded, response_entry) = movement
        .react_with_random_source_recorded(
            RpgReactionProposal {
                expected_revision: 0,
                reaction_id: pending.request.reaction_id,
                option_id: Some("respond".to_owned()),
            },
            &mut no_random,
        )
        .expect("movement response");
    let response = accepted(responded);
    assert_eq!(
        movement
            .state()
            .entity(ACTOR_ID)
            .expect("moved actor")
            .position(),
        GridPosition { x: 0, y: 2 }
    );
    assert_eq!(
        movement
            .state()
            .entity(ACTOR_ID)
            .expect("responded actor")
            .vitality()
            .current,
        21
    );
    assert!(response.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementTransition {
            movement_kind: RpgMovementKind::Voluntary,
            route_cell_ids,
            movement_cost: 2,
            provokes: true,
            ..
        } if route_cell_ids == &["cell-0-2"]
    )));
    assert!(response.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementReactionResolved {
            owner_id,
            accepted: true,
            response_action_id,
            ..
        } if owner_id == FIRST_TARGET_ID && response_action_id == "action.leave-response"
    )));
    assert_ne!(movement.state_hash().expect("accepted hash"), pending_hash);
    assert_eq!(
        movement
            .encounter_view()
            .participants
            .iter()
            .find(|participant| participant.id == ACTOR_ID)
            .and_then(|participant| participant.movement_allowance.as_ref())
            .map(|allowance| allowance.remaining),
        Some(3)
    );
    let movement_replayed =
        RpgAuthoritySession::replay(movement_initial, &[move_entry, response_entry])
            .expect("movement reaction replay");
    assert_eq!(
        movement_replayed.state_hash().expect("replayed movement hash"),
        movement.state_hash().expect("accepted movement hash")
    );

    let mut push =
        RpgAuthoritySession::from_scenario(bundle.clone(), setup.clone()).expect("push setup");
    let push_initial = push.checkpoint().expect("push initial");
    let mut no_random = ScriptedSource::new(&push, []);
    let (pushed, push_entry) = push
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.push".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("push submission");
    let pushed = accepted(pushed);
    assert_eq!(
        push.state()
            .entity(FIRST_TARGET_ID)
            .expect("pushed target")
            .position(),
        GridPosition { x: 2, y: 1 }
    );
    assert!(pushed.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementTransition {
            movement_kind: RpgMovementKind::Push,
            route_cell_ids,
            provokes: false,
            ..
        } if route_cell_ids == &["cell-2-1"]
    )));
    assert!(!pushed.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementReactionOpened { .. }
    )));
    let pushed_replayed =
        RpgAuthoritySession::replay(push_initial, &[push_entry]).expect("push replay");
    assert_eq!(
        pushed_replayed.state_hash().expect("replayed push hash"),
        push.state_hash().expect("push hash")
    );

    let mut slide = RpgAuthoritySession::from_scenario(bundle, setup).expect("slide setup");
    let slide_initial = slide.checkpoint().expect("slide initial");
    let mut no_random = ScriptedSource::new(&slide, []);
    let (pending, slide_entry) = slide
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.slide".to_owned(),
                actor_id: ACTOR_ID.to_owned(),
                target_ids: vec![FIRST_TARGET_ID.to_owned()],
                item_binding: None,
            },
            &mut no_random,
        )
        .expect("slide submission");
    let RpgCommandOutcome::AwaitingForcedMovement(pending) = pending else {
        panic!("slide should await an authority route choice");
    };
    let option = pending
        .options
        .iter()
        .find(|option| option.route.destination_cell_id == "cell-1-0")
        .cloned()
        .expect("slide destination");
    let pending_checkpoint = slide.checkpoint().expect("pending slide checkpoint");
    let mut restored =
        RpgAuthoritySession::restore_checkpoint(pending_checkpoint).expect("restore slide");
    let restored_before = restored.state_hash().expect("restored pending hash");
    let stale = restored
        .resolve_forced_movement_recorded(
            asha_rpg::RpgForcedMovementCommand {
                option: option.clone(),
            },
            Vec::new(),
        )
        .expect("stale slide result");
    assert!(matches!(
        stale.0,
        RpgCommandOutcome::Rejected(ref rejection)
            if rejection.code == "RPG_FORCED_MOVEMENT_OPTION_STALE"
    ));
    assert_eq!(
        restored.state_hash().expect("stale slide hash"),
        restored_before
    );
    let mut no_random = ScriptedSource::new(&slide, []);
    let (slid, choice_entry) = slide
        .resolve_forced_movement_with_random_source_recorded(option, &mut no_random)
        .expect("slide choice");
    let slid = accepted(slid);
    assert!(slid.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::MovementTransition {
            movement_kind: RpgMovementKind::Slide,
            provokes: false,
            ..
        }
    )));
    assert_eq!(
        slide
            .state()
            .entity(FIRST_TARGET_ID)
            .expect("slid target")
            .position(),
        GridPosition { x: 1, y: 0 }
    );
    let slide_replayed =
        RpgAuthoritySession::replay(slide_initial, &[slide_entry, choice_entry])
            .expect("slide replay");
    assert_eq!(
        slide_replayed.state_hash().expect("replayed slide hash"),
        slide.state_hash().expect("slide hash")
    );
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
            cells: line_cells(5, 3),
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

fn line_cells(width: u32, height: u32) -> Vec<RpgCellSetup> {
    (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| RpgCellSetup {
                id: format!("cell-{x}-{y}"),
                position: GridPosition { x, y },
                capabilities: vec![RpgCellCapabilitySetup {
                    id: RPG_LINE_OF_EFFECT_OBSTRUCTION_ID.to_owned(),
                    version: RPG_LINE_OF_EFFECT_OBSTRUCTION_VERSION,
                    definition_id: None,
                    value: RpgCellCapabilityValue::LineOfEffectObstruction { blocks: false },
                }, RpgCellCapabilitySetup {
                    id: "traversal".to_owned(),
                    version: 1,
                    definition_id: None,
                    value: RpgCellCapabilityValue::Traversal {
                        passable: true,
                        movement_cost: 1,
                    },
                }],
            })
        })
        .collect()
}

fn set_traversal(
    scenario: &mut RpgScenario,
    x: u32,
    y: u32,
    passable: bool,
    movement_cost: u32,
) {
    let cell = scenario
        .board
        .cells
        .iter_mut()
        .find(|cell| cell.position == GridPosition { x, y })
        .expect("traversal cell");
    let traversal = cell
        .capabilities
        .iter_mut()
        .find(|capability| {
            matches!(
                capability.value,
                RpgCellCapabilityValue::Traversal { .. }
            )
        })
        .expect("traversal capability");
    traversal.value = RpgCellCapabilityValue::Traversal {
        passable,
        movement_cost,
    };
}

fn set_line_blocker(scenario: &mut RpgScenario, x: u32, y: u32, blocks: bool) {
    let cell = scenario
        .board
        .cells
        .iter_mut()
        .find(|cell| cell.position == GridPosition { x, y })
        .expect("line-of-effect cell");
    cell.capabilities[0].value = RpgCellCapabilityValue::LineOfEffectObstruction { blocks };
}

fn actor(focus: i32) -> RpgParticipantSetup {
    let mut actor = participant(
        ACTOR_ID,
        "Vanguard",
        RpgTeamId::ally(),
        GridPosition { x: 1, y: 1 },
    );
    actor.definition_ids = vec![
        "action.burst".to_owned(),
        "action.core-attack".to_owned(),
        "action.create-and-leave-hazard".to_owned(),
        "action.create-fixed-hazard".to_owned(),
        "action.expose".to_owned(),
        "action.push".to_owned(),
        "action.rally".to_owned(),
        "action.shift".to_owned(),
        "action.slide".to_owned(),
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
