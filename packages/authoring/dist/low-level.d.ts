import type { RulesetCatalogCategory, RulesetCatalogReference, RulesetCatalogValue } from './catalogs.js';
/**
 * Explicit compiler-fixture escape hatch for references that do not come from
 * defineRulesetCatalog. The owner package remains mandatory and participates
 * in normal dependency resolution; this never performs first-match lookup.
 */
export declare function lowLevelCatalogReference<const Category extends RulesetCatalogCategory, const PackageId extends string>(input: {
    readonly category: Category;
    readonly packageId: PackageId;
    readonly definitionId: string;
}): RulesetCatalogReference<Category, PackageId>;
/**
 * Produces canonical normalized-IR data only. High-level authoring builders do
 * not accept the returned bare value. Package identity is required so advanced
 * consumers cannot erase ownership accidentally at the call site.
 */
export declare function unsafeNormalizedCatalogId<const Category extends RulesetCatalogCategory, const PackageId extends string>(input: {
    readonly category: Category;
    readonly packageId: PackageId;
    readonly definitionId: string;
}): RulesetCatalogValue<Category>;
export type { RulesetCatalogValue } from './catalogs.js';
//# sourceMappingURL=low-level.d.ts.map