# Scenario and authority-session contract

## Purpose

`RpgScenario` is the versioned setup-only input for one authority session. A
consumer compiles or loads a `CompiledPlayBundle` and calls
`RpgAuthoritySession::from_scenario`. Rust validates the entire Scenario before
creating mutable authority state.

The schema is `asha.rpg.scenario@3`. `playBundleId` must exactly match the
compiled artifact. Checkpoint schema `asha.rpg.session.checkpoint@14` stores the
Scenario and its `fnv1a64.rpg-scenario.v1` fingerprint. Replay entry schema
version 15 binds before/after boundaries to that Scenario, source binding,
turn, revision, and state hash. Accepted event schema version 13 carries the
contextual contribution ledger, activation transitions, and exact
authority-derived area selections plus structured typed-damage packet
resolution and spatial-source transitions. Encounter-view schema version 15 exposes
that event history plus explicit class/feature selection, active named effects,
movement allowance, activation-budget readback, and session-bound area and
forced-movement options plus active source cells, tenure, and source-scoped
trigger evidence.

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
A pending reaction, forced-movement destination choice, or turn save blocks
other commands until resolved. Rejections preserve the complete pending phase,
state, log, turn, and accepted-random position.

Fixed spatial sources are also created only by a typed artifact operation.
Rust derives their included cells from the authored board and retains exact
definition/version, owner, source, instance, origin, stacking, tenure, and
application revision. Enter/exit use accepted intermediary movement routes;
start/end use the ordinary turn transition. Trigger evidence is keyed by that
complete source identity, so independent sources may reuse an instance id
without sharing history. Source creation, deterministic trigger procedures,
aging, expiry, and their ordinary mutations remain one atomic authority
transaction.

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

A Ruleset may designate one owner-turn action budget as movement allowance.
Projection caps routes by its remaining value and accepted `moveToCell`
transitions spend the exact weighted route cost. Readback exposes the remaining
allowance explicitly; setup cannot supply it, and checkpoint restore rejects
missing, extra, negative, or unreachable budget state.

`push` and `slide` use the same authored cells, bounds, traversal, occupancy,
and canonical route facts. Push is cardinal and source-relative, moves as far
as possible up to its bound, and stops before the first unavailable cell.
Slide suspends the original transaction and exposes bounded destination/route
options tied to session, artifact, Scenario, revision, turn, actor, action,
source, target, and operation path. Forced movement never provokes.

The closed `voluntaryLeavesAdjacency` feature registration captures its owner,
feature source, trigger cells, reach, line of effect, response action, and
exact optional item binding from the trigger revision. The response owns an
exclusive exact-capacity owner-turn reaction budget. Accept executes it at the
trigger position; decline consumes no response evidence. Either decision
commits with the movement as one revision, while stale identity, invalid
evidence, or failed response legality leaves the pending transaction intact.

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

When a selector requires `line-of-effect.square-grid-supercover@1`, the action
options also bind the exact session, artifact, Scenario fingerprint, revision,
round, turn, actor, action, and item identity. Participant, destination-cell,
and area projection all call the same Rust supercover traversal over authored
cell centers. A typed `line-of-effect.obstruction@1` fact blocks intermediate
cells; start and target cells do not block themselves. At an exact corner the
traversal includes both adjacent cells in row-major order. Missing endpoint or
traversed cells fail closed. Filtered readback and accepted area events retain
generic reasons and blocker cell ids. Complete bound submissions are
revalidated before randomness and stale, blocked, or tampered choices leave
state, hash, revision, random position, log, positions, and vitality unchanged.

A `spatialSource` definition binds one fixed Manhattan diamond, an
all/allies/hostiles filter, source-aware stacking, fixed tenure, and a
canonical map of enter/start-turn/end-turn/exit procedures. Creation names the
owner and source through typed operation subjects; Rust derives the origin and
canonical authored-cell set. Route intermediaries and turn transitions produce
canonically ordered trigger candidates. The authority records applied,
inapplicable, and suppressed evidence and uses the application revision plus
per-transition keys to prevent same-transaction loops. Creation, trigger
mutation, aging, expiry, checkpoint, and replay are one Rust-owned state path.

## Random evidence

Interactive calls use a bound `RpgRandomSource`. Rust requests the exact draw,
validates its shape and range, and records consumed evidence. Hosts do not
inspect a random plan or select semantic branches. `RpgRollTapeSource` remains
a bounded portable source for consumers and focused tests; no seeded algorithm
is a portability claim.

Replay invokes ordinary submit/reaction/forced-movement/turn-control paths with recorded
evidence. It never rematerializes content, resolves versions, regenerates
randomness, or reapplies events.

## Non-claims

The initial board authority does not claim diagonal movement or hex topology,
jumping/flying/pull/teleport movement, general opportunity-attack or trigger
authoring,
cones, arbitrary polygons, elevation, cover, concealment, vision contests,
arbitrary-topology or client-computed sight, moving zones, source collisions,
summons, recursive spatial triggers, a general scheduler, campaign
persistence, scripted runners, AI control, Tester configuration, class levels
or prerequisites, calculation owners beyond attack and scalar-test profiles,
a general condition language, or Rulebench product protocols.
