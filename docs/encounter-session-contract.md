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
events, and encounter outcome. For a cell-target movement action, Rust binds
each submitted cell id to the Scenario position, applies the authored range
and movement bound, and exposes only destinations whose staged result remains
inside the board, passable, and unoccupied. It is descriptive output; only
Rust mutates authority state.

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

The initial board authority does not claim pathfinding, area-target semantics,
multi-cell movement-cost budgeting, campaign persistence, scripted runners,
AI control, Tester configuration, or Rulebench product protocols.
