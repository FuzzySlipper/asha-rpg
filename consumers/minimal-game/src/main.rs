use std::io::{self, Read};

use asha_rpg::{
    compile_prepared_ruleset_json, decode_replay_entries, encode_replay_entries, GridPosition,
    RpgAuthorityCommand, RpgAuthoritySession, RpgCapabilityState, RpgCommandOutcome,
    RpgDomainEvent, RpgEntityState, RpgIntent, RpgReactionCommand, Team,
};

fn main() {
    let mut prepared_source = Vec::new();
    io::stdin()
        .read_to_end(&mut prepared_source)
        .expect("read prepared ruleset from stdin");
    let bundle =
        compile_prepared_ruleset_json(&prepared_source).expect("compile exact prepared artifact");
    let mut initial_state = RpgCapabilityState::default();
    initial_state.insert_entity(RpgEntityState::new(
        "hero",
        Team::Ally,
        GridPosition { x: 0, y: 0 },
        20,
    ));
    initial_state.insert_entity(RpgEntityState::new(
        "guardian",
        Team::Enemy,
        GridPosition { x: 1, y: 0 },
        20,
    ));
    let session = RpgAuthoritySession::from_compiled_ruleset(bundle, initial_state);
    let initial_checkpoint = session.checkpoint().expect("create checkpoint");
    let initial_json = session.checkpoint_json().expect("serialize checkpoint");
    let mut recording =
        RpgAuthoritySession::restore_checkpoint_json(&initial_json).expect("clean restore");

    let (pending_outcome, submit_entry) = recording
        .submit_recorded(RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "portable.reactive-strike".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
            },
            random_values: Vec::new(),
        })
        .expect("record suspended command");
    let RpgCommandOutcome::AwaitingReaction(pending) = pending_outcome else {
        panic!("consumer command should suspend: {pending_outcome:?}");
    };
    let pending_json = recording
        .checkpoint_json()
        .expect("serialize complete pending transaction");
    let mut pending_restore =
        RpgAuthoritySession::restore_checkpoint_json(&pending_json).expect("restore pending phase");

    let reaction = RpgReactionCommand {
        expected_revision: 0,
        reaction_id: pending.request.reaction_id,
        option_id: Some("ward".to_owned()),
        additional_random_values: vec![2, 2],
    };
    let (accepted, reaction_entry) = recording
        .react_recorded(reaction.clone())
        .expect("record resumed reaction");
    let restored_accepted = pending_restore.react(reaction);
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
