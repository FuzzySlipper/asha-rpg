# Non-claims

Asha RPG is not:

- a complete RPG or balance model;
- a generic rules engine or dynamic plugin system;
- a TypeScript runtime evaluator or semantic preview fallback;
- a mutable gameplay-context or callback API;
- a Rulebench UI, protocol, process host, archive, experiment, or storage layer;
- an exhaustive fixture, golden, browser, compatibility, or certification repo;
- a D&D compatibility target, campaign system, or broad character builder;
- a home for every consumer's named content catalog.

The initial semantic profile does not yet claim contested checks,
source-specific critical damage rules, area selectors, conditions, temporary vitality,
permanent or scheduler-relative durations, persistent-modifier stacking policies,
or reaction-window authoring. Unknown requirements for those meanings fail
closed. Portable checkpoint and replay are part of the existing authority
session; a separate replay engine, event-applier, or state path is not an
implementation claim.

Scenario is setup-only data, not a gameplay runner, campaign save, AI plan, or
product protocol. Version 1 claims entity target candidates and one
artifact-authored cell-target shape for selected-destination movement: an
unconditional no-roll branch containing only one `moveToCell` operation. Rust
projects deterministic least-cost routes over orthogonally adjacent authored
cells, charges each entered cell's traversal cost (or one by default), and
excludes occupied or impassable cells. Commands submit only the destination;
Rust recomputes the route against current authority state before atomically
committing it within the authored movement-cost bound. Diagonal travel,
participant footprints, forced movement, conditional, repeated,
random-composed, area, and general cell-target semantics remain non-claims.

Typed item instances and initial equipment are authority-owned setup facts.
Inventory economy, loot, encumbrance, consumable depletion, and gameplay
equip/unequip operations remain non-claims.

The TypeScript packages do not provide semantic preview, target evaluation,
dice execution, predicate evaluation, state access, effect execution, or a
mutable gameplay context. Their structural diagnostics are convenience only;
Rust compilation remains authoritative.

The contextual contribution ledger and ordered scalar-test profile are closed
contracts, not a generic modifier or scripting facility. They do not provide
arbitrary option bags,
mutable toggles, user-authored predicates, pool/category transformations,
probability previews, or calculation owners beyond the currently bound attack
modifier and scalar-test profiles. `F3` through `F6` in the first-wave catalog
remain implementation non-claims.

The implemented variable activation-budget model is deliberately turn-relative:
Rulesets may declare action and reaction budgets with owner-turn-start or
round-start reset boundaries and a bounded accepted-activation ceiling. It is
not a scheduler, real-time economy, rest/day recovery system,
initiative-derived budget calculator, borrowing model, or delay/ready system.

Scalar-test base and explicit-difficulty expressions cannot request formula
dice. The profile's one primary die is their complete random surface;
additional or heterogeneous dice belong to the deferred pool contract.
