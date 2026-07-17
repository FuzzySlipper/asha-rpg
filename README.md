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
compiled program with closed operation bindings, and `rpg-runtime` owns an
authority session that stages state and deterministic randomness together.
The `asha-rpg` crate is the supported consumer facade. The extracted public-ASHA
decision/reaction fabric remains alongside this semantic path.

The initial operation set is deliberately closed: damage, healing, resource
change, bounded grid movement, and turn-bounded modifier application with
explicit replace or refresh stacking. Checks support attack, saving throw, and no-roll flows. Programs
support bounded sequence, predicate branch, repeat, per-target branch, check
outcome branch, and one atomic root. Unavailable semantics fail compilation;
they are never delegated to consumer callbacks.

`@asha-rpg/ir` publishes the strict normalized data contract and a checked
vocabulary generated from the Rust registry. `@asha-rpg/authoring` publishes
immutable branded ids, selectors, checks, formulas, predicates, costs,
duration/stacking/timing data, operations, bounded composition, consumer source
composition, diagnostics, and canonical normalization. It does not evaluate
any of those semantics. Representative consumer code lives in
`examples/representative-actions.ts`; its normalized artifact is sent through
the Rust compiler during `npm test`.

No Rulebench crate or package is part of this workspace. The independent
`consumers/minimal-game` workspace verifies consumption through the public Git
boundary rather than an unpublished sibling path.

## Checks

```bash
npm test
npm run build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path consumers/minimal-game/Cargo.toml
```

The canonical architecture is [docs/design.md](docs/design.md). The language
contract is currently maintained in Den document
`asha-rulebench/rpg-rules-language` while the #5934 extraction series runs.
