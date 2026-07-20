# asha-rpg

`asha-rpg` is the portable ECRP RPG domain substrate for ASHA consumers.
Rust owns semantic validation, compilation, deterministic resolution, accepted
DomainEvents, capability mutation, trace, replay contracts, and typed views.
The supported TypeScript packages own immutable authoring syntax and
deterministic normalization into versioned RPG IR. TypeScript never executes
gameplay semantics.

## Repository position

```text
public asha-engine APIs
          |
          v
      asha-rpg
       /     \
      v       v
downstream games   asha-rulebench
                         |
                         v
              asha-rulebench-testing
```

`asha-rpg` never depends on Rulebench or its testing repository. Consumer
content such as named actions, classes, items, conditions, encounters, and
rulesets remains in the owning game or workbench.

## Current implementation

The Rust semantic path is active. `rpg-ir` strictly decodes `asha.rpg.ir@1`,
`rpg-compiler` resolves exact requirements and references into an opaque
compiled program with closed operation bindings, and `rpg-runtime` owns a
versioned, artifact-bound encounter setup and authority session that stages
state, explicit random evidence, typed reaction decisions, and turn
progression together. Structured readbacks expose the board, participants,
current actor, legal actions/targets, reaction, log, and outcome without
moving rule interpretation into a host. The `asha-rpg` crate is the supported
consumer facade. There is no parallel gameplay fabric or disposable preview session.
There is also no predecessor provider/module/action-definition compatibility
surface: normalized RPG IR, the compiled artifact, and the artifact-bound
authority session are the single supported Rust ruleset path.

The initial operation set is deliberately closed: damage, healing, resource
change, bounded grid movement, turn-bounded modifier application with explicit
replace or refresh stacking, and a typed before-damage reaction window. Checks
support attack, saving throw, and no-roll flows. Programs
support bounded sequence, predicate branch, repeat, per-target branch, check
outcome branch, and one atomic root. Unavailable semantics fail compilation;
they are never delegated to consumer callbacks.

`@asha-rpg/ir` publishes the strict normalized data contract and a checked
vocabulary generated from the Rust registry. `@asha-rpg/authoring` publishes
immutable catalog-bound ids, selectors, checks, formulas, predicates, costs,
duration/stacking/timing data, operations, bounded composition, consumer source
composition, explicit versioned ruleset package manifests, exact dependency
resolution, exported-root closure, diagnostics, and canonical normalization.
Action AST traversal derives package closure and semantic requirements without
consumer-maintained reference ledgers, while schema-aware patch builders own
valid paths, operations, stable member selectors, and impact planes.
It does not evaluate any gameplay semantics or discover packages from global
registries or the filesystem. Rust validates the prepared graph, creates the
private executable plan, and emits the closed `asha.rpg.ruleset.compiled@1`
artifact with independent source, semantic, and presentation fingerprints.
Representative consumer code lives in
`examples/representative-actions.ts`; its normalized artifact is sent through
the Rust compiler during `npm test`.

Derivation, ordered typed mixins, local relational patches, package overlays,
and exposed configuration options are materialized deterministically during
authoring compilation. The emitted artifact contains no runtime inheritance or
plugin graph: only final definitions, exact definition fingerprints, and typed
source-to-effective-value provenance. Rust independently validates those
closed records and recompiles gameplay semantics from the materialized graph.
Runtime activation remains a downstream host responsibility.

The same artifact-bound authority session now owns the supported portable
checkpoint and replay contract. A checkpoint embeds the exact validated
compiled artifact, encounter setup and setup fingerprint, a stable
capability-state projection, current turn and accepted-event log, the accepted
random position and source binding, the full ready or awaiting-reaction phase,
operation/capability/event schema versions, and a canonical session-state hash. Replay restores the
embedded artifact without executing authoring code or resolving packages, then
re-enters the normal Rust submit/reaction paths and verifies structured random
evidence, accepted events, turns, revisions, phases, and hashes. Restore and replay
construct a temporary session and replace a target only after complete
validation, so corrupt input is atomic. Compatibility inspection classifies
source, presentation, semantic, package-lock, and artifact drift without ever
substituting a candidate for historical authority.

No Rulebench crate or package is part of this workspace. The independent
`consumers/minimal-game` workspace verifies consumption through the public Git
boundary rather than an unpublished sibling path.

## Checks

```bash
npm test
npm run build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node examples/generate-portable-replay-source.ts | \
  cargo run --manifest-path consumers/minimal-game/Cargo.toml
```

The canonical architecture is [docs/design.md](docs/design.md), and the public
setup/random/turn compatibility surface is documented in
[docs/encounter-session-contract.md](docs/encounter-session-contract.md). The language
contract is currently maintained in Den document
`asha-rulebench/rpg-rules-language` while the #5934 extraction series runs.
