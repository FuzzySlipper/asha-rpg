use asha_rpg::{
    compile_prepared_play_bundle, load_compiled_play_bundle, materialized_definition_fingerprint,
    BoundedValue, ContentDefinitionProvenance, ContentExtensionPolicy, ContentPackRequirements,
    ContentRelationshipKind, ContentRelationshipProvenance, ContentSourceLocation,
    ContentValueRequirement, GridPosition, MaterializedContentDefinition,
    MaterializedContentDefinitionKind, MaterializedContentVisibility, PlayBundleArtifactSchema,
    PreparedPlayBundle, ResolvedContentPack, RpgActionProposal, RpgAuthoritySession, RpgBoardSetup,
    RpgCellCapabilitySetup, RpgCellCapabilityValue, RpgCellSetup, RpgCommandOutcome,
    RpgContributionDisposition, RpgContributionStackingPolicy, RpgDomainEvent,
    RpgInitialCapability, RpgParticipantSetup, RpgRandomRequest, RpgRandomRequestKind,
    RpgRandomSourceBinding, RpgRollTapeEntry, RpgRollTapeSource, RpgScalarContributionLedger,
    RpgScenario, RpgTeamId, RpgTurnControl, RpgTurnControlProposal, RpgTurnInitialization,
    RpgVersionedIdentity, Ruleset, RulesetCalculationSelectorContract,
    RulesetContributionStackingGroupContract, RulesetModels, RulesetNumericDomain,
    RulesetProvisions, RulesetSchema, RulesetValueContract, RulesetValueExpression,
    RulesetValueFormula, RulesetValueFormulaSchema, RulesetValueKind, RulesetValueSource,
    VersionedRpgRequirement, PLAY_BUNDLE_ARTIFACT_MAJOR, PREPARED_PLAY_BUNDLE_IDENTITY,
};
use serde_json::json;

#[test]
fn public_facade_builds_an_artifact_bound_setup_and_executes_a_turn() {
    let bundle = healing_bundle();
    let scenario = RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 5,
            height: 3,
            cells: Vec::new(),
        },
        participants: vec![
            participant("actor", "Actor", RpgTeamId::ally(), 0, 20),
            participant("target", "Target", RpgTeamId::ally(), 1, 13),
            participant("opponent", "Opponent", RpgTeamId::enemy(), 4, 20),
        ],
        turn: RpgTurnInitialization {
            initiative_order: vec![
                "actor".to_owned(),
                "target".to_owned(),
                "opponent".to_owned(),
            ],
            current_actor_id: "actor".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "consumer.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "consumer.roll-tape".to_owned(),
            source_version: 1,
        },
    };
    let mut session = RpgAuthoritySession::from_scenario(bundle, scenario).unwrap();
    let mut source = RpgRollTapeSource::new(session.scenario().random_source.clone(), Vec::new());

    let (outcome, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.heal".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: None,
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Accepted(receipt) = outcome else {
        panic!("public command should be accepted: {outcome:?}");
    };

    assert_eq!(receipt.random_consumed, 0);
    assert_eq!(
        session.state().entity("target").unwrap().vitality().current,
        17
    );
    assert_eq!(session.turn().current_actor_id, "target");
    assert_eq!(session.encounter_view().log.len(), 1);

    let (control_outcome, _) = session
        .control_recorded(RpgTurnControlProposal {
            expected_revision: 1,
            actor_id: "target".to_owned(),
            control: RpgTurnControl::EndTurn,
        })
        .unwrap();
    assert!(matches!(
        control_outcome,
        RpgCommandOutcome::ControlAccepted(_)
    ));
    assert_eq!(session.turn().current_actor_id, "opponent");
    assert_eq!(session.encounter_view().log.len(), 2);
}

#[test]
fn equipped_items_project_distinct_bound_actions_and_reject_tampering_atomically() {
    let bundle = item_bound_bundle();
    let mut changed_item = item_bound_prepared();
    changed_item.materialized_definitions[1].semantic["attributes"][0]["value"] = json!(8);
    changed_item.materialized_definitions[1].fingerprint =
        materialized_definition_fingerprint(&changed_item.materialized_definitions[1]).unwrap();
    let changed_bundle = compile_prepared_play_bundle(changed_item).unwrap();
    assert_ne!(
        changed_bundle.artifact().artifact_id,
        bundle.artifact().artifact_id
    );

    let mut executable_item = item_bound_prepared();
    executable_item.materialized_definitions[1].semantic["execute"] =
        json!({"kind": "operation", "operation": {"kind": "heal"}});
    executable_item.materialized_definitions[1].fingerprint =
        materialized_definition_fingerprint(&executable_item.materialized_definitions[1]).unwrap();
    let executable_failure = compile_prepared_play_bundle(executable_item).unwrap_err();
    assert!(executable_failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ITEM_SEMANTIC_DECODE_FAILED"));

    let mut invalid_equipment = item_bound_scenario(&bundle);
    invalid_equipment.participants[0].equipment[0].slot_id = "backpack".to_owned();
    let invalid_equipment_failure =
        RpgAuthoritySession::from_scenario(bundle.clone(), invalid_equipment).unwrap_err();
    assert!(invalid_equipment_failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "RPG_SCENARIO_EQUIPMENT_SLOT_NOT_ALLOWED"));

    let mut without_items = item_bound_scenario(&bundle);
    without_items.participants[0].items.clear();
    without_items.participants[0].equipment.clear();
    let unavailable = RpgAuthoritySession::from_scenario(bundle.clone(), without_items).unwrap();
    let unavailable_actions = unavailable.encounter_view().actions;
    assert!(!unavailable_actions.is_empty());
    assert!(unavailable_actions.iter().all(|action| {
        !action.available
            && action
                .unavailable
                .as_ref()
                .is_some_and(|failure| failure.code == "RPG_ACTION_ITEM_BINDING_UNAVAILABLE")
    }));

    let scenario = item_bound_scenario(&bundle);
    let mut session = RpgAuthoritySession::from_scenario(bundle.clone(), scenario).unwrap();
    let actions = session.encounter_view().actions;
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].label, "Use Healing Kit — Greater Healing Kit");
    assert_eq!(actions[1].label, "Use Healing Kit — Healing Kit");
    assert_ne!(
        actions[0]
            .item_binding
            .as_ref()
            .map(|binding| binding.item_instance_id.as_str()),
        actions[1]
            .item_binding
            .as_ref()
            .map(|binding| binding.item_instance_id.as_str()),
    );

    let mut tampered = actions[1]
        .item_binding
        .clone()
        .expect("bound action carries exact equipment");
    tampered.item_instance_id = "kit.missing".to_owned();
    let before = session.state_hash().unwrap();
    let mut source = RpgRollTapeSource::new(session.scenario().random_source.clone(), Vec::new());
    let (outcome, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.item-heal".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: Some(tampered),
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Rejected(rejection) = outcome else {
        panic!("tampered item binding must be rejected: {outcome:?}");
    };
    assert_eq!(rejection.code, "RPG_ACTION_ITEM_BINDING_STALE");
    assert_eq!(session.state_hash().unwrap(), before);

    let selected = actions[0]
        .item_binding
        .clone()
        .expect("bound action carries exact equipment");
    let initial = session.checkpoint().unwrap();
    let (outcome, entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.item-heal".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: Some(selected.clone()),
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Accepted(receipt) = outcome else {
        panic!("valid equipment binding must execute: {outcome:?}");
    };
    assert_eq!(receipt.item_binding, Some(selected));
    assert_eq!(
        session.state().entity("target").unwrap().vitality().current,
        17
    );
    assert_eq!(
        session.encounter_view().log[0].item_binding,
        receipt.item_binding
    );

    let replayed = RpgAuthoritySession::replay(initial, &[entry]).unwrap();
    assert_eq!(
        replayed.state_hash().unwrap(),
        session.state_hash().unwrap()
    );
    let restored = RpgAuthoritySession::restore_checkpoint(session.checkpoint().unwrap()).unwrap();
    assert_eq!(
        restored.state_hash().unwrap(),
        session.state_hash().unwrap()
    );
}

#[test]
fn bound_item_tags_change_contextual_contribution_applicability() {
    let bundle = compile_prepared_play_bundle(item_attack_prepared()).unwrap();

    let greater = item_attack_ledger(bundle.clone(), "item.greater-healing-kit");
    assert_eq!(greater.base_value, 0);
    assert_eq!(greater.final_value, 2);
    assert_eq!(greater.candidates.len(), 1);
    assert_eq!(
        greater.candidates[0],
        asha_rpg::RpgScalarContributionDecision {
            source_definition_id: "item.greater-healing-kit".to_owned(),
            source_instance_id: Some("kit.greater".to_owned()),
            source_label: "Greater Healing Kit".to_owned(),
            contribution_id: "precise".to_owned(),
            selector_id: "attack-total".to_owned(),
            stacking_group_id: "circumstance".to_owned(),
            declared_value: 2,
            applied_value: 2,
            disposition: RpgContributionDisposition::Applied,
        }
    );

    let standard = item_attack_ledger(bundle, "item.healing-kit");
    assert_eq!(standard.final_value, 0);
    assert_eq!(standard.candidates.len(), 1);
    assert_eq!(
        standard.candidates[0].source_instance_id.as_deref(),
        Some("kit.standard")
    );
    assert_eq!(
        standard.candidates[0].disposition,
        RpgContributionDisposition::Inapplicable {
            reason: "boundItemTag.required.precise".to_owned(),
        }
    );
}

#[test]
fn character_features_resolve_multiple_spatial_roll_contributions_and_replay() {
    let prepared = conditional_feature_prepared();
    let bundle = compile_prepared_play_bundle(prepared.clone()).unwrap();
    assert_eq!(bundle.character_classes().len(), 1);
    assert_eq!(bundle.character_features().len(), 2);

    let mut changed = prepared.clone();
    let surrounded = changed
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.surrounded")
        .unwrap();
    surrounded.semantic["contributions"][2]["value"]["value"] = json!(2);
    surrounded.fingerprint = materialized_definition_fingerprint(surrounded).unwrap();
    let changed_bundle = compile_prepared_play_bundle(changed).unwrap();
    assert_ne!(
        changed_bundle.artifact().artifact_id,
        bundle.artifact().artifact_id
    );

    let mut invalid_threshold = prepared.clone();
    let invalid_surrounded = invalid_threshold
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.surrounded")
        .unwrap();
    invalid_surrounded.semantic["contributions"][2]["predicate"]["minimumHostiles"] = json!(5);
    invalid_surrounded.fingerprint =
        materialized_definition_fingerprint(invalid_surrounded).unwrap();
    let invalid_threshold_failure = compile_prepared_play_bundle(invalid_threshold).unwrap_err();
    assert!(invalid_threshold_failure
        .diagnostics
        .iter()
        .any(|diagnostic| {
            diagnostic.code == "SCALAR_CONTRIBUTION_SURROUNDED_THRESHOLD_INVALID"
        }));

    let mut duplicate_identity = prepared.clone();
    let duplicate_flanking = duplicate_identity
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    duplicate_flanking.semantic["contributions"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
            "id": "flanking",
            "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
            "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
            "value": {"kind": "constant", "value": 1},
            "predicate": {"kind": "always"}
        }));
    duplicate_flanking.fingerprint =
        materialized_definition_fingerprint(duplicate_flanking).unwrap();
    let duplicate_identity_failure = compile_prepared_play_bundle(duplicate_identity).unwrap_err();
    assert!(duplicate_identity_failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SCALAR_CONTRIBUTIONS_NOT_CANONICAL"));

    let mut tampered_artifact = bundle.artifact().clone();
    tampered_artifact
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap()
        .semantic["contributions"][1]["value"]["value"] = json!(99);
    let tamper_failure = load_compiled_play_bundle(tampered_artifact).unwrap_err();
    assert!(
        tamper_failure
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "CONTENT_PACK_DEFINITION_FINGERPRINT_MISMATCH"),
        "{:?}",
        tamper_failure.diagnostics
    );

    let scenario = conditional_feature_scenario(&bundle);
    let mut session = RpgAuthoritySession::from_scenario(bundle.clone(), scenario).unwrap();
    let initial = session.checkpoint().unwrap();
    let mut source = attack_roll_source(&session, 5);
    let (outcome, entry) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.strike".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: None,
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Accepted(receipt) = outcome else {
        panic!("conditional attack should be accepted: {outcome:?}");
    };
    let contributions = receipt
        .events
        .iter()
        .find_map(|event| match event {
            RpgDomainEvent::AttackResolved {
                total,
                contribution_ledger,
                ..
            } => {
                assert_eq!(*total, 16);
                Some(contribution_ledger)
            }
            _ => None,
        })
        .expect("attack event retains contribution evidence");
    assert_eq!(
        contributions
            .candidates
            .iter()
            .map(|contribution| (
                contribution.contribution_id.as_str(),
                contribution.applied_value,
                contribution.disposition.clone(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("action-context", 2, RpgContributionDisposition::Applied),
            ("bonus-a", 2, RpgContributionDisposition::Applied),
            ("cell", 1, RpgContributionDisposition::Applied),
            ("distance", 1, RpgContributionDisposition::Applied),
            ("flanking", 2, RpgContributionDisposition::Applied),
            (
                "inapplicable",
                0,
                RpgContributionDisposition::Inapplicable {
                    reason: "distance.greaterThan.10.actual.1".to_owned(),
                },
            ),
            ("penalty-a", -1, RpgContributionDisposition::Applied),
            (
                "bonus-b",
                0,
                RpgContributionDisposition::Suppressed {
                    policy: RpgContributionStackingPolicy::SignedExtremes,
                    retained_contribution_ids: vec![
                        "feature.flanking#-:bonus-a".to_owned(),
                        "feature.flanking#-:penalty-a".to_owned(),
                    ],
                },
            ),
            (
                "penalty-b",
                0,
                RpgContributionDisposition::Suppressed {
                    policy: RpgContributionStackingPolicy::SignedExtremes,
                    retained_contribution_ids: vec![
                        "feature.flanking#-:bonus-a".to_owned(),
                        "feature.flanking#-:penalty-a".to_owned(),
                    ],
                },
            ),
            ("surrounded", 1, RpgContributionDisposition::Applied),
        ]
    );

    let replayed = RpgAuthoritySession::replay(initial, &[entry]).unwrap();
    assert_eq!(
        replayed.state_hash().unwrap(),
        session.state_hash().unwrap()
    );
    assert_eq!(replayed.encounter_view().log, session.encounter_view().log);
    let accepted_hash = session.state_hash().unwrap();
    let mut stale_source = attack_roll_source(&session, 5);
    let (stale_outcome, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.strike".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: None,
            },
            &mut stale_source,
        )
        .unwrap();
    let RpgCommandOutcome::Rejected(stale_rejection) = stale_outcome else {
        panic!("stale contribution action must reject: {stale_outcome:?}");
    };
    assert_eq!(stale_rejection.code, "RPG_SESSION_REVISION_MISMATCH");
    assert_eq!(session.state_hash().unwrap(), accepted_hash);
    let mut without_cell_capability = conditional_feature_scenario(&bundle);
    without_cell_capability.board.cells.clear();
    let without_cell_ledger = conditional_attack_ledger(bundle.clone(), without_cell_capability);
    let cell_decision = without_cell_ledger
        .candidates
        .iter()
        .find(|candidate| candidate.contribution_id == "cell")
        .unwrap();
    assert_eq!(cell_decision.applied_value, 0);
    assert_eq!(
        cell_decision.disposition,
        RpgContributionDisposition::Inapplicable {
            reason: "cellCapability.actor.required.terrain.high-ground".to_owned(),
        }
    );
    let mut out_of_domain = conditional_feature_scenario(&bundle);
    let actor_power = out_of_domain
        .participants
        .iter_mut()
        .find(|participant| participant.id == "actor")
        .unwrap()
        .capabilities
        .iter_mut()
        .find(|capability| {
            matches!(
                capability,
                RpgInitialCapability::Stat { id, .. } if id == "power"
            )
        })
        .unwrap();
    *actor_power = RpgInitialCapability::Stat {
        id: "power".to_owned(),
        value: 100,
    };
    let mut rejected_session =
        RpgAuthoritySession::from_scenario(bundle.clone(), out_of_domain).unwrap();
    let rejected_initial = rejected_session.checkpoint().unwrap();
    let rejected_hash = rejected_session.state_hash().unwrap();
    let mut rejected_source = attack_roll_source(&rejected_session, 5);
    let (rejected, rejected_entry) = rejected_session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.strike".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: None,
            },
            &mut rejected_source,
        )
        .unwrap();
    let RpgCommandOutcome::Rejected(rejection) = rejected else {
        panic!("out-of-domain contribution total must reject: {rejected:?}");
    };
    assert_eq!(rejection.code, "RPG_RUNTIME_CONTRIBUTION_DOMAIN_EXCEEDED");
    assert_eq!(rejected_session.state_hash().unwrap(), rejected_hash);
    assert_eq!(rejected_session.state().revision(), 0);
    assert_eq!(rejected_session.accepted_random_values(), 0);
    assert!(rejected_session.encounter_view().log.is_empty());
    let replayed_rejection =
        RpgAuthoritySession::replay(rejected_initial, &[rejected_entry]).unwrap();
    assert_eq!(replayed_rejection.state_hash().unwrap(), rejected_hash);
    assert!(replayed_rejection.encounter_view().log.is_empty());
    let mut tampered_checkpoint = session.checkpoint().unwrap();
    tampered_checkpoint
        .state
        .entities
        .iter_mut()
        .find(|entity| entity.id == "actor")
        .unwrap()
        .character_feature_ids
        .clear();
    let checkpoint_failure =
        RpgAuthoritySession::restore_checkpoint(tampered_checkpoint).unwrap_err();
    assert_eq!(
        checkpoint_failure.diagnostics[0].code,
        "RPG_CHECKPOINT_STATE_HASH_MISMATCH"
    );

    let mut without_flank = conditional_feature_scenario(&bundle);
    without_flank
        .participants
        .iter_mut()
        .find(|participant| participant.id == "ally")
        .unwrap()
        .position = GridPosition { x: 4, y: 1 };
    assert_eq!(
        conditional_attack_feature_sources(bundle.clone(), without_flank),
        vec!["feature.surrounded"]
    );

    let mut without_surround = conditional_feature_scenario(&bundle);
    without_surround
        .participants
        .iter_mut()
        .find(|participant| participant.id == "hostile-two")
        .unwrap()
        .position = GridPosition { x: 4, y: 0 };
    assert_eq!(
        conditional_attack_feature_sources(bundle.clone(), without_surround),
        vec!["feature.flanking"]
    );

    let mut defeated_ally = conditional_feature_scenario(&bundle);
    defeated_ally
        .participants
        .iter_mut()
        .find(|participant| participant.id == "ally")
        .unwrap()
        .capabilities[0] = RpgInitialCapability::Vitality {
        value: BoundedValue {
            current: 0,
            max: 20,
        },
    };
    assert_eq!(
        conditional_attack_feature_sources(bundle.clone(), defeated_ally),
        vec!["feature.surrounded"]
    );

    let mut wrong_team_ally = conditional_feature_scenario(&bundle);
    wrong_team_ally
        .participants
        .iter_mut()
        .find(|participant| participant.id == "ally")
        .unwrap()
        .team_id = RpgTeamId::enemy();
    assert_eq!(
        conditional_attack_feature_sources(bundle.clone(), wrong_team_ally),
        vec!["feature.surrounded"]
    );

    let mut defeated_hostile = conditional_feature_scenario(&bundle);
    defeated_hostile
        .participants
        .iter_mut()
        .find(|participant| participant.id == "hostile-two")
        .unwrap()
        .capabilities[0] = RpgInitialCapability::Vitality {
        value: BoundedValue {
            current: 0,
            max: 20,
        },
    };
    assert_eq!(
        conditional_attack_feature_sources(bundle.clone(), defeated_hostile),
        vec!["feature.flanking"]
    );

    let mut duplicate_selection = conditional_feature_scenario(&bundle);
    duplicate_selection.participants[0].feature_definition_ids =
        vec!["feature.flanking".to_owned(), "feature.flanking".to_owned()];
    let duplicate_failure =
        RpgAuthoritySession::from_scenario(bundle, duplicate_selection).unwrap_err();
    assert!(duplicate_failure
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "RPG_SCENARIO_FEATURE_DEFINITIONS_NOT_CANONICAL" }));
}

#[test]
fn public_facade_rejects_noncanonical_value_and_numeric_domain_requirements() {
    let mut duplicated = healing_prepared();
    duplicated.content_requirements.values = vec![
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "power".to_owned(),
        },
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "power".to_owned(),
        },
    ];
    duplicated.content_requirements.numeric_domains =
        vec!["attribute".to_owned(), "attribute".to_owned()];
    let duplicate_failure = compile_prepared_play_bundle(duplicated).unwrap_err();
    assert!(duplicate_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL"
            && diagnostic.path == "$.contentRequirements.values[1]"
    }));
    assert!(duplicate_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL"
            && diagnostic.path == "$.contentRequirements.numericDomains[1]"
    }));

    let mut reordered = healing_prepared();
    reordered.content_requirements.values = vec![
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "wisdom".to_owned(),
        },
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "power".to_owned(),
        },
    ];
    reordered.content_requirements.numeric_domains =
        vec!["bonus".to_owned(), "attribute".to_owned()];
    let reordered_failure = compile_prepared_play_bundle(reordered).unwrap_err();
    assert!(reordered_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL"
            && diagnostic.path == "$.contentRequirements.values[1]"
    }));
    assert!(reordered_failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "PLAY_BUNDLE_REQUIREMENTS_NOT_CANONICAL"
            && diagnostic.path == "$.contentRequirements.numericDomains[1]"
    }));
}

#[test]
fn rust_derives_ruleset_values_with_floor_division_and_protects_checkpoints() {
    let bundle = compile_prepared_play_bundle(derived_value_prepared(2)).unwrap();
    let artifact_id = bundle.artifact().artifact_id.clone();
    let scenario = derived_value_scenario(&bundle, 1);
    let session = RpgAuthoritySession::from_scenario(bundle, scenario).unwrap();
    let actor = session.state().entity("actor").unwrap();
    assert_eq!(actor.stat("score"), Some(1));
    assert_eq!(actor.stat("modifier"), Some(-5));

    let mut checkpoint = session.checkpoint().unwrap();
    checkpoint.state.entities[0]
        .stats
        .iter_mut()
        .find(|stat| stat.id == "modifier")
        .unwrap()
        .value = -4;
    let failure = RpgAuthoritySession::restore_checkpoint(checkpoint).unwrap_err();
    assert_eq!(
        failure.diagnostics[0].code,
        "RPG_CHECKPOINT_DERIVED_RULESET_VALUE_MISMATCH"
    );

    let changed = compile_prepared_play_bundle(derived_value_prepared(3)).unwrap();
    assert_ne!(artifact_id, changed.artifact().artifact_id);
}

#[test]
fn rust_rejects_contextual_contribution_contract_tampering() {
    let prepared = conditional_feature_prepared();

    let mut unknown_selector = prepared.clone();
    let flanking = unknown_selector
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    flanking.semantic["contributions"][1]["selector"]["id"] = json!("unknown-selector");
    flanking.fingerprint = materialized_definition_fingerprint(flanking).unwrap();
    let failure = compile_prepared_play_bundle(unknown_selector).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SCALAR_CONTRIBUTION_SELECTOR_UNKNOWN"));

    let mut unsupported_group_version = prepared.clone();
    unsupported_group_version
        .ruleset
        .provides
        .contribution_stacking_groups[0]
        .version = 2;
    let failure = compile_prepared_play_bundle(unsupported_group_version).unwrap_err();
    assert!(failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "RULESET_CONTRIBUTION_STACKING_GROUPS_NOT_CANONICAL"
    }));

    let mut predicate_bound = prepared.clone();
    let flanking = predicate_bound
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    let mut predicate = json!({"kind": "always"});
    for _ in 0..17 {
        predicate = json!({"kind": "not", "predicate": predicate});
    }
    flanking.semantic["contributions"][1]["predicate"] = predicate;
    flanking.fingerprint = materialized_definition_fingerprint(flanking).unwrap();
    let failure = compile_prepared_play_bundle(predicate_bound).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "SCALAR_CONTRIBUTION_PREDICATE_BOUNDS_EXCEEDED" }));

    let mut source_bound = prepared.clone();
    let flanking = source_bound
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    let template = flanking.semantic["contributions"][1].clone();
    flanking.semantic["contributions"] = json!((0..33)
        .map(|index| {
            let mut contribution = template.clone();
            contribution["id"] = json!(format!("bounded-{index:02}"));
            contribution
        })
        .collect::<Vec<_>>());
    flanking.fingerprint = materialized_definition_fingerprint(flanking).unwrap();
    let failure = compile_prepared_play_bundle(source_bound).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "CHARACTER_FEATURE_CONTRIBUTIONS_INVALID"));

    let mut missing_edge = prepared.clone();
    let flanking = missing_edge
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    flanking.references.clear();
    flanking.fingerprint = materialized_definition_fingerprint(flanking).unwrap();
    let high_ground = missing_edge
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "terrain.high-ground")
        .unwrap();
    high_ground.visibility = MaterializedContentVisibility::Exported;
    high_ground.fingerprint = materialized_definition_fingerprint(high_ground).unwrap();
    missing_edge
        .exported_roots
        .push("terrain.high-ground".to_owned());
    missing_edge
        .relationships
        .push(ContentRelationshipProvenance {
            kind: ContentRelationshipKind::Exports,
            source: "consumer.package@1.0.0".to_owned(),
            target: "terrain.high-ground".to_owned(),
            order: 4,
        });
    let failure = compile_prepared_play_bundle(missing_edge).unwrap_err();
    assert!(failure.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "SCALAR_CONTRIBUTION_DEFINITION_REFERENCE_UNDECLARED"
    }));

    let bundle = compile_prepared_play_bundle(prepared).unwrap();
    let mut tampered_artifact = bundle.artifact().clone();
    let feature = tampered_artifact
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "feature.flanking")
        .unwrap();
    feature.semantic["contributions"][1]["selector"]["id"] = json!("unknown-selector");
    feature.fingerprint = materialized_definition_fingerprint(feature).unwrap();
    let failure = load_compiled_play_bundle(tampered_artifact).unwrap_err();
    assert!(
        failure.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_str(),
                "PLAY_BUNDLE_SEMANTIC_FINGERPRINT_MISMATCH"
                    | "PLAY_BUNDLE_ARTIFACT_ID_MISMATCH"
                    | "CONTENT_PACK_DEFINITION_COMMITMENT_MISMATCH"
                    | "SCALAR_CONTRIBUTION_SELECTOR_UNKNOWN"
            )
        }),
        "{:?}",
        failure.diagnostics
    );
}

#[test]
fn rust_rejects_supplied_unknown_and_cyclic_derived_values_before_session_state() {
    let bundle = compile_prepared_play_bundle(derived_value_prepared(2)).unwrap();
    let mut supplied = derived_value_scenario(&bundle, 16);
    supplied.participants[0]
        .capabilities
        .push(RpgInitialCapability::Stat {
            id: "modifier".to_owned(),
            value: 3,
        });
    let failure = RpgAuthoritySession::from_scenario(bundle, supplied).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "RPG_SCENARIO_DERIVED_RULESET_VALUE_SUPPLIED" }));

    let mut unknown = derived_value_prepared(2);
    let RulesetValueSource::Derived { formula } = &mut unknown.ruleset.provides.values[0].source
    else {
        panic!("modifier is derived");
    };
    formula.expression = RulesetValueExpression::ReadValue {
        ruleset_id: "consumer.rules".to_owned(),
        value_kind: RulesetValueKind::Stat,
        value_id: "missing".to_owned(),
    };
    let failure = compile_prepared_play_bundle(unknown).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "RULESET_VALUE_FORMULA_REFERENCE_MISSING" }));

    let mut cyclic = derived_value_prepared(2);
    cyclic.ruleset.provides.values[1].source = RulesetValueSource::Derived {
        formula: ruleset_value_formula(RulesetValueExpression::ReadValue {
            ruleset_id: "consumer.rules".to_owned(),
            value_kind: RulesetValueKind::Stat,
            value_id: "modifier".to_owned(),
        }),
    };
    let failure = compile_prepared_play_bundle(cyclic).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "RULESET_VALUE_DERIVATION_CYCLE"));
}

#[test]
fn rust_validates_and_exposes_typed_participant_profiles() {
    let prepared = participant_profile_prepared();
    let bundle = compile_prepared_play_bundle(prepared.clone()).unwrap();
    let profiles = bundle.participant_profiles();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].definition_id, "profile.healer");
    assert_eq!(profiles[0].profile_id, "healer");
    assert_eq!(profiles[0].definition_ids, ["action.heal"]);

    let scenario = RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 1,
            height: 1,
            cells: Vec::new(),
        },
        participants: vec![participant("healer", "Healer", RpgTeamId::ally(), 0, 10)],
        turn: RpgTurnInitialization {
            initiative_order: vec!["healer".to_owned()],
            current_actor_id: "healer".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "consumer.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "consumer.roll-tape".to_owned(),
            source_version: 1,
        },
    };
    RpgAuthoritySession::from_scenario(bundle, scenario).unwrap();

    let mut malformed = prepared;
    malformed.materialized_definitions[1].semantic["data"]["commands"] = json!([]);
    malformed.materialized_definitions[1].fingerprint =
        materialized_definition_fingerprint(&malformed.materialized_definitions[1]).unwrap();
    let failure = compile_prepared_play_bundle(malformed).unwrap_err();
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "PARTICIPANT_PROFILE_DATA_INVALID"));
}

fn participant_profile_prepared() -> PreparedPlayBundle {
    let mut prepared = healing_prepared();
    let provenance = ContentDefinitionProvenance {
        definition_id: "profile.healer".to_owned(),
        package_id: "consumer.package".to_owned(),
        package_version: "1.0.0".to_owned(),
        source: ContentSourceLocation {
            module: "profiles/healer.ts".to_owned(),
            declaration: "healer".to_owned(),
        },
    };
    let mut profile = MaterializedContentDefinition {
        id: "profile.healer".to_owned(),
        kind: MaterializedContentDefinitionKind::Support,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "catalog": "participantProfile",
            "id": "healer",
            "data": {
                "schema": {
                    "identity": "asha.rpg.participant-profile",
                    "version": 2
                },
                "role": "player",
                "definitionIds": ["action.heal"],
                "classDefinitionId": null,
                "featureDefinitionIds": [],
                "capabilities": [{
                    "owner": "vitality",
                    "value": {"current": 10, "max": 10}
                }]
            }
        }),
        presentation: json!({"label": "Healer", "description": "A typed setup profile."}),
        references: vec!["action.heal".to_owned()],
        provenance: provenance.clone(),
        fingerprint: String::new(),
    };
    profile.fingerprint = materialized_definition_fingerprint(&profile).unwrap();
    prepared.materialized_definitions.push(profile);
    prepared.exported_roots.push("profile.healer".to_owned());
    prepared.definition_provenance.push(provenance);
    prepared.relationships.push(ContentRelationshipProvenance {
        kind: ContentRelationshipKind::Exports,
        source: "consumer.package@1.0.0".to_owned(),
        target: "profile.healer".to_owned(),
        order: 1,
    });
    prepared
}

fn derived_value_prepared(divisor: i64) -> PreparedPlayBundle {
    let mut prepared = healing_prepared();
    prepared.ruleset.provides.capabilities.insert(
        0,
        VersionedRpgRequirement {
            id: "capability.stats".to_owned(),
            version: 1,
        },
    );
    prepared.content_requirements.capabilities.insert(
        0,
        VersionedRpgRequirement {
            id: "capability.stats".to_owned(),
            version: 1,
        },
    );
    prepared.ruleset.provides.numeric_domains = vec![RulesetNumericDomain {
        id: "integer".to_owned(),
        minimum: -100,
        maximum: 100,
    }];
    prepared.ruleset.provides.values = vec![
        RulesetValueContract {
            kind: RulesetValueKind::Stat,
            id: "modifier".to_owned(),
            label: "Modifier".to_owned(),
            numeric_domain_id: "integer".to_owned(),
            source: RulesetValueSource::Derived {
                formula: ruleset_value_formula(RulesetValueExpression::FloorDivide {
                    dividend: Box::new(RulesetValueExpression::Subtract {
                        minuend: Box::new(RulesetValueExpression::ReadValue {
                            ruleset_id: "consumer.rules".to_owned(),
                            value_kind: RulesetValueKind::Stat,
                            value_id: "score".to_owned(),
                        }),
                        subtrahend: Box::new(RulesetValueExpression::Constant { value: 10 }),
                    }),
                    divisor: Box::new(RulesetValueExpression::Constant { value: divisor }),
                }),
            },
        },
        RulesetValueContract {
            kind: RulesetValueKind::Stat,
            id: "score".to_owned(),
            label: "Score".to_owned(),
            numeric_domain_id: "integer".to_owned(),
            source: RulesetValueSource::Input,
        },
    ];
    prepared.content_requirements.values = vec![
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "modifier".to_owned(),
        },
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "score".to_owned(),
        },
    ];
    prepared.content_requirements.numeric_domains = vec!["integer".to_owned()];
    prepared
}

fn ruleset_value_formula(expression: RulesetValueExpression) -> RulesetValueFormula {
    RulesetValueFormula {
        schema: RulesetValueFormulaSchema {
            identity: "asha.rpg.ruleset-value-formula".to_owned(),
            version: 1,
        },
        expression,
    }
}

fn derived_value_scenario(bundle: &asha_rpg::CompiledPlayBundle, score: i32) -> RpgScenario {
    let mut actor = participant("actor", "Actor", RpgTeamId::ally(), 0, 20);
    actor.capabilities.push(RpgInitialCapability::Stat {
        id: "score".to_owned(),
        value: score,
    });
    RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 2,
            height: 1,
            cells: Vec::new(),
        },
        participants: vec![actor],
        turn: RpgTurnInitialization {
            initiative_order: vec!["actor".to_owned()],
            current_actor_id: "actor".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "consumer.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "consumer.roll-tape".to_owned(),
            source_version: 1,
        },
    }
}

fn participant(
    id: &str,
    label: &str,
    team_id: RpgTeamId,
    x: u32,
    vitality: i32,
) -> RpgParticipantSetup {
    RpgParticipantSetup {
        id: id.to_owned(),
        label: label.to_owned(),
        team_id,
        position: GridPosition { x, y: 0 },
        definition_ids: vec!["action.heal".to_owned()],
        class_definition_id: None,
        feature_definition_ids: Vec::new(),
        items: Vec::new(),
        equipment: Vec::new(),
        capabilities: vec![RpgInitialCapability::Vitality {
            value: BoundedValue {
                current: vitality,
                max: 20,
            },
        }],
    }
}

fn conditional_feature_prepared() -> PreparedPlayBundle {
    let mut prepared = healing_prepared();
    let provenance = |definition_id: &str, module: &str| ContentDefinitionProvenance {
        definition_id: definition_id.to_owned(),
        package_id: "consumer.package".to_owned(),
        package_version: "1.0.0".to_owned(),
        source: ContentSourceLocation {
            module: module.to_owned(),
            declaration: definition_id.replace('.', "_"),
        },
    };
    let mut action = MaterializedContentDefinition {
        id: "action.strike".to_owned(),
        kind: MaterializedContentDefinitionKind::Action,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.action-definition", "version": 1},
            "kind": "inline",
            "action": {
                "id": "action.strike",
                "name": "Strike",
                "sourcePath": "actions/strike.ts#strike",
                "tags": ["attack"],
                "targets": {
                    "kind": "participant",
                    "team": "hostile",
                    "maximumRange": 1,
                    "maximumTargets": 1
                },
                "check": {
                    "kind": "attack",
                    "modifier": {"kind": "constant", "value": 3},
                    "defenseId": "guard",
                    "contributionSelector": {
                        "rulesetId": "consumer.rules",
                        "id": "attack-total"
                    }
                },
                "rollScope": "perTarget",
                "costs": [],
                "program": {
                    "kind": "atomic",
                    "body": {
                        "kind": "onCheck",
                        "hit": {
                            "kind": "operation",
                            "operation": {
                                "kind": "heal",
                                "amount": {"kind": "constant", "value": 1}
                            }
                        }
                    }
                }
            }
        }),
        presentation: json!({"label": "Strike"}),
        references: Vec::new(),
        provenance: provenance("action.strike", "actions/strike.ts"),
        fingerprint: String::new(),
    };
    let mut class = MaterializedContentDefinition {
        id: "class.vanguard".to_owned(),
        kind: MaterializedContentDefinitionKind::CharacterClass,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.character-class", "version": 1},
            "featureDefinitionIds": ["feature.flanking", "feature.surrounded"]
        }),
        presentation: json!({"label": "Vanguard"}),
        references: vec![
            "feature.flanking".to_owned(),
            "feature.surrounded".to_owned(),
        ],
        provenance: provenance("class.vanguard", "classes/vanguard.ts"),
        fingerprint: String::new(),
    };
    let mut flanking = MaterializedContentDefinition {
        id: "feature.flanking".to_owned(),
        kind: MaterializedContentDefinitionKind::CharacterFeature,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.character-feature", "version": 2},
            "contributions": [
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "action-context",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {
                        "kind": "readValue",
                        "subject": "actor",
                        "rulesetId": "consumer.rules",
                        "valueKind": "stat",
                        "valueId": "power"
                    },
                    "predicate": {
                        "kind": "all",
                        "predicates": [
                            {"kind": "actionTag", "tag": "attack"},
                            {"kind": "actorIsTarget", "expected": false},
                            {"kind": "teamRelation", "relation": "different"},
                            {"kind": "living", "subject": "actor", "expected": true},
                            {"kind": "living", "subject": "target", "expected": true},
                            {
                                "kind": "namedValue",
                                "subject": "target",
                                "rulesetId": "consumer.rules",
                                "valueKind": "defense",
                                "valueId": "guard",
                                "comparison": "greaterThanOrEqual",
                                "value": 10
                            }
                        ]
                    }
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "bonus-a",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "signed"},
                    "value": {"kind": "constant", "value": 2},
                    "predicate": {"kind": "always"}
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "cell",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {"kind": "constant", "value": 1},
                    "predicate": {
                        "kind": "cellCapability",
                        "subject": "actor",
                        "capabilityId": "terrain.high-ground"
                    }
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "distance",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {"kind": "constant", "value": 1},
                    "predicate": {
                        "kind": "distance",
                        "comparison": "equal",
                        "value": 1
                    }
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "flanking",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {"kind": "constant", "value": 2},
                    "predicate": {"kind": "actorFlanksTarget"}
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "inapplicable",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {"kind": "constant", "value": 9},
                    "predicate": {
                        "kind": "distance",
                        "comparison": "greaterThan",
                        "value": 10
                    }
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "penalty-a",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "signed"},
                    "value": {"kind": "constant", "value": -1},
                    "predicate": {"kind": "always"}
                }
            ]
        }),
        presentation: json!({"label": "Flanking Discipline"}),
        references: vec!["terrain.high-ground".to_owned()],
        provenance: provenance("feature.flanking", "features/flanking.ts"),
        fingerprint: String::new(),
    };
    let mut surrounded = MaterializedContentDefinition {
        id: "feature.surrounded".to_owned(),
        kind: MaterializedContentDefinitionKind::CharacterFeature,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.character-feature", "version": 2},
            "contributions": [
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "bonus-b",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "signed"},
                    "value": {"kind": "constant", "value": 2},
                    "predicate": {"kind": "always"}
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "penalty-b",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "signed"},
                    "value": {"kind": "constant", "value": -1},
                    "predicate": {"kind": "always"}
                },
                {
                    "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
                    "id": "surrounded",
                    "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
                    "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
                    "value": {"kind": "constant", "value": 1},
                    "predicate": {
                        "kind": "actorSurrounded",
                        "minimumHostiles": 2
                    }
                }
            ]
        }),
        presentation: json!({"label": "Against the Press"}),
        references: Vec::new(),
        provenance: provenance("feature.surrounded", "features/surrounded.ts"),
        fingerprint: String::new(),
    };
    let mut high_ground = MaterializedContentDefinition {
        id: "terrain.high-ground".to_owned(),
        kind: MaterializedContentDefinitionKind::Support,
        visibility: MaterializedContentVisibility::Support,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "catalog": "cell-capability",
            "data": {"kind": "flag"}
        }),
        presentation: json!({"label": "High ground"}),
        references: Vec::new(),
        provenance: provenance("terrain.high-ground", "terrain/high-ground.ts"),
        fingerprint: String::new(),
    };
    for definition in [
        &mut action,
        &mut class,
        &mut flanking,
        &mut surrounded,
        &mut high_ground,
    ] {
        definition.fingerprint = materialized_definition_fingerprint(definition).unwrap();
    }

    prepared.play_bundle_identity.id = "consumer.conditional-features".to_owned();
    prepared.ruleset.provides.capabilities = vec![
        VersionedRpgRequirement {
            id: "capability.defenses".to_owned(),
            version: 1,
        },
        VersionedRpgRequirement {
            id: "capability.random".to_owned(),
            version: 1,
        },
        VersionedRpgRequirement {
            id: "capability.stats".to_owned(),
            version: 1,
        },
        VersionedRpgRequirement {
            id: "capability.vitality".to_owned(),
            version: 1,
        },
    ];
    prepared.ruleset.provides.numeric_domains = vec![RulesetNumericDomain {
        id: "check-total".to_owned(),
        minimum: -100,
        maximum: 100,
    }];
    prepared.ruleset.provides.calculation_selectors = vec![RulesetCalculationSelectorContract {
        id: "attack-total".to_owned(),
        version: 1,
        label: "Attack total".to_owned(),
        numeric_domain_id: "check-total".to_owned(),
    }];
    prepared.ruleset.provides.contribution_stacking_groups = vec![
        RulesetContributionStackingGroupContract {
            id: "circumstance".to_owned(),
            version: 1,
            label: "Circumstance".to_owned(),
            policy: RpgContributionStackingPolicy::Sum,
        },
        RulesetContributionStackingGroupContract {
            id: "signed".to_owned(),
            version: 1,
            label: "Signed extremes".to_owned(),
            policy: RpgContributionStackingPolicy::SignedExtremes,
        },
    ];
    prepared.ruleset.provides.values = vec![
        RulesetValueContract {
            kind: RulesetValueKind::Defense,
            id: "guard".to_owned(),
            label: "Guard".to_owned(),
            numeric_domain_id: "check-total".to_owned(),
            source: RulesetValueSource::Input,
        },
        RulesetValueContract {
            kind: RulesetValueKind::Stat,
            id: "power".to_owned(),
            label: "Power".to_owned(),
            numeric_domain_id: "check-total".to_owned(),
            source: RulesetValueSource::Input,
        },
    ];
    prepared.content_requirements.capabilities = prepared.ruleset.provides.capabilities.clone();
    prepared.content_requirements.values = vec![
        ContentValueRequirement {
            kind: RulesetValueKind::Defense,
            id: "guard".to_owned(),
        },
        ContentValueRequirement {
            kind: RulesetValueKind::Stat,
            id: "power".to_owned(),
        },
    ];
    prepared.content_requirements.numeric_domains = vec!["check-total".to_owned()];
    prepared.exported_roots = vec![
        "action.strike".to_owned(),
        "class.vanguard".to_owned(),
        "feature.flanking".to_owned(),
        "feature.surrounded".to_owned(),
    ];
    prepared.materialized_definitions = vec![action, class, flanking, surrounded, high_ground];
    prepared.definition_provenance = prepared
        .materialized_definitions
        .iter()
        .map(|definition| definition.provenance.clone())
        .collect();
    prepared.relationships = prepared
        .exported_roots
        .iter()
        .enumerate()
        .map(|(order, target)| ContentRelationshipProvenance {
            kind: ContentRelationshipKind::Exports,
            source: "consumer.package@1.0.0".to_owned(),
            target: target.clone(),
            order,
        })
        .collect();
    prepared
}

fn conditional_feature_scenario(bundle: &asha_rpg::CompiledPlayBundle) -> RpgScenario {
    let mut actor = participant("actor", "Actor", RpgTeamId::ally(), 1, 20);
    actor.position = GridPosition { x: 1, y: 1 };
    actor.definition_ids = vec!["action.strike".to_owned()];
    actor.class_definition_id = Some("class.vanguard".to_owned());
    actor.feature_definition_ids = vec![
        "feature.flanking".to_owned(),
        "feature.surrounded".to_owned(),
    ];
    actor.capabilities.push(RpgInitialCapability::Defense {
        id: "guard".to_owned(),
        value: 10,
    });
    actor.capabilities.push(RpgInitialCapability::Stat {
        id: "power".to_owned(),
        value: 2,
    });

    let mut ally = participant("ally", "Ally", RpgTeamId::ally(), 3, 20);
    ally.position = GridPosition { x: 3, y: 1 };
    ally.definition_ids = vec!["action.strike".to_owned()];
    ally.capabilities.push(RpgInitialCapability::Defense {
        id: "guard".to_owned(),
        value: 10,
    });

    let mut target = participant("target", "Target", RpgTeamId::enemy(), 2, 10);
    target.position = GridPosition { x: 2, y: 1 };
    target.definition_ids = vec!["action.strike".to_owned()];
    target.capabilities.push(RpgInitialCapability::Defense {
        id: "guard".to_owned(),
        value: 10,
    });

    let mut hostile_two = participant("hostile-two", "Hostile Two", RpgTeamId::enemy(), 1, 20);
    hostile_two.position = GridPosition { x: 1, y: 0 };
    hostile_two.definition_ids = vec!["action.strike".to_owned()];
    hostile_two
        .capabilities
        .push(RpgInitialCapability::Defense {
            id: "guard".to_owned(),
            value: 10,
        });

    RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 5,
            height: 3,
            cells: vec![RpgCellSetup {
                id: "cell.actor".to_owned(),
                position: GridPosition { x: 1, y: 1 },
                capabilities: vec![RpgCellCapabilitySetup {
                    id: "terrain".to_owned(),
                    version: 1,
                    definition_id: Some("terrain.high-ground".to_owned()),
                    value: RpgCellCapabilityValue::Flag { value: true },
                }],
            }],
        },
        participants: vec![actor, ally, target, hostile_two],
        turn: RpgTurnInitialization {
            initiative_order: vec![
                "actor".to_owned(),
                "ally".to_owned(),
                "target".to_owned(),
                "hostile-two".to_owned(),
            ],
            current_actor_id: "actor".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "consumer.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "consumer.roll-tape".to_owned(),
            source_version: 1,
        },
    }
}

fn attack_roll_source(session: &RpgAuthoritySession, roll: u32) -> RpgRollTapeSource {
    RpgRollTapeSource::new(
        session.scenario().random_source.clone(),
        [RpgRollTapeEntry {
            request: RpgRandomRequest {
                kind: RpgRandomRequestKind::AttackCheck,
                count: 1,
                sides: 20,
                path: "$.action.check.targets[0].roll".to_owned(),
            },
            values: vec![roll],
        }],
    )
}

fn conditional_attack_feature_sources(
    bundle: asha_rpg::CompiledPlayBundle,
    scenario: RpgScenario,
) -> Vec<&'static str> {
    conditional_attack_ledger(bundle, scenario)
        .candidates
        .iter()
        .filter_map(|contribution| {
            if contribution.applied_value == 0 {
                return None;
            }
            match contribution.contribution_id.as_str() {
                "flanking" => Some("feature.flanking"),
                "surrounded" => Some("feature.surrounded"),
                _ => None,
            }
        })
        .collect()
}

fn conditional_attack_ledger(
    bundle: asha_rpg::CompiledPlayBundle,
    scenario: RpgScenario,
) -> RpgScalarContributionLedger {
    let mut session = RpgAuthoritySession::from_scenario(bundle, scenario).unwrap();
    let mut source = attack_roll_source(&session, 5);
    let (outcome, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.strike".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["target".to_owned()],
                item_binding: None,
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Accepted(receipt) = outcome else {
        panic!("counterexample attack should remain accepted: {outcome:?}");
    };
    receipt
        .events
        .iter()
        .find_map(|event| match event {
            RpgDomainEvent::AttackResolved {
                contribution_ledger,
                ..
            } => Some(contribution_ledger.clone()),
            _ => None,
        })
        .expect("accepted attack exposes its contribution ledger")
}

fn item_bound_bundle() -> asha_rpg::CompiledPlayBundle {
    compile_prepared_play_bundle(item_bound_prepared()).unwrap()
}

fn item_bound_scenario(bundle: &asha_rpg::CompiledPlayBundle) -> RpgScenario {
    let mut actor = participant("actor", "Actor", RpgTeamId::ally(), 0, 20);
    actor.definition_ids = vec!["action.item-heal".to_owned()];
    actor.items = vec![
        asha_rpg::RpgItemInstanceSetup {
            id: "kit.greater".to_owned(),
            definition_id: "item.greater-healing-kit".to_owned(),
        },
        asha_rpg::RpgItemInstanceSetup {
            id: "kit.standard".to_owned(),
            definition_id: "item.healing-kit".to_owned(),
        },
    ];
    actor.equipment = vec![
        asha_rpg::RpgEquipmentSlotSetup {
            slot_id: "hand.main".to_owned(),
            item_instance_id: "kit.greater".to_owned(),
        },
        asha_rpg::RpgEquipmentSlotSetup {
            slot_id: "hand.off".to_owned(),
            item_instance_id: "kit.standard".to_owned(),
        },
    ];
    let mut target = participant("target", "Target", RpgTeamId::ally(), 1, 10);
    target.definition_ids = vec!["action.item-heal".to_owned()];
    let mut opponent = participant("opponent", "Opponent", RpgTeamId::enemy(), 2, 20);
    opponent.definition_ids = vec!["action.item-heal".to_owned()];
    RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 3,
            height: 1,
            cells: Vec::new(),
        },
        participants: vec![actor, target, opponent],
        turn: RpgTurnInitialization {
            initiative_order: vec![
                "actor".to_owned(),
                "target".to_owned(),
                "opponent".to_owned(),
            ],
            current_actor_id: "actor".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "consumer.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "consumer.roll-tape".to_owned(),
            source_version: 1,
        },
    }
}

fn item_bound_prepared() -> PreparedPlayBundle {
    let mut prepared = healing_prepared();
    let package_id = "consumer.package";
    let package_version = "1.0.0";
    let provenance = |definition_id: &str, module: &str| ContentDefinitionProvenance {
        definition_id: definition_id.to_owned(),
        package_id: package_id.to_owned(),
        package_version: package_version.to_owned(),
        source: ContentSourceLocation {
            module: module.to_owned(),
            declaration: definition_id.replace('.', "_"),
        },
    };
    let mut action = MaterializedContentDefinition {
        id: "action.item-heal".to_owned(),
        kind: MaterializedContentDefinitionKind::Action,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.action-definition", "version": 1},
            "kind": "invocation",
            "tags": [],
            "procedureId": "procedure.item-heal",
            "procedureOwnerPackageId": package_id,
            "arguments": {
                "amount": {
                    "kind": "equippedItemAttribute",
                    "bindingId": "healing-kit",
                    "attributeId": "healing",
                    "parameterType": "boundedInteger"
                }
            },
            "binding": {
                "id": "healing-kit",
                "requiredTags": ["healing"],
                "requiredTraits": ["usable"],
                "slotIds": ["hand.main", "hand.off"]
            }
        }),
        presentation: json!({"label": "Use Healing Kit"}),
        references: vec!["procedure.item-heal".to_owned()],
        provenance: provenance("action.item-heal", "actions/item-heal.ts"),
        fingerprint: String::new(),
    };
    let mut greater_item = MaterializedContentDefinition {
        id: "item.greater-healing-kit".to_owned(),
        kind: MaterializedContentDefinitionKind::Item,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.item", "version": 2},
            "tags": ["healing"],
            "traits": ["usable"],
            "allowedSlots": ["hand.main", "hand.off"],
            "attributes": [{
                "type": "boundedInteger",
                "id": "healing",
                "value": 7,
                "minimum": 0,
                "maximum": 20
            }],
            "contributions": []
        }),
        presentation: json!({"label": "Greater Healing Kit"}),
        references: Vec::new(),
        provenance: provenance("item.greater-healing-kit", "items/healing-kits.ts"),
        fingerprint: String::new(),
    };
    let mut standard_item = MaterializedContentDefinition {
        id: "item.healing-kit".to_owned(),
        kind: MaterializedContentDefinitionKind::Item,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.item", "version": 2},
            "tags": ["healing"],
            "traits": ["usable"],
            "allowedSlots": ["hand.main", "hand.off"],
            "attributes": [{
                "type": "boundedInteger",
                "id": "healing",
                "value": 4,
                "minimum": 0,
                "maximum": 20
            }],
            "contributions": []
        }),
        presentation: json!({"label": "Healing Kit"}),
        references: Vec::new(),
        provenance: provenance("item.healing-kit", "items/healing-kits.ts"),
        fingerprint: String::new(),
    };
    let mut procedure = MaterializedContentDefinition {
        id: "procedure.item-heal".to_owned(),
        kind: MaterializedContentDefinitionKind::ActionProcedure,
        visibility: MaterializedContentVisibility::Support,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.action-procedure", "version": 1},
            "ownerPackageId": package_id,
            "parameters": [{
                "type": "boundedInteger",
                "id": "amount",
                "minimum": 0,
                "maximum": 20
            }],
            "implementation": {
                "kind": "inline",
                "template": {
                    "targets": {
                        "kind": "participant",
                        "team": "ally",
                        "maximumRange": 3,
                        "maximumTargets": 1
                    },
                    "check": {"kind": "noRoll"},
                    "rollScope": "none",
                    "costs": [],
                    "program": {
                        "kind": "atomic",
                        "body": {
                            "kind": "onCheck",
                            "noRoll": {
                                "kind": "operation",
                                "operation": {
                                    "kind": "heal",
                                    "amount": {
                                        "kind": "constant",
                                        "value": {
                                            "kind": "parameter",
                                            "parameterId": "amount",
                                            "parameterType": "boundedInteger"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }),
        presentation: json!({"label": "Item Heal Procedure"}),
        references: Vec::new(),
        provenance: provenance("procedure.item-heal", "procedures/item-heal.ts"),
        fingerprint: String::new(),
    };
    for definition in [
        &mut action,
        &mut greater_item,
        &mut standard_item,
        &mut procedure,
    ] {
        definition.fingerprint = materialized_definition_fingerprint(definition).unwrap();
    }
    prepared.play_bundle_identity.id = "consumer.item-bundle".to_owned();
    prepared.exported_roots = vec![
        "action.item-heal".to_owned(),
        "item.greater-healing-kit".to_owned(),
        "item.healing-kit".to_owned(),
    ];
    prepared.materialized_definitions = vec![action, greater_item, standard_item, procedure];
    prepared.definition_provenance = prepared
        .materialized_definitions
        .iter()
        .map(|definition| definition.provenance.clone())
        .collect();
    prepared.relationships = prepared
        .exported_roots
        .iter()
        .enumerate()
        .map(|(order, target)| ContentRelationshipProvenance {
            kind: ContentRelationshipKind::Exports,
            source: format!("{package_id}@{package_version}"),
            target: target.clone(),
            order,
        })
        .collect();
    prepared
}

fn item_attack_prepared() -> PreparedPlayBundle {
    let mut prepared = item_bound_prepared();
    prepared.play_bundle_identity.id = "consumer.item-attack-bundle".to_owned();
    prepared.ruleset.provides.capabilities = vec![
        VersionedRpgRequirement {
            id: "capability.defenses".to_owned(),
            version: 1,
        },
        VersionedRpgRequirement {
            id: "capability.random".to_owned(),
            version: 1,
        },
        VersionedRpgRequirement {
            id: "capability.vitality".to_owned(),
            version: 1,
        },
    ];
    prepared.ruleset.provides.numeric_domains = vec![RulesetNumericDomain {
        id: "check-total".to_owned(),
        minimum: -100,
        maximum: 100,
    }];
    prepared.ruleset.provides.values = vec![RulesetValueContract {
        kind: RulesetValueKind::Defense,
        id: "guard".to_owned(),
        label: "Guard".to_owned(),
        numeric_domain_id: "check-total".to_owned(),
        source: RulesetValueSource::Input,
    }];
    prepared.ruleset.provides.calculation_selectors = vec![RulesetCalculationSelectorContract {
        id: "attack-total".to_owned(),
        version: 1,
        label: "Attack total".to_owned(),
        numeric_domain_id: "check-total".to_owned(),
    }];
    prepared.ruleset.provides.contribution_stacking_groups =
        vec![RulesetContributionStackingGroupContract {
            id: "circumstance".to_owned(),
            version: 1,
            label: "Circumstance".to_owned(),
            policy: RpgContributionStackingPolicy::Sum,
        }];
    prepared.content_requirements.capabilities = prepared.ruleset.provides.capabilities.clone();
    prepared.content_requirements.values = vec![ContentValueRequirement {
        kind: RulesetValueKind::Defense,
        id: "guard".to_owned(),
    }];
    prepared.content_requirements.numeric_domains = vec!["check-total".to_owned()];

    let action = prepared
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "action.item-heal")
        .unwrap();
    action.semantic["tags"] = json!(["attack"]);

    let procedure = prepared
        .materialized_definitions
        .iter_mut()
        .find(|definition| definition.id == "procedure.item-heal")
        .unwrap();
    let template = &mut procedure.semantic["implementation"]["template"];
    template["targets"]["team"] = json!("hostile");
    template["check"] = json!({
        "kind": "attack",
        "modifier": {"kind": "constant", "value": 0},
        "defenseId": "guard",
        "contributionSelector": {
            "rulesetId": "consumer.rules",
            "id": "attack-total"
        }
    });
    template["rollScope"] = json!("perTarget");
    template["program"]["body"]["noRoll"].take();
    template["program"]["body"]["hit"] = json!({
        "kind": "operation",
        "operation": {
            "kind": "heal",
            "amount": {
                "kind": "constant",
                "value": {
                    "kind": "parameter",
                    "parameterId": "amount",
                    "parameterType": "boundedInteger"
                }
            }
        }
    });

    let contribution = json!({
        "schema": {"identity": "asha.rpg.scalar-contribution", "version": 1},
        "id": "precise",
        "selector": {"rulesetId": "consumer.rules", "id": "attack-total"},
        "stackingGroup": {"rulesetId": "consumer.rules", "id": "circumstance"},
        "value": {"kind": "constant", "value": 2},
        "predicate": {"kind": "boundItemTag", "tag": "precise"}
    });
    for definition in prepared
        .materialized_definitions
        .iter_mut()
        .filter(|definition| {
            matches!(
                definition.id.as_str(),
                "item.greater-healing-kit" | "item.healing-kit"
            )
        })
    {
        definition.semantic["contributions"] = json!([contribution.clone()]);
        if definition.id == "item.greater-healing-kit" {
            definition.semantic["tags"] = json!(["healing", "precise"]);
        }
    }
    for definition in &mut prepared.materialized_definitions {
        definition.fingerprint = materialized_definition_fingerprint(definition).unwrap();
    }
    prepared
}

fn item_attack_ledger(
    bundle: asha_rpg::CompiledPlayBundle,
    item_definition_id: &str,
) -> RpgScalarContributionLedger {
    let mut scenario = item_bound_scenario(&bundle);
    scenario
        .participants
        .iter_mut()
        .find(|participant| participant.id == "opponent")
        .unwrap()
        .capabilities
        .push(RpgInitialCapability::Defense {
            id: "guard".to_owned(),
            value: 10,
        });
    let mut session = RpgAuthoritySession::from_scenario(bundle, scenario).unwrap();
    let item_binding = session
        .encounter_view()
        .actions
        .iter()
        .filter_map(|action| action.item_binding.as_ref())
        .find(|binding| binding.item_definition_id == item_definition_id)
        .cloned()
        .unwrap();
    let mut source = attack_roll_source(&session, 10);
    let (outcome, _) = session
        .submit_with_random_source_recorded(
            RpgActionProposal {
                expected_revision: 0,
                action_id: "action.item-heal".to_owned(),
                actor_id: "actor".to_owned(),
                target_ids: vec!["opponent".to_owned()],
                item_binding: Some(item_binding),
            },
            &mut source,
        )
        .unwrap();
    let RpgCommandOutcome::Accepted(receipt) = outcome else {
        panic!("item attack should be accepted: {outcome:?}");
    };
    receipt
        .events
        .into_iter()
        .find_map(|event| match event {
            RpgDomainEvent::AttackResolved {
                contribution_ledger,
                ..
            } => Some(contribution_ledger),
            _ => None,
        })
        .unwrap()
}

fn healing_bundle() -> asha_rpg::CompiledPlayBundle {
    compile_prepared_play_bundle(healing_prepared()).unwrap()
}

fn healing_prepared() -> PreparedPlayBundle {
    let provenance = ContentDefinitionProvenance {
        definition_id: "action.heal".to_owned(),
        package_id: "consumer.package".to_owned(),
        package_version: "1.0.0".to_owned(),
        source: ContentSourceLocation {
            module: "actions/heal.ts".to_owned(),
            declaration: "heal".to_owned(),
        },
    };
    let mut action = MaterializedContentDefinition {
        id: "action.heal".to_owned(),
        kind: MaterializedContentDefinitionKind::Action,
        visibility: MaterializedContentVisibility::Exported,
        extension_policy: ContentExtensionPolicy::Sealed,
        semantic: json!({
            "schema": {"identity": "asha.rpg.action-definition", "version": 1},
            "kind": "inline",
            "action": {
                "id": "action.heal",
                "name": "Heal",
                "sourcePath": "actions/heal.ts#heal",
                "targets": {"team": "ally", "maximumRange": 3, "maximumTargets": 1},
                "check": {"kind": "noRoll"},
                "rollScope": "none",
                "costs": [],
                "program": {"kind": "atomic", "body": {"kind": "onCheck", "noRoll": {
                    "kind": "operation",
                    "operation": {"kind": "heal", "amount": {"kind": "constant", "value": 4}}
                }}}
            }
        }),
        presentation: json!({"label": "Heal"}),
        references: Vec::new(),
        provenance: provenance.clone(),
        fingerprint: String::new(),
    };
    action.fingerprint = materialized_definition_fingerprint(&action).unwrap();
    let package = "consumer.package@1.0.0".to_owned();
    PreparedPlayBundle {
        schema: PlayBundleArtifactSchema {
            identity: PREPARED_PLAY_BUNDLE_IDENTITY.to_owned(),
            major: PLAY_BUNDLE_ARTIFACT_MAJOR,
        },
        play_bundle_identity: RpgVersionedIdentity {
            id: "consumer.package".to_owned(),
            version: "1.0.0".to_owned(),
        },
        ruleset: Ruleset {
            schema: RulesetSchema {
                identity: "asha.rpg.ruleset".to_owned(),
                major: 1,
            },
            identity: RpgVersionedIdentity {
                id: "consumer.rules".to_owned(),
                version: "1.0.0".to_owned(),
            },
            language: RpgVersionedIdentity {
                id: "asha-rpg".to_owned(),
                version: "1.0.0".to_owned(),
            },
            models: RulesetModels {
                checks: VersionedRpgRequirement {
                    id: "check.d20-roll-over".to_owned(),
                    version: 1,
                },
                turns: VersionedRpgRequirement {
                    id: "turn.ordered-one-action".to_owned(),
                    version: 1,
                },
                initiative: VersionedRpgRequirement {
                    id: "initiative.scenario-ordered".to_owned(),
                    version: 1,
                },
                reactions: VersionedRpgRequirement {
                    id: "reaction.before-damage-choice".to_owned(),
                    version: 1,
                },
                action_economy: VersionedRpgRequirement {
                    id: "action-economy.one-action-plus-reaction".to_owned(),
                    version: 1,
                },
            },
            provides: RulesetProvisions {
                operations: vec![VersionedRpgRequirement {
                    id: "operation.heal".to_owned(),
                    version: 1,
                }],
                capabilities: vec![VersionedRpgRequirement {
                    id: "capability.vitality".to_owned(),
                    version: 1,
                }],
                values: Vec::new(),
                numeric_domains: Vec::new(),
                calculation_selectors: Vec::new(),
                contribution_stacking_groups: Vec::new(),
            },
        },
        content_packs: vec![ResolvedContentPack {
            id: "consumer.package".to_owned(),
            version: "1.0.0".to_owned(),
            source_fingerprint: "fnv1a64:1111111111111111".to_owned(),
        }],
        dependency_lock: Vec::new(),
        content_requirements: ContentPackRequirements {
            operations: vec![VersionedRpgRequirement {
                id: "operation.heal".to_owned(),
                version: 1,
            }],
            capabilities: vec![VersionedRpgRequirement {
                id: "capability.vitality".to_owned(),
                version: 1,
            }],
            values: Vec::new(),
            numeric_domains: Vec::new(),
        },
        exported_roots: vec!["action.heal".to_owned()],
        materialized_definitions: vec![action],
        compiled_policy_bindings: Vec::new(),
        definition_provenance: vec![provenance],
        definition_commitments: Vec::new(),
        relationships: vec![ContentRelationshipProvenance {
            kind: ContentRelationshipKind::Exports,
            source: package,
            target: "action.heal".to_owned(),
            order: 0,
        }],
        derivation_provenance: Vec::new(),
        overlay_provenance: Vec::new(),
    }
}
