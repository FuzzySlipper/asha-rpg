# Governance architecture

The canonical repository architecture is `docs/design.md`. This file records
the machine-governed lane shape used during extraction.

Rust layer order is:

```text
rpg-core -> rpg-ir -> rpg-compiler -> rpg-runtime -> rpg-replay -> asha-rpg
```

The TypeScript authoring layer is:

```text
@asha-rpg/ir -> @asha-rpg/authoring -> consumer content
```

TypeScript has no edge into Rust private structures. Normalized RPG IR is the
only authored semantic artifact crossing the language boundary.
