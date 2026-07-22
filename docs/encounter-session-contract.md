# Scenario and authority-session contract

## Purpose

`RpgScenario` is the versioned setup-only input for one authority session. A
consumer compiles or loads a `CompiledPlayBundle` and calls
`RpgAuthoritySession::from_scenario`. Rust validates the entire Scenario before
creating mutable authority state.

The schema is `asha.rpg.scenario@1`. `playBundleId` must exactly match the
compiled artifact. Checkpoint schema `asha.rpg.session.checkpoint@3` stores the
Scenario and its `fnv1a64.rpg-scenario.v1` fingerprint. Replay entry schema
version 4 binds before/after boundaries to that Scenario, source binding, turn,
revision, and state hash.

## Setup-only data

A Scenario contains only:

- board extent and typed cell capabilities;
- participants, teams, positions, and selected exported Content Pack definition ids;
- initial vitality, named Ruleset stat/defense values, and Content Pack resource/modifier values;
- initiative order, current actor, round, and turn;
- random policy/source identity and versions.

Initial stat/defense ids must be provided by the selected Ruleset and their
values must be inside the named numeric domain. Resource/modifier ids must be
defined by the selected Content Packs. Each participant needs exactly one
vitality value and at least one selected action.

Scenario is not an execution script. It cannot encode definitions, commands,
targets, reactions, roll values, expected events/outcomes, or Tester settings.
Strict decoding rejects every additional field.

## Authority and readbacks

Accepted actions, including artifact-authored selected-cell movement, and
explicit end-turn controls atomically update state, modifier tenure, accepted
events, and the next living initiative participant.
A pending reaction blocks other commands until resolved. Rejections preserve
state, log, turn, reaction, and accepted-random position.

`RpgAuthoritySession::encounter_view` exposes board/cells, participant state,
current actor and initiative, selected and legal actions plus participant or
cell options, available turn controls, pending reaction options, accepted
events, and encounter outcome. A cell movement option is an authority path:
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
Tester configuration, or Rulebench product protocols.
