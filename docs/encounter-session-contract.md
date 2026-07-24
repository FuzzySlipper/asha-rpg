# Scenario and authority-session contract

## Purpose

`RpgScenario` is the versioned setup-only input for one authority session. A
consumer compiles or loads a `CompiledPlayBundle` and calls
`RpgAuthoritySession::from_scenario`. Rust validates the entire Scenario before
creating mutable authority state.

The schema is `asha.rpg.scenario@2`. `playBundleId` must exactly match the
compiled artifact. Checkpoint schema `asha.rpg.session.checkpoint@5` stores the
Scenario and its `fnv1a64.rpg-scenario.v1` fingerprint. Replay entry schema
version 6 binds before/after boundaries to that Scenario, source binding, turn,
revision, and state hash. Accepted event schema version 3 carries structured
roll contributions, and encounter-view schema version 5 exposes explicit
class/feature selection.

## Setup-only data

A Scenario contains only:

- board extent and typed cell capabilities;
- participants, teams, positions, selected class/features, and selected exported Content Pack definition ids;
- stable item instances plus exact item-instance-to-slot equipment bindings;
- initial vitality, named Ruleset stat/defense values, and Content Pack resource/modifier values;
- initiative order, current actor, round, and turn;
- random policy/source identity and versions.

Initial stat/defense ids must be provided by the selected Ruleset and their
values must be inside the named numeric domain. Resource/modifier ids must be
defined by the selected Content Packs. Each participant needs exactly one
vitality value and at least one selected action.

Items are immutable Content Pack data. Rust validates their portable attribute
schemas, catalog and Ruleset ownership, exported graph closure, allowed slots,
and every Scenario instance/equipment reference before state exists. Equipment
is authority state embedded in the Scenario, state hash, checkpoint, and
replay boundary; it is not a host-side presentation hint.

Participant profile schema `asha.rpg.participant-profile@2` and Scenario
participants bind an optional exported character class and a canonical selected
feature list. Every selected feature must be exported, compiled by Rust, and
listed by the selected class. Features cannot be selected without a class.
Class and feature selection is authority state covered by checkpoint restore,
state hashing, and replay; it is not a host-supplied modifier list.

Scenario is not an execution script. It cannot encode definitions, commands,
targets, reactions, roll values, expected events/outcomes, or Tester settings.
Strict decoding rejects every additional field.

## Authority and readbacks

Accepted actions, including artifact-authored selected-cell movement, and
explicit end-turn controls atomically update state, modifier tenure, accepted
events, and the next living initiative participant.
A pending reaction blocks other commands until resolved. Rejections preserve
state, log, turn, reaction, and accepted-random position.

Attack resolution reports its applied roll contributions as structured event
data. The action check modifier is first. Selected feature contributions follow
in canonical feature and contribution order when their Rust-evaluated spatial
conditions hold. Flanking requires a living same-team ally opposite the actor
across a cardinally adjacent living target. Surrounded counts living hostiles in
the four cardinally adjacent cells. Defeated or repositioned participants
therefore change later resolutions without changing the selected features.

`RpgAuthoritySession::encounter_view` exposes board/cells, participant state,
inventory/equipment, current actor and initiative, selected and legal actions
plus participant or cell options, available turn controls, pending reaction
options, accepted events, and encounter outcome. An item-bound action is
projected once for each compatible equipped item instance. Its view and
proposal carry the exact binding, and Rust rejects missing, unexpected,
tampered, or stale bindings without mutation.

A cell movement option is an authority path:
the destination cell id, the ordered traversed cell ids excluding the origin
and including the destination, and the total movement cost. Rust finds a
deterministic least-cost route over orthogonally adjacent authored cells.
Entering a cell consumes its traversal `movementCost`; a cell without a
traversal capability is passable at cost one. Impassable and occupied cells
cannot be entered or crossed. Equal-cost routes use a stable row-major path
ordering.

Commands still submit only a destination. Rust binds that id to the Scenario,
recomputes the path against current occupancy and traversal state, applies the
authored range and total-cost movement bound, and rejects a stale or
unreachable destination without mutation. The path is descriptive output;
only Rust mutates authority state.

## Random evidence

Interactive calls use a bound `RpgRandomSource`. Rust requests the exact draw,
validates its shape and range, and records consumed evidence. Hosts do not
inspect a random plan or select semantic branches. `RpgRollTapeSource` remains
a bounded portable source for consumers and focused tests; no seeded algorithm
is a portability claim.

Replay invokes ordinary submit/reaction/turn-control paths with recorded
evidence. It never rematerializes content, resolves versions, regenerates
randomness, or reapplies events.

## Non-claims

The initial board authority does not claim diagonal or hex topology,
jumping/flying/teleport movement, opportunity attacks, per-step effects,
area-target semantics, campaign persistence, scripted runners, AI control,
Tester configuration, class levels or prerequisites, non-attack feature
contributions, a general condition language, or Rulebench product protocols.
