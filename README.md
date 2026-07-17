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

## Bootstrap status

Task #5940 establishes this repository and its governance boundary. The
portable implementation is intentionally not copied here yet; #5941 performs
the coherent source-of-authority extraction. Planned ownership cells are
recorded in `governance/ownership.toml` and are marked `planned` until their
source and owner-local tests move together.

## Checks

```bash
npm test
cargo metadata --no-deps --format-version 1
```

The canonical architecture is [docs/design.md](docs/design.md). The language
contract is currently maintained in Den document
`asha-rulebench/rpg-rules-language` while the #5934 extraction series runs.
