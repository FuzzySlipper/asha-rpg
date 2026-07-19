# @asha-rpg/authoring

Immutable builders and deterministic normalization for the published RPG IR
vocabulary. Pure consumer helpers may compose these builders; normalization
emits data only and Rust remains the semantic authority.

Use `defineRulesetCatalog` for nominal category- and package-owned stat,
defense, resource, modifier, and damage references. Action definitions derive
their package graph edges from the immutable action AST. Use `actionPatch`,
`deriveAction`, and `defineRulesetOverlay` for ordinary materialization work;
raw patch records and explicit graph edges are low-level compiler escape
hatches.

See `../../examples/representative-actions.ts` for branching damage,
turn-duration modifier stacking, bounded movement, source composition, and a
consumer-defined action-family helper.
