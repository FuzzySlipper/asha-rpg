import { createLowLevelCatalogReference, } from './catalogs.js';
/**
 * Explicit compiler-fixture escape hatch for references that do not come from
 * defineRulesetCatalog. The owner package remains mandatory and participates
 * in normal dependency resolution; this never performs first-match lookup.
 */
export function lowLevelCatalogReference(input) {
    return createLowLevelCatalogReference(input);
}
/**
 * Produces canonical normalized-IR data only. High-level authoring builders do
 * not accept the returned bare value. Package identity is required so advanced
 * consumers cannot erase ownership accidentally at the call site.
 */
export function unsafeNormalizedCatalogId(input) {
    return createLowLevelCatalogReference(input).definitionId;
}
//# sourceMappingURL=low-level.js.map