# Asha RPG design

## Purpose

Asha RPG is a portable RPG authority substrate. Rust owns semantic validation,
operation bindings, deterministic resolution, mutation, events, trace,
checkpoint, replay, and typed views. TypeScript owns an immutable authoring AST
that produces data for Rust. TypeScript never executes gameplay semantics.

## Four public contracts

The public model deliberately separates four things that used to be called a
ruleset:

| Contract | Owns | Must not contain |
| --- | --- | --- |
| `Ruleset` (`asha.rpg.ruleset@1`) | language compatibility, Rust-bound operation and capability provisions, named stat/defense contracts, numeric domains | actions, spells, classes, creatures, items, conditions, presentation, setup |
| `ContentPack` | authored definitions, presentation, dependencies, derivation, mixins, overlays | Rust execution callbacks, board/participants, commands or expected outcomes |
| `PlayBundle` (`asha.rpg.play-bundle.prepared@2` / `.compiled@2`) | one Ruleset plus an exact compatible Content Pack closure and fingerprints | ambient discovery, executable TypeScript, scenario scripts |
| `Scenario` (`asha.rpg.scenario@2`) | board, participants, selected definitions, initial values, initiative, and random-source policy for one PlayBundle | definitions, commands, targets, reactions, rolls, expected events/outcomes, Tester configuration |

A Tester is a caller of the same accessible interaction surface as a person;
it is not a field in any of these contracts.

## Compilation and authority flow

```text
Ruleset + Content Pack sources
  -> TypeScript resolves dependencies and materializes a prepared PlayBundle
  -> Rust independently validates provisions, requirements, closure, and fingerprints
  -> Rust emits a compiled PlayBundle and private CompiledRpgRules
  -> Scenario validation creates one persistent authority session
  -> typed proposals + random source -> atomic events/state/turn readbacks
```

Content Pack requirements must be a direct subset of the selected Ruleset's
provided operation versions, capability versions, named values, and numeric
domains. Rust also verifies every Ruleset operation/capability provision has a
registered authority binding. There is no compatibility matrix or registry.

The initial model registry binds d20 roll-over checks, ordered one-action turns,
scenario-supplied initiative order, before-damage reaction choices, and the
one-action-plus-reaction economy. A consumer cannot introduce a new executable
model by naming it in TypeScript.

Content dependencies and definition ownership use exact existing package
resolution. Artifact identity and source/semantic/presentation fingerprints
cover the Ruleset contract as well as the materialized content, so changing
either changes the authority input.

## Named values

Rulesets expose named stat and defense contracts. TypeScript callers use
`rulesetStat` and `rulesetDefense`, preserving the Ruleset owner in a nominal
reference while normalized IR carries the stable id. Each named contract
selects a declared numeric domain. Rust stores and evaluates generic ids and
numbers; it does not enumerate game-specific names.

Content presentation may display an alias such as Might for a Strength stat,
but it does not change the Ruleset identity. Content-defined resources,
modifiers, and damage types remain owned by Content Packs.

### Named-value derivation versus action formulas

`RulesetValueExpression` and `RpgIrFormula` are deliberately separate bounded
contracts because they run at different authority phases and have different
inputs:

- A `RulesetValueExpression` is a deterministic compile-time dependency graph
  over named Ruleset values. Rust validates its nominal Ruleset/value
  references, node/depth bounds, checked integer arithmetic, mathematical floor
  division, output numeric domain, and acyclic topological order before mutable
  Session state exists. Scenario setup supplies only input facts; Rust
  materializes derived values and revalidates them when restoring checkpoints
  or replay state. The expression has no actor, target, action phase, or random
  source.
- An `RpgIrFormula` is a Session-time action expression evaluated inside a
  command or reaction. It may read an actor or target stat and consume declared
  dice evidence. Its `Add` and `Half` nodes serve action/check evaluation; it is
  not a graph that declares persistent named outputs or dependencies between
  Ruleset contracts.

Reusing `RpgIrFormula` for named values would admit subject bindings and
randomness into setup materialization, while still lacking nominal cross-value
references, dependency ordering, and the declared floor-division semantics.
Conversely, extending `RulesetValueExpression` with action subjects or dice
would turn setup derivation into a second Session expression engine. The two
contracts may share implementation primitives without sharing a public AST.

The reusable upstream candidates are the noun-free pieces: bounded expression
tree validation, checked integer subtraction, mathematical floor division,
named-key dependency collection, cycle detection, and deterministic
topological planning. Promotion requires a second non-RPG consumer and a
governed public contract. The current schema identity, `RulesetValueKind`,
nominal Ruleset ownership, numeric-domain lookup, Scenario materialization, and
checkpoint/replay enforcement remain RPG-owned and are not upstream claims.

Support definitions may use consumer-owned catalog names and inert `data` for
conditions or other product presentation. Rust keeps that data inside the
artifact identity and definition graph but interprets only registered semantic
schemas. This is how an independent rules repository can describe setup
ergonomics without creating a second rules engine or a d20-specific Rust enum.

## Character classes and roll contributions

Character classes and character features are closed, typed Content Pack
definitions. A class lists exported feature definitions; a participant profile
and Scenario select one class and a canonical subset of its features. Rust
validates those graph edges and selections, stores the selection in participant
authority state, and binds it into the Scenario fingerprint, checkpoint,
portable state hash, and replay boundary. A command cannot submit or replace
feature semantics.

The initial feature selector contributes to attack checks. A contribution has a
stable id, source definition id and label, signed amount, and one bounded
condition tree. Conditions currently support always, actor flanks target, actor
surrounded by a minimum number of hostiles, and conjunction. Flanking means the
living actor and a living ally on the same team are cardinally adjacent on
opposite sides of the living target. Surrounded counts living hostile
participants in the four cardinally adjacent cells. These are exact square-grid
authority rules, not presentation heuristics.

Rust evaluates conditions from staged participant state at resolution time.
The action check modifier is recorded first; applicable feature contributions
then follow canonical feature-definition and contribution-id order. Every
applied source is retained in `AttackResolved.contributions`, and checked
addition produces the resolved modifier and total. A feature may declare at
most one contribution for a selector, and duplicate feature selection is
rejected rather than stacked accidentally.

Classes and features are sealed in this contract version. It does not yet claim
levels, prerequisites, feature choices during a session, diagonal flanking,
range-shaped auras, a general-purpose condition VM, or contribution selectors
for saves, damage, healing, or defenses.

## Rust semantic profile

The initial closed operation vocabulary supports damage, healing, resource
change, fixed-delta and selected-cell grid movement, turn-bounded modifiers,
and a typed reaction window.
Checks support attack, saving throw, and no-roll flows. Programs support one
atomic root containing bounded sequence, predicate branch, repeat, per-target,
and check-outcome branches. Unknown operations, capabilities, references, or
versions fail closed.

Every rejected command is atomic. A reaction suspends the same transaction and
revision. Random requests preserve their exact count/sides and target order.
Accepted turn transitions age modifiers and emit events. Runtime internals,
compiled programs, and capability-store layout are not serialized contracts.

## Scenario and persistence

Scenario decoding denies unknown fields. Loading validates its PlayBundle id,
selected exported definitions, participant actions, named stat/defense ids and
numeric domains, content-owned resource/modifier ids, board, occupancy,
initiative, capability owners, and random-source binding before mutable state
exists.

Checkpoint schema version 5 embeds the exact compiled PlayBundle, Scenario and
Scenario fingerprint, portable state, turn/log, accepted random position,
pending phase, and canonical state hash. Replay entry schema version 6 records
ordinary submit/reaction/turn-control operations and verifies before/after
boundaries. Accepted event schema version 3 and encounter-view schema version 5
carry the contribution and character-selection additions. Replay never reruns
authoring or substitutes a candidate artifact.

## TypeScript authoring

`@asha-rpg/authoring` exports separate builders for Rulesets, Content Packs,
PlayBundles, and Scenarios. Action AST traversal derives semantic requirements
and content graph edges. Package selection is explicit; callers pass immutable
sources and no global registry or filesystem scan is used.

Action reuse is represented by exported `actionProcedure` definitions. A
procedure declares an owner package, a closed typed parameter schema, and
either an abstract normalized action body or an invocation of another
procedure. An action definition is exactly one inline action or one
owner-bound procedure invocation. Bounded integers, identifiers, booleans,
formulas, Ruleset-value references, Content Pack catalog references, targeting,
checks, costs, programs, and check-outcome branches are portable argument
types. Parameter references are inert JSON nodes; they are never TypeScript
callbacks.

The prepared and compiled artifacts retain procedure definitions and
invocations as the authoritative structure. They do not carry a parallel
submitted expansion. Rust independently checks owners, exact arguments,
bounds, reference closure, composition cycles, and template shape, then
expands to private `RpgIrAction` plans. Procedure source and semantics therefore
participate in definition fingerprints and the PlayBundle artifact id, which
also binds checkpoints and replay. Mixins and patches remain deterministic
content-derivation tools, not the primary action-reuse mechanism.

Derivation, ordered mixins, local patches, overlays, and configuration are
materialized deterministically. The artifact contains final definitions and
typed provenance, not runtime inheritance. Low-level graph and patch builders
exist only for focused compiler fixtures.

## Dependency direction and content ownership

Asha RPG depends only on the public ASHA revision recorded in governance. It
never imports Rulebench, product protocols, hosts, filesystem storage, or
cross-product proof code. Downstream games and Rulebench consume the public
facade and SDK.

Asha RPG owns semantic vocabulary and authority behavior. Independent content
repositories own concrete Rulesets and Content Packs. A new content noun is a
TypeScript/content change; a new meaning that changes legality, evaluation,
timing, mutation, events, randomness, or replay starts in Rust.

## Versioning

Ruleset, Content Pack, PlayBundle, Scenario, IR, operation, capability,
checkpoint, replay, Rust facade, and TypeScript package versions evolve
independently. Unknown required data fails closed. Obsolete pre-split
`ruleset package`, composition, artifact, and encounter-setup names are removed
rather than retained as aliases.
