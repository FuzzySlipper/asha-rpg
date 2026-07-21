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
| `PlayBundle` (`asha.rpg.play-bundle.prepared@1` / `.compiled@1`) | one Ruleset plus an exact compatible Content Pack closure and fingerprints | ambient discovery, executable TypeScript, scenario scripts |
| `Scenario` (`asha.rpg.scenario@1`) | board, participants, selected definitions, initial values, initiative, and random-source policy for one PlayBundle | definitions, commands, targets, reactions, rolls, expected events/outcomes, Tester configuration |

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

## Rust semantic profile

The initial closed operation vocabulary supports damage, healing, resource
change, grid movement, turn-bounded modifiers, and a typed reaction window.
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

Checkpoint schema version 3 embeds the exact compiled PlayBundle, Scenario and
Scenario fingerprint, portable state, turn/log, accepted random position,
pending phase, and canonical state hash. Replay entry schema version 4 records
ordinary submit/reaction/turn-control operations and verifies before/after
boundaries. Replay never reruns authoring or substitutes a candidate artifact.

## TypeScript authoring

`@asha-rpg/authoring` exports separate builders for Rulesets, Content Packs,
PlayBundles, and Scenarios. Action AST traversal derives semantic requirements
and content graph edges. Package selection is explicit; callers pass immutable
sources and no global registry or filesystem scan is used.

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
