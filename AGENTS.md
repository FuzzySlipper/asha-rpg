# AGENTS.md

## Den guidance bootstrap

- Project ID: `asha-rpg`
- Resolve live guidance with the Den MCP `get_agent_guidance` tool before substantial work.
- Den owns current tasks, planning, review packets, and known limitations.
- If Den is unreachable, stop and report the failed tool and intended action.

## Architecture soul

> TypeScript composes immutable RPG intent. Rust validates and executes RPG meaning.

- Rust owns selectors, checks, formulas, predicates, operations, timing,
  mutation, DomainEvents, deterministic randomness, trace, replay, and views.
- TypeScript authoring helpers may only construct immutable AST nodes and
  normalize them to published RPG IR.
- No callback, mutable gameplay context, executable TypeScript, browser API,
  host route, storage adapter, or product DTO may enter the portable boundary.
- Capability stores are private to their Rust mutation owners. Rules use
  declared typed reads and staged owner transactions.
- Consumer content remains downstream. Do not add named Rulebench catalogs to
  this repository as a shortcut.

## Source-of-truth posture

- `docs/design.md` is the canonical committed architecture.
- `governance/ownership.toml` assigns every crate and package.
- `governance/dependency-policy.toml` defines dependency direction.
- `governance/upstream-asha.toml` records allowed exact public ASHA inputs.
- Code and focused owner-local tests are implementation truth.

## Commands

```bash
npm test
cargo metadata --no-deps --format-version 1
```

Do not describe planned ownership cells as implemented. New implementation
must arrive with its owner-local tests and an updated ownership entry.
