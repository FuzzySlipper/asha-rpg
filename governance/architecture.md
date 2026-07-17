# Governance architecture

The canonical repository architecture is `docs/design.md`. This file records
the machine-governed lane shape used during extraction.

Rust layer order is:

```text
rpg-core -> rpg-ir -> rpg-compiler -> rpg-runtime -> rpg-replay -> asha-rpg
```

During the staged extraction, `rpg-core`, `rpg-ir`, `rpg-compiler`,
`rpg-runtime`, and the public facade are active. The replay cell remains
reserved for its owner task; active crates do not conceal it behind a
placeholder API.

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
