use rpg_compiler::CompiledRpgRuleset;
use rpg_core::{
    DeterministicRandomStream, RpgCapabilityState, RpgIntent, RpgReactionDecision,
    RpgReactionRequest, RpgResolutionReceipt, RpgResolutionRejection, RpgTraceStep,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgAuthorityCommand {
    pub expected_revision: u64,
    pub intent: RpgIntent,
    pub random_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgReactionCommand {
    pub expected_revision: u64,
    pub reaction_id: String,
    pub option_id: Option<String>,
    pub additional_random_values: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgPendingReaction {
    pub expected_revision: u64,
    pub request: RpgReactionRequest,
    pub trace: Vec<RpgTraceStep>,
    pub random_attempted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpgCommandOutcome {
    Accepted(RpgResolutionReceipt),
    AwaitingReaction(RpgPendingReaction),
    Rejected(RpgResolutionRejection),
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    expected_revision: u64,
    intent: RpgIntent,
    random_values: Vec<u32>,
    pending: RpgPendingReaction,
}

/// Owner of one compiled artifact's persistent capability state and staged
/// reaction transaction.
#[derive(Debug)]
pub struct RpgAuthoritySession {
    ruleset: CompiledRpgRuleset,
    state: RpgCapabilityState,
    pending: Option<PendingTransaction>,
    accepted_random_values: usize,
}

impl RpgAuthoritySession {
    pub fn new(ruleset: CompiledRpgRuleset, initial_state: RpgCapabilityState) -> Self {
        Self {
            ruleset,
            state: initial_state,
            pending: None,
            accepted_random_values: 0,
        }
    }

    pub fn ruleset(&self) -> &CompiledRpgRuleset {
        &self.ruleset
    }

    pub fn state(&self) -> &RpgCapabilityState {
        &self.state
    }

    pub fn pending_reaction(&self) -> Option<&RpgPendingReaction> {
        self.pending
            .as_ref()
            .map(|transaction| &transaction.pending)
    }

    pub fn accepted_random_values(&self) -> usize {
        self.accepted_random_values
    }

    pub fn submit(&mut self, command: RpgAuthorityCommand) -> RpgCommandOutcome {
        if self.pending.is_some() {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_PENDING",
                "$.command",
                "resolve the pending reaction before submitting another command",
            ));
        }
        if command.expected_revision != self.state.revision() {
            return RpgCommandOutcome::Rejected(revision_rejection(
                command.expected_revision,
                self.state.revision(),
            ));
        }

        let mut staged_state = self.state.clone();
        let mut random = DeterministicRandomStream::new(command.random_values.clone());
        match self
            .ruleset
            .resolve(&mut staged_state, &mut random, &command.intent)
        {
            Ok(receipt) => {
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                RpgCommandOutcome::Accepted(receipt)
            }
            Err(mut error) => {
                let Some(request) = error.reaction_request.take() else {
                    return RpgCommandOutcome::Rejected(error);
                };
                let pending = RpgPendingReaction {
                    expected_revision: command.expected_revision,
                    request: *request,
                    trace: error.trace,
                    random_attempted: error.random_attempted,
                };
                self.pending = Some(PendingTransaction {
                    expected_revision: command.expected_revision,
                    intent: command.intent,
                    random_values: command.random_values,
                    pending: pending.clone(),
                });
                RpgCommandOutcome::AwaitingReaction(pending)
            }
        }
    }

    pub fn react(&mut self, command: RpgReactionCommand) -> RpgCommandOutcome {
        let Some(transaction) = self.pending.clone() else {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_SESSION_REACTION_ABSENT",
                "$.reaction",
                "there is no pending reaction to resolve",
            ));
        };
        if command.expected_revision != transaction.expected_revision
            || command.expected_revision != self.state.revision()
        {
            return RpgCommandOutcome::Rejected(revision_rejection(
                command.expected_revision,
                self.state.revision(),
            ));
        }
        if command.reaction_id != transaction.pending.request.reaction_id {
            return RpgCommandOutcome::Rejected(rejection(
                "RPG_REACTION_ID_MISMATCH",
                "$.reaction.reactionId",
                format!(
                    "expected reaction {}",
                    transaction.pending.request.reaction_id
                ),
            ));
        }

        let mut evidence = transaction.random_values.clone();
        evidence.extend(command.additional_random_values);
        let mut staged_state = self.state.clone();
        let mut random = DeterministicRandomStream::new(evidence.clone());
        let decision = RpgReactionDecision {
            reaction_id: command.reaction_id,
            option_id: command.option_id,
        };
        match self.ruleset.resolve_with_reaction_decision(
            &mut staged_state,
            &mut random,
            &transaction.intent,
            &decision,
        ) {
            Ok(receipt) => {
                if random.remaining() != 0 {
                    return RpgCommandOutcome::Rejected(unused_random_rejection(
                        random.remaining(),
                    ));
                }
                self.pending = None;
                self.state = staged_state;
                self.accepted_random_values = self
                    .accepted_random_values
                    .saturating_add(receipt.random_consumed);
                RpgCommandOutcome::Accepted(receipt)
            }
            Err(error) => RpgCommandOutcome::Rejected(error),
        }
    }
}

fn revision_rejection(expected: u64, actual: u64) -> RpgResolutionRejection {
    rejection(
        "RPG_SESSION_REVISION_MISMATCH",
        "$.expectedRevision",
        format!("expected state revision {expected}, but active revision is {actual}"),
    )
}

fn unused_random_rejection(remaining: usize) -> RpgResolutionRejection {
    rejection(
        "RPG_RANDOM_EVIDENCE_UNUSED",
        "$.randomValues",
        format!("{remaining} supplied random value(s) were not consumed"),
    )
}

fn rejection(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> RpgResolutionRejection {
    RpgResolutionRejection {
        code: code.to_owned(),
        path: path.into(),
        message: message.into(),
        trace: Vec::new(),
        random_attempted: 0,
        random_request: None,
        reaction_request: None,
    }
}

#[cfg(test)]
mod tests {
    use rpg_compiler::compile_normalized_rpg_json;
    use rpg_core::{GridPosition, RpgDomainEvent, RpgEntityState, Team};

    use super::*;

    fn reaction_session() -> RpgAuthoritySession {
        let source = br#"{
          "schema":{"identity":"asha.rpg.ir","major":1},
          "package":{"id":"session.test","version":"1.0.0"},
          "catalogs":{"resources":["focus"],"capabilities":[
            "capability.random","capability.reactions","capability.resources","capability.vitality"
          ]},
          "requirements":[
            {"kind":"operation","id":"operation.damage","version":1},
            {"kind":"operation","id":"operation.openReaction","version":1},
            {"kind":"capability","id":"capability.random","version":1},
            {"kind":"capability","id":"capability.reactions","version":1},
            {"kind":"capability","id":"capability.resources","version":1},
            {"kind":"capability","id":"capability.vitality","version":1}
          ],
          "actions":[{
            "id":"action.reactive","name":"Reactive strike","sourcePath":"actions/reactive",
            "targets":{"team":"hostile","maximumRange":3,"maximumTargets":1},
            "check":{"kind":"noRoll"},"rollScope":"none",
            "costs":[{"resourceId":"focus","amount":1}],
            "program":{"kind":"atomic","body":{"kind":"sequence","steps":[
              {"kind":"operation","operation":{"kind":"openReaction","reactionId":"reaction.ward","options":[
                {"id":"ward","label":"Raise ward","damageReduction":3}
              ]}},
              {"kind":"operation","operation":{"kind":"damage","amount":{"kind":"dice","count":5,"sides":4,"bonus":0},"damageType":"force"}}
            ]}}
          }]
        }"#;
        let ruleset = compile_normalized_rpg_json(source).expect("reaction ruleset compiles");
        let actor = RpgEntityState::new("hero", Team::Ally, GridPosition { x: 0, y: 0 }, 20)
            .with_resource("focus", 2, 2);
        let target = RpgEntityState::new("guardian", Team::Enemy, GridPosition { x: 1, y: 0 }, 20);
        let mut state = RpgCapabilityState::default();
        state.insert_entity(actor);
        state.insert_entity(target);
        RpgAuthoritySession::new(ruleset, state)
    }

    fn command() -> RpgAuthorityCommand {
        RpgAuthorityCommand {
            expected_revision: 0,
            intent: RpgIntent {
                action_id: "action.reactive".to_owned(),
                actor_id: "hero".to_owned(),
                target_ids: vec!["guardian".to_owned()],
            },
            random_values: Vec::new(),
        }
    }

    #[test]
    fn reaction_resumes_the_same_atomic_state_and_random_transaction() {
        let mut session = reaction_session();
        let RpgCommandOutcome::AwaitingReaction(pending) = session.submit(command()) else {
            panic!("command must suspend");
        };
        assert_eq!(pending.request.reaction_id, "reaction.ward");
        assert_eq!(session.state().revision(), 0);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .resource("focus")
                .unwrap()
                .current,
            2
        );

        let invalid = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("missing".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        assert!(matches!(invalid, RpgCommandOutcome::Rejected(_)));
        assert_eq!(session.state().revision(), 0);

        let accepted = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        let RpgCommandOutcome::Accepted(receipt) = accepted else {
            panic!("valid reaction must resume and commit: {accepted:?}");
        };
        assert_eq!(receipt.random_consumed, 5);
        assert!(receipt
            .events
            .iter()
            .any(|event| matches!(event, RpgDomainEvent::DamageApplied { amount: 7, .. })));
        assert_eq!(session.state().revision(), 1);
        assert_eq!(
            session
                .state()
                .entity("hero")
                .unwrap()
                .resource("focus")
                .unwrap()
                .current,
            1
        );
        assert_eq!(
            session
                .state()
                .entity("guardian")
                .unwrap()
                .vitality()
                .current,
            13
        );
        assert!(session.pending_reaction().is_none());
    }

    #[test]
    fn rejected_reaction_evidence_does_not_accumulate_between_retries() {
        let mut session = reaction_session();
        let RpgCommandOutcome::AwaitingReaction(_) = session.submit(command()) else {
            panic!("command must suspend");
        };

        let insufficient = RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2],
        };
        let first = session.react(insufficient.clone());
        let second = session.react(insufficient);

        assert_eq!(first, second);
        let RpgCommandOutcome::Rejected(rejection) = first else {
            panic!("insufficient evidence must reject");
        };
        assert_eq!(rejection.code, "RPG_RANDOM_EXHAUSTED");
        assert_eq!(rejection.random_attempted, 0);
        assert!(session.pending.as_ref().unwrap().random_values.is_empty());
        assert_eq!(session.state().revision(), 0);

        let accepted = session.react(RpgReactionCommand {
            expected_revision: 0,
            reaction_id: "reaction.ward".to_owned(),
            option_id: Some("ward".to_owned()),
            additional_random_values: vec![2, 2, 2, 2, 2],
        });
        assert!(matches!(accepted, RpgCommandOutcome::Accepted(_)));
        assert_eq!(session.state().revision(), 1);
    }
}
