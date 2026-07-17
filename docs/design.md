# Asha RPG design

## Purpose

Asha RPG is a portable, downstream-game-compatible RPG substrate over public
ASHA RuntimeSession and gameplay-module surfaces. It supplies Rust-owned RPG
semantics and a TypeScript authoring frontend that compiles data into normalized
RPG IR. It is not a game, workbench, host, archive browser, or proof harness.

## Four representations

1. TypeScript authoring AST and immutable builders optimize for composition.
2. Versioned normalized RPG IR optimizes for canonical interchange.
3. Private Rust compiled rulesets optimize for authority execution.
4. Runtime capability state, workspaces, accepted DomainEvents, trace, replay
   inputs, and typed views implement the ECRP loop.

These are one-way compilation stages, not generated mirrors. TypeScript helper
functions disappear during normalization. Rust performs final compatibility,
reference, and semantic validation.

## Authority loop

```text
typed intent -> Rules -> staged resolution workspace -> accepted DomainEvents
             -> capability mutation owners -> typed views
```

A rejected resolution commits no capability mutation. Cross-capability actions
use composed-owner transactions with declared reads and expected revisions.
Random requests have stable keys and canonical target order. Trace explains
authority decisions but is not mutation input.

## Public surfaces

The permanent public surface includes:

- Rust facade `asha-rpg`, re-exporting only supported portable contracts;
- normalized IR decode and compatibility contracts;
- a Rust compiler and semantic kernel over closed operation registrations;
- typed intents, accepted DomainEvents, replayable authority records, and views;
- TypeScript packages `@asha-rpg/ir` and `@asha-rpg/authoring`.

Private compiled structures, capability-store layout, ASHA routing envelopes,
and optimization indexes are not serialization contracts.

## Initial Rust semantic profile

The active compatibility profile is:

| Surface | Supported version or vocabulary |
| --- | --- |
| normalized IR | `asha.rpg.ir` major 1 |
| operations | `operation.damage@1`, `operation.heal@1`, `operation.changeResource@1`, `operation.applyModifier@1` |
| capabilities | vitality, stats, defenses, resources, modifiers, deterministic random, each at version 1 |
| checks | attack, saving throw, no roll |
| formulas | constant, typed stat read, add, bounded dice, half |
| predicates | always, comparison, not, all, any |
| composition | atomic root, sequence, when, bounded repeat, bounded per-target, check branch |
| modifier tenure | positive turn count with replace or refresh stacking |

Strict decode rejects unknown semantic fields. Compatibility requires exact
operation and capability versions. Compilation resolves catalog references,
checks declared reads and owners, enforces expression/program/expanded-program
bounds, and binds every operation to a static Rust registration. The compiled
program and capability plan are private; consumers receive identity,
requirement, intent, event, trace, rejection, receipt, state-view, and session
surfaces only.

Resolution clones capability state and the deterministic random stream into a
workspace. Costs, checks, branches, and owner mutations are staged there. A
successful action advances both authoritative surfaces and emits accepted
DomainEvents plus explanatory trace. A rejection returns stable code/path
evidence while leaving both authoritative surfaces unchanged.

## Dependency direction

Asha RPG may depend only on the exact public ASHA packages recorded in
`governance/upstream-asha.toml` plus ordinary language/runtime dependencies
approved in the ownership map. It never reaches a sibling checkout or imports
private ASHA crates. It never depends on Rulebench, Angular, a process host,
filesystem storage, fixtures, goldens, experiments, or certification code.

Downstream games and Rulebench depend on the supported facade and SDK.
Asha Rulebench Testing consumes pinned published revisions of both products;
neither product imports the testing repository.

## Content ownership

Asha RPG owns semantic vocabulary, validation, compilation, execution, and
portable replay contracts. Consumers own ordinary named actions, classes,
items, conditions, encounters, rulesets, presentation, and product workflows.
A new name or composition is not a Rust extension. A new meaning that changes
evaluation, legality, timing, mutation, events, randomness, or replay begins in
Rust and then publishes authoring vocabulary.

## Versioning

RPG IR major versions, operation versions, capability versions, Rust facade
versions, and TypeScript package versions evolve independently. Content changes
update content identity only. Unknown required semantic data fails closed.

## Extraction sequence

- #5940: repository and decision-complete ownership bootstrap.
- #5941: coherent extraction of existing portable source and owner tests.
- #5936: normalized IR compiler and Rust semantic kernel (active).
- #5937: constrained TypeScript SDK and deterministic normalizer.
- #5938: Rulebench consumer migration and legacy-path deletion.
- #5939: mechanized boundary enforcement.
