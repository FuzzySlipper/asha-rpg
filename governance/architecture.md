# Governance architecture

The canonical repository architecture is `docs/design.md`. This file records
the machine-governed lane shape used during extraction.

Rust layer order is:

```text
rpg-core -> rpg-ir -> rpg-compiler -> rpg-runtime -> asha-rpg
```

`rpg-runtime` owns the single artifact-bound authority session, including its
portable checkpoint and replay surface. Keeping replay on that session avoids
a second state or event-application path. All listed crates and the public
facade are active.

The TypeScript authoring layer is:

```text
@asha-rpg/ir -> @asha-rpg/authoring -> consumer content
```

TypeScript has no edge into Rust private structures. Normalized RPG IR is the
only authored semantic artifact crossing the language boundary.

Both TypeScript packages are active. The IR vocabulary is generated from the
Rust registry, while the authoring package depends only on the IR package.
Repository governance checks the generated output, runtime dependency set, and
production-source import allowlists.
