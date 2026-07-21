# @asha-rpg/authoring

Immutable builders for four explicit contracts:

- `defineRuleset`, `rulesetStat`, and `rulesetDefense` describe Rust-executed
  semantic provisions and ergonomic named values;
- `defineContentPack` owns authored definitions, presentation, dependencies,
  derivation, mixins, and overlays;
- `composePlayBundle` plus `preparePlayBundle` resolve one Ruleset and selected
  compatible Content Packs;
- `defineScenario` creates setup-only data for one compiled PlayBundle.

Use `defineContentCatalog` for Content Pack-owned resources, modifiers, damage
types, and presentation aliases. Action AST references close the package graph
without a second ledger. Use `actionPatch`, `deriveAction`, and
`defineContentOverlay` for materialization; raw patches and explicit graph edges
are low-level compiler-fixture escape hatches.

The package emits data only. Rust remains semantic and state authority.
