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

The first source-authority extraction owns dependency-free RPG values in
`rpg-core`, normalized rule declarations in `rpg-ir`, and the public-ASHA
RuntimeSession decision/reaction loop in `rpg-runtime`. The `asha-rpg` crate is
the supported Rust facade. `@asha-rpg/ir` and `@asha-rpg/authoring` establish
the permanent TypeScript package boundary; the complete compiler and
authoring language arrive in the next implementation slices.

No Rulebench crate or package is part of this workspace. The independent
`consumers/minimal-game` workspace verifies consumption through the public Git
boundary rather than an unpublished sibling path.

## Checks

```bash
npm test
cargo test --workspace
cargo test --manifest-path consumers/minimal-game/Cargo.toml
```

The canonical architecture is [docs/design.md](docs/design.md). The language
contract is currently maintained in Den document
`asha-rulebench/rpg-rules-language` while the #5934 extraction series runs.
