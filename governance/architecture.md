# Governance architecture

The canonical repository architecture is `docs/design.md`. This file records
the machine-governed lane shape used during extraction.

Rust layer order is:

```text
rpg-core -> rpg-ir -> rpg-compiler -> rpg-runtime -> rpg-replay -> asha-rpg
```

During the staged extraction, `rpg-core`, `rpg-ir`, `rpg-runtime`, and the
public facade are active. The compiler and replay cells remain reserved for
their owner tasks; active crates do not conceal them behind placeholder APIs.

The TypeScript authoring layer is:

```text
@asha-rpg/ir -> @asha-rpg/authoring -> consumer content
```

TypeScript has no edge into Rust private structures. Normalized RPG IR is the
only authored semantic artifact crossing the language boundary.
