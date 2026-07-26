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
| `Ruleset` (`asha.rpg.ruleset@1`) | language compatibility, Rust-bound operation and capability provisions, named stat/defense contracts, numeric domains, scalar calculation selectors, contribution stacking groups, scalar-test profiles, heterogeneous-pool profiles | actions, spells, classes, creatures, items, conditions, presentation, setup |
| `ContentPack` | authored definitions, presentation, dependencies, derivation, mixins, overlays | Rust execution callbacks, board/participants, commands or expected outcomes |
| `PlayBundle` (`asha.rpg.play-bundle.prepared@10` / `.compiled@10`) | one Ruleset plus an exact compatible Content Pack closure and fingerprints | ambient discovery, executable TypeScript, scenario scripts |
| `Scenario` (`asha.rpg.scenario@3`) | board, participants, selected definitions, initial values, initiative, random-source policy, and typed line-of-effect obstruction facts for one PlayBundle | definitions, commands, targets, reactions, rolls, expected events/outcomes, Tester configuration |

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

The model registry binds d20 roll-over checks, ordered turns,
scenario-supplied initiative order, before-damage reaction choices, and either
the legacy one-action-plus-reaction economy or `F2@1` variable activation
budgets. `F2@1` admits at most eight Ruleset-owned action/reaction budgets,
owner-turn-start or round-start resets, and an accepted-activation ceiling no
greater than 64. A consumer cannot introduce a new executable model by naming
it in TypeScript.

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

## Character classes and contextual contributions

Character classes and character features are closed, typed Content Pack
definitions. A class lists exported feature definitions; a participant profile
and Scenario select one class and a canonical subset of its features. Rust
validates those graph edges and selections, stores the selection in participant
authority state, and binds it into the Scenario fingerprint, checkpoint,
portable state hash, and replay boundary. A command cannot submit or replace
feature semantics.

Rulesets own versioned scalar calculation selectors, their numeric domains, and
versioned stacking groups. Item and character-feature schema version 4 may
declare `asha.rpg.scalar-contribution@1` records. Each retains a stable
contribution id, source definition id and label, owned selector and stacking
group references, a signed bounded value expression, and a bounded typed
predicate. The initial group policies are `sum`, `greatest`, `least`, and
`signed-extremes`. Equal values are resolved by source-definition id and then
contribution id, never by Content Pack export order.

The closed predicate vocabulary reads actor/target identity and team, living
state, named stats or defenses, cardinal distance, current flanking and
surrounding, exact bound item definition and tags, selected action tags, and
support-definition capabilities on the actor or target cell. Boolean
composition is limited to `not`, `all`, and `any`; it is not an option bag or a
generic predicate VM. Flanking and surrounding retain their existing
living-participant cardinal-grid definitions.

Rulesets may additionally bind
`line-of-effect.square-grid-supercover@1`. An authored selector explicitly
chooses `required` or `ignored`; required participant, destination-cell, and
bounded-area choices are projected by one Rust traversal over square-grid cell
centers. Typed `line-of-effect.obstruction@1` Scenario facts are the only
blockers. Exact corner crossings include both adjacent cells in row-major
order, endpoint cells do not block their own target, missing traversed cells
fail closed, and readback reports the canonical blocker identities. The
TypeScript builders declare policy and facts but perform no ray casting.

Rust gathers feature candidates plus contributions from the exact bound item
after target and item validation. It evaluates every candidate against one
staged revision, reduces groups canonically, checks arithmetic and the
selector's final numeric domain, and supplies the final value to an attack or
generic scalar-test calculation. `AttackResolved.contributionLedger`,
`ScalarTestResolved.contributionLedger`, and trace retain the base and final
values plus every applied, inapplicable, and suppressed candidate. An
inapplicable entry carries the failed typed-fact reason; a suppressed entry
names the policy and canonical retained sources. The ledger owns no mutable
state and consumes no randomness.

The item, feature, and named-effect schemas may separately declare
`asha.rpg.pool-contribution@2` records for a Ruleset-owned heterogeneous-pool
profile. Their closed effects add a die count, add an automatic result-axis
value, or replace one die at a time with an explicit fallback. They reuse the
typed predicate and stacking contracts, but never pass through the scalar
selector ledger.

Scalar, outcome-band, and heterogeneous-pool contributions use an explicit
`actor` or `target` subject. Items and selected features are actor-owned;
active named effects may contribute through either lane. Rust gathers the
actor lane from effects on the acting participant and the target lane from
effects on the participant currently being resolved. The subject is part of
the canonical contribution identity and retained ledger; TypeScript does not
infer it from presentation or execute it.

One source may declare at most 32 contributions of each typed family, one
evaluation may gather at
most 256, predicates allow depth 16 and 128 nodes, and value expressions use
the existing formula depth/node bounds. Unknown owners, selectors, groups,
facts, tags, versions, duplicates, overflow, and out-of-domain values fail
closed. Classes and features remain sealed. Levels, prerequisites, runtime
feature choice, diagonal flanking, arbitrary auras, arbitrary pool/category
scripts, and selectors for damage, healing, or defenses remain separate work.

## Rust semantic profile

The initial closed operation vocabulary supports damage, healing, resource
change, fixed-delta and weighted selected-cell grid movement, cardinal push,
bounded slide choice, turn-bounded modifiers, and typed before-damage and
voluntary-leave-adjacency reaction windows.
Damage is one bounded packet of canonically ordered typed parts. Selected
features and active named effects contribute exact-version immunity,
signed-flat, and rational-scale responses. Rust filters and orders candidates
by phase and runtime identity, applies one flat clamp and per-scale floor,
mutates vitality once, and retains every original/final part and response
decision in the accepted event.
Checks support attack, saving throw, generic Ruleset-owned scalar tests,
Ruleset-owned heterogeneous pools, and no-roll flows. Scalar profiles own one
d2..d100 primary die, a checked numeric
domain, complete margin classification, ordered bands, disjoint natural rules,
and an optional scalar contribution selector. Item/feature schema version 4 may
also declare typed contextual band shifts. Rust resolves margin, natural rule,
then each canonical contextual shift with per-step end clamping and emits the
complete ledger before selecting one known band branch or its required default.
The scalar base and explicit difficulty use the non-random subset of the
formula AST (constants, named stat reads, addition, and halving), so the
profile's primary die is the check's only random evidence.

Heterogeneous-pool profiles own canonical d2..d100 die types with complete
face-to-vector tables, result axes, disjoint cancellation pairs, and ordered
vector-outcome rules with a required default band. Actions provide canonical
base die counts and automatic axes. Rust gathers applicable item,
selected-feature, and active-effect pool contributions, performs grouped additions and sequential
replacement-or-fallback units, then freezes the exact die-type/count/sides
request before reading evidence. Accepted evidence retains every die type,
ordinal, side count, and face value. Events and trace retain the source ledger,
replacement decisions, frozen pool, raw and automatic axes, cancellation, net
axes, and selected outcome branch. TypeScript only authors and validates the
retained declarations.
Programs support one atomic root containing bounded sequence, predicate branch,
repeat, per-target, check-outcome, and scalar-outcome branches. Unknown
operations, capabilities, references, or versions fail closed.

Every rejected command is atomic. A reaction or slide choice suspends the same
transaction and revision. Random requests preserve their exact count/sides and target order.
Under the variable model actions and reactions pay staged budgets and remain
on the current turn until explicit end-turn. Zero-cost activations still count
toward the Ruleset ceiling. Turn transitions emit round/turn events, age
matching named effects in canonical anchor/source order, age modifiers, reset
only matching budgets, reset the accepted count, and emit the complete
lifecycle evidence. Runtime internals,
compiled programs, and capability-store layout are not serialized contracts.

Named effect definitions are sealed `asha.rpg.effect@2` content with a bounded
rank, one portable stacking identity/policy, and one typed tenure. Fixed tenure
uses a positive `1..=1000` global-turn, round, source-turn, or target-turn
count. Save-ends tenure requests one d20 at the target's turn end and succeeds
on 10 or higher. An effect may also carry at most 32 canonical typed condition
clauses that forbid an action tag, require an action tag, or forbid movement.
Rust applies those restrictions both while projecting choices and during
submission preflight, retaining the exact unavailable effect source. At most
32 typed registered contributions are accepted. `applyEffect` and
`removeEffect` are the only authored mutation operations. Rust retains exact
target/source/definition/instance identity, rank, tenure, remaining count,
condition clauses, policy, and application revision.
`independentBySource`, `replace`, and `refresh` are deterministic; an instance
applied or refreshed in the transaction that advances a matching boundary
does not age until a later accepted transaction.

Explicit end-turn may enter an awaiting-turn-save phase with canonically
ordered effect candidates. Each candidate owns one exact
`effectSave` d20 request. Resolution, effect expiry or retention, budget reset,
and turn transition are one staged transaction. Wrong evidence cardinality or
range preserves the pending phase, state, log, revision, accepted random
position, and state hash. Fixed-action turn models collect the same evidence
before their automatic transition, so they cannot bypass save-ends tenure.

## Scenario and persistence

Scenario decoding denies unknown fields. Loading validates its PlayBundle id,
selected exported definitions, participant actions, named stat/defense ids and
numeric domains, content-owned resource/modifier ids, board, occupancy,
initiative, capability owners, and random-source binding before mutable state
exists.

Checkpoint schema version 13 embeds the exact compiled PlayBundle, Scenario and
Scenario fingerprint, portable state, turn/log, accepted random position,
pending reaction, forced-movement choice, or turn-save phase, named effect
instances, and canonical state hash. Replay entry schema version 14 records
ordinary submit/reaction/forced-movement/turn-control operations and verifies
before/after boundaries. Accepted event schema version 12 and encounter-view
schema version 14
carry the contextual contribution ledger, character selection, activation
budgets, accepted activation count, exact authority-derived area selections,
weighted movement routes, forced-choice and movement-reaction identity,
structured typed-damage packet evidence, active-effect tenure/conditions, and
pending save requests. Prepared and compiled PlayBundle artifact schema major
12 covers these retained semantic contracts. Replay never reruns
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

## Planned first-wave breadth

The survey-selected neutral expansion is specified in
[`first-wave-primitive-catalog.md`](first-wave-primitive-catalog.md). That
catalog is an implementation map. `F0@1`, the contextual contribution ledger,
`F1@1`, generic scalar tests and ordered outcomes, `F2@1`, variable activation
budgets, `F3@1`, named effects with bounded expiry, `F4@1`, typed damage
packets and qualified responses, and `F6@1`, heterogeneous pools and vector
outcomes, are implemented as described above. `F5@1`, bounded area selection
and spatial legality, is also implemented through the session-bound authority
path. The narrower square-grid line-of-effect extension uses the same
session-bound option, submission, event, checkpoint, and replay authority.
