# @asha-rpg/authoring

Immutable builders and deterministic normalization for the published RPG IR
vocabulary. Pure consumer helpers may compose these builders; normalization
emits data only and Rust remains the semantic authority.

See `../../examples/representative-actions.ts` for branching damage,
turn-duration modifier stacking, bounded movement, source composition, and a
consumer-defined action-family helper.
