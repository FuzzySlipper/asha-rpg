# @asha-rpg/authoring

Immutable builders for four explicit contracts:

- `defineRuleset`, `rulesetStat`, and `rulesetDefense` describe Rust-executed
  semantic provisions and ergonomic named values;
- `defineContentPack` owns authored definitions, presentation, dependencies,
  derivation, mixins, and overlays;
- `composePlayBundle` plus `preparePlayBundle` resolve one Ruleset and selected
  compatible Content Packs;
- `defineScenario` creates setup-only data for one compiled PlayBundle.
- `defineScenarioTemplate` publishes artifact-independent setup examples;
  `instantiateScenarioTemplate` binds one to an explicitly chosen compiled
  PlayBundle artifact.

Use `defineContentCatalog` for Content Pack-owned resources, modifiers, damage
types, and presentation aliases. Action AST references close the package graph
without a second ledger. Use `actionPatch`, `deriveAction`, and
`defineContentOverlay` for materialization; raw patches and explicit graph edges
are low-level compiler-fixture escape hatches.

Ordinary support definitions may also carry a consumer-owned catalog name and
inert `semantic.data`. This is intended for setup profiles, item grants, and
similar product data that must survive PlayBundle compilation. It is never an
executable callback surface; Rust only interprets the registered action
catalogs and operations.

Use `defineParticipantProfileDefinition` for portable participant defaults. It
stores capabilities as Scenario DTOs and closes the definition graph over the
profile's selected action/content ids, so consumers do not maintain a second
reference ledger.

The package emits data only. Rust remains semantic and state authority.
