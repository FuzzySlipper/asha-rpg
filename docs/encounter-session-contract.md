# Scenario and authority-session contract

## Purpose

`RpgScenario` is the versioned setup-only input for one authority session. A
consumer compiles or loads a `CompiledPlayBundle` and calls
`RpgAuthoritySession::from_scenario`. Rust validates the entire Scenario before
creating mutable authority state.

The schema is `asha.rpg.scenario@2`. `playBundleId` must exactly match the
compiled artifact. Checkpoint schema `asha.rpg.session.checkpoint@10` stores the
Scenario and its `fnv1a64.rpg-scenario.v1` fingerprint. Replay entry schema
version 11 binds before/after boundaries to that Scenario, source binding,
turn, revision, and state hash. Accepted event schema version 9 carries the
contextual contribution ledger, activation transitions, and exact
authority-derived area selections plus structured typed-damage packet
resolution. Encounter-view schema version 11 exposes
that event history plus explicit class/feature selection, active named effects,
activation-budget readback, and session-bound area options.

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
It also cannot submit activation-budget values; Rust initializes them from the
compiled Ruleset. Strict decoding rejects every additional field.

## Authority and readbacks

Accepted actions, including artifact-authored selected-cell movement, and
explicit end-turn controls atomically update state, modifier tenure, accepted
events, and the next living initiative participant. Under the variable
activation-budget model, action and selected-reaction costs stage with their
consequences, several actions may occur in one turn, zero-cost activations
still count toward the ceiling, and only explicit end-turn advances
initiative. Owner-turn-start and round-start budgets reset only at their exact
boundary.
Named effects are created and removed only by typed artifact operations. Their
positive `1..=1000` duration uses exactly one global-turn, round, source-turn,
or target-turn anchor. A transition emits action/control lifecycle events,
round/turn events, canonical effect aging/expiry, modifier aging, and budget
resets in that order. Effects applied or refreshed in the same transaction
skip every boundary in that transaction. Checkpoint state retains definition,
instance, source, rank, stacking, anchor, remaining count, and application
revision exactly.
A pending reaction blocks other commands until resolved. Rejections preserve
state, log, turn, reaction, and accepted-random position.

Attack and generic scalar-test resolution report an authoritative scalar
contribution ledger as structured event data. It includes the base value, final
checked value, and every item, selected-feature, or active-effect candidate in canonical
source-definition and contribution-id order. Generic scalar events additionally
retain difficulty, total, margin, base band, matching natural rule, contextual
band-shift ledger, and final band before a separate event records the selected
branch. Candidates are `applied`, `inapplicable` with the exact failed
typed-fact reason, or `suppressed` with their stacking policy and retained
source identities. Rust evaluates actor/target state, named values, distance,
flanking/surrounding, exact item binding/tags, action tags, and current cell
support capabilities from one staged revision. Defeated, repositioned, or
differently equipped participants therefore change later resolutions without
changing content declarations.

`RpgAuthoritySession::encounter_view` exposes board/cells, participant state
and remaining activation budgets, the accepted activation count and ceiling,
inventory/equipment, current actor and initiative, selected and legal actions
plus participant, cell-path, or bounded area options, available turn controls,
pending reaction options, accepted events, and encounter outcome. An item-bound action is
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

An area option binds an opaque live session id, artifact id, Scenario
fingerprint, state revision, turn identity, action/item identity, and one
anchor. Rust projects either an anchor-origin Manhattan diamond or an
actor-origin orthogonal line over authored cells, then canonically derives the
eligible participants by cell and participant identity. Submission carries
only the live session id, revision, action/item identity, and anchor. Rust
rejects stale identity or revision before reading random evidence, and
recomputes the exact cells and participants once before executing per-target or
shared checks atomically. Events and replay retain the proposal revision and
exact derived set, including clipped or unauthored cells and team/living
filter reasons; the opaque live session id is deliberately not portable.

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
cones, arbitrary polygons, elevation, cover, line of effect, persistent auras,
campaign persistence, scripted runners, AI control, Tester configuration,
class levels or prerequisites, calculation owners beyond attack and
scalar-test profiles, a general condition language, or Rulebench product
protocols.
