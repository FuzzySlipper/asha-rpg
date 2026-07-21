use std::io::{self, Read};

use asha_rpg::{
    compile_prepared_play_bundle_json, decode_replay_entries, encode_replay_entries, BoundedValue,
    GridPosition, RpgActionProposal, RpgAuthoritySession, RpgBoardSetup, RpgCommandOutcome,
    RpgDomainEvent, RpgScenario, RpgInitialCapability, RpgParticipantSetup,
    RpgRandomRequest, RpgRandomSource, RpgRandomSourceBinding, RpgRandomSourceFailure,
    RpgReactionProposal, RpgTeamId, RpgTurnInitialization,
};

fn main() {
    let mut prepared_source = Vec::new();
    io::stdin()
        .read_to_end(&mut prepared_source)
        .expect("read prepared PlayBundle from stdin");
    let bundle =
        compile_prepared_play_bundle_json(&prepared_source).expect("compile exact prepared artifact");
    let scenario = RpgScenario {
        schema: RpgScenario::schema(),
        play_bundle_id: bundle.artifact().artifact_id.clone(),
        board: RpgBoardSetup {
            width: 4,
            height: 4,
            cells: Vec::new(),
        },
        participants: vec![
            participant("hero", "Hero", RpgTeamId::ally(), 0),
            participant("guardian", "Guardian", RpgTeamId::enemy(), 1),
        ],
        turn: RpgTurnInitialization {
            initiative_order: vec!["hero".to_owned(), "guardian".to_owned()],
            current_actor_id: "hero".to_owned(),
            round: 1,
            turn: 1,
        },
        random_source: RpgRandomSourceBinding {
            policy_id: "minimal-game.recorded-evidence".to_owned(),
            policy_version: 1,
            source_id: "minimal-game.roll-tape".to_owned(),
            source_version: 1,
        },
    };
    let session = RpgAuthoritySession::from_scenario(bundle, scenario).expect("validate scenario");
    let initial_checkpoint = session.checkpoint().expect("create checkpoint");
    let initial_json = session.checkpoint_json().expect("serialize checkpoint");
    let mut recording =
        RpgAuthoritySession::restore_checkpoint_json(&initial_json).expect("clean restore");
    let mut source = ConstantTwoSource {
        binding: recording.scenario().random_source.clone(),
    };

    let (pending_outcome, submit_entry) = recording
        .submit_with_random_source_recorded(
            RpgActionProposal {
            expected_revision: 0,
            action_id: "portable.reactive-strike".to_owned(),
            actor_id: "hero".to_owned(),
            target_ids: vec!["guardian".to_owned()],
            },
            &mut source,
        )
        .expect("record suspended command");
    let RpgCommandOutcome::AwaitingReaction(pending) = pending_outcome else {
        panic!("consumer command should suspend: {pending_outcome:?}");
    };
    let pending_json = recording
        .checkpoint_json()
        .expect("serialize complete pending transaction");
    let mut pending_restore =
        RpgAuthoritySession::restore_checkpoint_json(&pending_json).expect("restore pending phase");

    let reaction = RpgReactionProposal {
        expected_revision: 0,
        reaction_id: pending.request.reaction_id,
        option_id: Some("ward".to_owned()),
    };
    let (accepted, reaction_entry) = recording
        .react_with_random_source_recorded(reaction.clone(), &mut source)
        .expect("record resumed reaction");
    let mut restored_source = ConstantTwoSource {
        binding: pending_restore.scenario().random_source.clone(),
    };
    let (restored_accepted, _) = pending_restore
        .react_with_random_source_recorded(reaction, &mut restored_source)
        .expect("restored reaction resolves through the same source contract");
    assert_eq!(accepted, restored_accepted);

    let entries_json =
        encode_replay_entries(&[submit_entry, reaction_entry]).expect("serialize replay entries");
    let entries = decode_replay_entries(&entries_json).expect("decode replay entries");
    let replayed =
        RpgAuthoritySession::replay(initial_checkpoint, &entries).expect("deterministic replay");
    assert_eq!(replayed.state(), recording.state());
    assert_eq!(
        replayed.state_hash().expect("replay hash"),
        recording.state_hash().expect("recording hash")
    );
    let RpgCommandOutcome::Accepted(receipt) = accepted else {
        panic!("reaction should commit: {accepted:?}");
    };
    assert_eq!(receipt.random_consumed, 2);
    assert!(receipt.events.iter().any(|event| matches!(
        event,
        RpgDomainEvent::DamageApplied { amount: 1, .. }
    )));
    assert_eq!(
        replayed
            .state()
            .entity("guardian")
            .expect("guardian state")
            .vitality()
            .current,
        19
    );
    println!(
        "minimal consumer checkpointed, restored pending reaction, and replayed {} records at hash {}",
        entries.len(),
        replayed.state_hash().expect("final hash").value
    );
}

struct ConstantTwoSource {
    binding: RpgRandomSourceBinding,
}

impl RpgRandomSource for ConstantTwoSource {
    fn binding(&self) -> &RpgRandomSourceBinding {
        &self.binding
    }

    fn draw(
        &mut self,
        request: &RpgRandomRequest,
    ) -> Result<Vec<u32>, RpgRandomSourceFailure> {
        Ok(vec![2_u32.min(request.sides); request.count as usize])
    }
}

fn participant(id: &str, label: &str, team_id: RpgTeamId, x: u32) -> RpgParticipantSetup {
    RpgParticipantSetup {
        id: id.to_owned(),
        label: label.to_owned(),
        team_id,
        position: GridPosition { x, y: 0 },
        definition_ids: vec!["portable.reactive-strike".to_owned()],
        capabilities: vec![RpgInitialCapability::Vitality {
            value: BoundedValue {
                current: 20,
                max: 20,
            },
        }],
    }
}
