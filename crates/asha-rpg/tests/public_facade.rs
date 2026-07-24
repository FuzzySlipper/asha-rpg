use asha_rpg::{
    compile_prepared_play_bundle, materialized_definition_fingerprint, BoundedValue,
    ContentDefinitionProvenance, ContentExtensionPolicy, ContentPackRequirements,
    ContentRelationshipKind, ContentRelationshipProvenance, ContentSourceLocation,
    ContentValueRequirement, GridPosition, MaterializedContentDefinition,
    MaterializedContentDefinitionKind, MaterializedContentVisibility, PlayBundleArtifactSchema,
    PreparedPlayBundle, ResolvedContentPack, RpgActionProposal, RpgAuthoritySession, RpgBoardSetup,
    RpgCommandOutcome, RpgInitialCapability, RpgParticipantSetup, RpgRandomSourceBinding,
    RpgRollTapeSource, RpgScenario, RpgTeamId, RpgTurnControl, RpgTurnControlProposal,
    RpgTurnInitialization, RpgVersionedIdentity, Ruleset, RulesetModels, RulesetNumericDomain,
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
                    "version": 1
                },
                "role": "player",
                "definitionIds": ["action.heal"],
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
            "schema": {"identity": "asha.rpg.item", "version": 1},
            "tags": ["healing"],
            "traits": ["usable"],
            "allowedSlots": ["hand.main", "hand.off"],
            "attributes": [{
                "type": "boundedInteger",
                "id": "healing",
                "value": 7,
                "minimum": 0,
                "maximum": 20
            }]
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
            "schema": {"identity": "asha.rpg.item", "version": 1},
            "tags": ["healing"],
            "traits": ["usable"],
            "allowedSlots": ["hand.main", "hand.off"],
            "attributes": [{
                "type": "boundedInteger",
                "id": "healing",
                "value": 4,
                "minimum": 0,
                "maximum": 20
            }]
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
