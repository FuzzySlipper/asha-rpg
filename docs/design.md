# Asha RPG design

## Purpose

Asha RPG is a portable, downstream-game-compatible RPG authority substrate. It
supplies Rust-owned RPG semantics and a TypeScript authoring frontend that
compiles data into normalized RPG IR. It is not a game, workbench, host,
archive browser, or proof harness.

## Four representations

1. TypeScript authoring AST and immutable builders optimize for composition.
2. Explicit package manifests resolve into a closed prepared definition graph
   and version lock, then normalize to versioned RPG IR.
3. Rust validates that graph, builds a private compiled ruleset, and emits a
   closed portable artifact for interchange and activation.
4. Runtime capability state, workspaces, accepted DomainEvents, trace, replay
   inputs, and typed views implement the ECRP loop.

These are one-way compilation stages, not generated mirrors. TypeScript helper
functions disappear during normalization. Rust performs final compatibility,
reference, and semantic validation.

## Authority loop

```text
typed intent -> compiled Rules -> staged authority transaction -> accepted DomainEvents
                         reaction decision / random evidence ^
             -> capability mutation owners -> typed views
```

A rejected resolution commits no capability mutation. Cross-capability actions
use one session-owned transaction with declared reads and expected revisions.
A reaction suspends that transaction and resumes it against the same base
revision; no cost, randomness, or gameplay state becomes observable before the
resumed command is accepted. Random requests preserve their declared die shape
and canonical target order. Trace explains authority decisions but is not
mutation input.

## Public surfaces

The permanent public surface includes:

- Rust facade `asha-rpg`, re-exporting only supported portable contracts;
- normalized IR decode and compatibility contracts;
- a Rust compiler and semantic kernel over closed operation registrations;
- typed intents, accepted DomainEvents, replayable authority records, and views;
- TypeScript packages `@asha-rpg/ir` and `@asha-rpg/authoring`.

There is one public Rust ruleset model: normalized RPG IR is independently
validated and semantically compiled into a closed artifact, and that artifact
is loaded by the persistent authority session. Predecessor provider catalogs,
static ruleset modules, ability/spell kinds, action-resource kinds, targeting
declarations, and effect declarations are not compatibility surfaces. Closed
executor enums remain private to their semantic owners.

The compiled artifact contains its schema and composition identity, language
identity, exact source and dependency lock, operation and capability
requirements, exported roots and materialized definition closure, policy
bindings, relationship and definition provenance, typed derivation and overlay
provenance records, and separate source, semantic, and presentation
fingerprints. Runtime semantics are reconstructed from that single materialized
definition graph. The artifact contains no executable TypeScript, callbacks,
floating dependencies, filesystem discovery state, or private Rust plan.
Private compiled structures, capability-store layout, ASHA routing envelopes,
and optimization indexes are not serialization contracts.

## Initial Rust semantic profile

The active compatibility profile is:

| Surface | Supported version or vocabulary |
| --- | --- |
| normalized IR | `asha.rpg.ir` major 1 |
| operations | `operation.damage@1`, `operation.heal@1`, `operation.changeResource@1`, `operation.applyModifier@1`, `operation.move@1`, `operation.openReaction@1` |
| capabilities | vitality, stats, defenses, resources, modifiers, position, deterministic random, reactions, each at version 1 |
| checks | attack, saving throw, no roll |
| formulas | constant, typed stat read, add, bounded dice, half |
| predicates | always, comparison, not, all, any |
| composition | atomic root, sequence, when, bounded repeat, bounded per-target, check branch |
| modifier tenure | 1 to 1000 turns with replace or refresh stacking; unchanged modifiers age once per accepted encounter turn transition and emit duration/expiry events |
| movement | bounded signed grid delta with explicit provoke behavior |

Strict decode rejects unknown semantic fields. Compatibility requires exact
operation and capability versions. Compilation resolves catalog references,
checks declared reads and owners, enforces expression/program/expanded-program
bounds, and binds every operation to a static Rust registration. The compiled
program and capability plan are private; consumers receive identity,
requirement, intent, event, trace, rejection, receipt, state-view, and session
surfaces only.

The persistent authority session clones capability state and explicit random
evidence into a workspace. Costs, checks, branches, reaction decisions, and
owner mutations are staged there. A successful action advances the state
revision and emits accepted DomainEvents plus explanatory trace. A rejection
returns stable code/path evidence while leaving the authoritative state
unchanged. A pending reaction blocks other commands until it is resolved.

Artifact-bound session construction consumes
`asha.rpg.encounter.setup@1`, not consumer-built mutable capability state. The
strict setup describes only the initial board, typed cell capabilities,
participants and their artifact definition references, initial owner values,
initiative state, and an exact random policy/source binding. Rust validates
the whole setup before authority exists. The same session owns current actor,
round/turn advancement, legal action and target readbacks, pending reaction,
accepted-event log, and encounter outcome. Team identity is an open typed id,
not a closed product enum. The detailed compatibility contract is
`docs/encounter-session-contract.md`.

That same artifact-bound session owns portable persistence. Its versioned
checkpoint embeds the exact closed artifact, exact setup plus setup
fingerprint, a stable list-based projection of capability state rather than
private maps, current turn and accepted-event log, the cumulative
accepted-random position, and either a ready phase or the complete pending
transaction needed to resume one reaction. The canonical hash covers setup,
state, turn, log, random position, and phase. Replay records typed
submit/reaction operations, structured random requests and values, accepted
events, turns, revisions, source binding, phases, and before/after hashes.
Replay loads the embedded artifact and invokes the ordinary session paths; it
never rematerializes content, resolves a range, regenerates randomness, or
reapplies events. Restore/replay validate into a temporary session before
replacing a target.
Product storage, browsing, migration policy, and exhaustive compatibility
matrices remain downstream responsibilities.

## TypeScript authoring profile

`@asha-rpg/ir` owns only immutable normalized data types plus operation and
capability version maps generated from the Rust registry. The checked generator
fails when those maps drift. `@asha-rpg/authoring` owns a distinct ergonomic AST
whose builders return frozen data. Authoring-only immediate timing markers and
consumer helper identity are eliminated during normalization.

The normalizer performs deterministic structural work only: it expands
consumer-composed action sources, sorts semantic catalogs and actions, derives
exact requirements and definition-graph edges from data use, attaches stable
path/source-path diagnostics, wraps each action in one atomic root, and emits
recursively key-sorted canonical JSON. Catalog builders return nominal
category- and package-owned references, so a stat cannot accidentally stand in
for a defense and ordinary actions do not maintain parallel reference arrays.
It does not roll dice, compare formulas, choose branches, test legality, apply
stacking, move entities, or mutate state. Every representative artifact is
passed to the Rust compiler as the final acceptance authority.

Package selection is equally explicit and deterministic. A composition names
one base plus additions, overlays, and configuration values. The caller passes
the available immutable package sources directly; there is no ambient registry
or directory scan. Resolution produces exact versions and fingerprints,
validates dependency aliases and compatibility, walks exported-root closure,
rejects private cross-package access and unreachable public definitions, and
materializes only the reachable graph. Rust then repeats artifact-level
compatibility and closure checks before semantic compilation. Loading a stored
artifact recompiles those contents and requires an exact artifact match.

The authoring compiler now materializes one primary base, explicitly ordered
typed mixins, local relational patches, authorized composition-ordered package
overlays, and deliberately exposed configuration options. Ordinary authoring
uses schema-aware patch builders: valid fields and operations are fixed by the
builder, semantic versus presentation planes are derived, and list edits use
stable member selectors rather than indexes. Raw patch AST and explicit graph
edges remain named low-level escape hatches for compiler fixtures. Every
applied step records before/after values, exact package and definition
identities, parameters, order, conflict policy, impact plane, and fingerprints.
Cycles, excessive depth, private or sealed targets, missing parameters,
ambiguous members, conflicting writes, and expected-fingerprint drift fail
before normalization.

This phase remains deterministic structural compilation over immutable data; it
does not evaluate gameplay meaning. The portable artifact contains only fully
materialized definitions plus typed provenance. Rust rejects unresolved or
internally inconsistent records, independently verifies every final definition
fingerprint and the closed artifact fingerprint planes, and compiles gameplay
semantics from the materialized graph only.

Archetype, item, and scenario authoring helpers are pure action-composition
sources in the initial profile. Their helper identities do not become new IR
definition categories. This keeps ordinary consumer composition independent of
Rust, product protocols, hosts, and runtime routing.

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

## Governed extension paths

The checked amplification contract reports three downstream layers for an
ordinary content-only addition: TypeScript content/composition, its owner-local
normalization expectation, and the generated normalized IR artifact. Rust,
product protocols, host routes, capability manifests, and certification proof
are forbidden amplification for that change class.

A new semantic operation crosses seven explicit owner layers. Its Rust
registration must declare reads, mutation owner, validation behavior, accepted
DomainEvents, trace behavior, and replay implications before the vocabulary
generator will publish authoring-facing identity. The complete checklist lives
in `governance/boundary-rules.md`; the machine-readable layer report lives in
`governance/change-amplification.json`.

Non-claims: the report is an architectural change contract, not a claim that
all semantic operations are implemented, that TypeScript policy can execute
rules, or that product proof belongs here. Focused owner tests stay in this
repository; exhaustive cross-product boundary proof belongs to
`asha-rulebench-testing`.

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
