import {
  createLowLevelCatalogReference,
} from './catalogs.js';
import type {
  ContentCatalogCategory,
  ContentCatalogReference,
  ContentCatalogValue,
} from './catalogs.js';

/**
 * Explicit compiler-fixture escape hatch for references that do not come from
 * defineContentCatalog. The owner package remains mandatory and participates
 * in normal dependency resolution; this never performs first-match lookup.
 */
export function lowLevelCatalogReference<
  const Category extends ContentCatalogCategory,
  const PackageId extends string,
>(input: {
  readonly category: Category;
  readonly packageId: PackageId;
  readonly definitionId: string;
}): ContentCatalogReference<Category, PackageId> {
  return createLowLevelCatalogReference(input);
}

/**
 * Produces canonical normalized-IR data only. High-level authoring builders do
 * not accept the returned bare value. Package identity is required so advanced
 * consumers cannot erase ownership accidentally at the call site.
 */
export function unsafeNormalizedCatalogId<
  const Category extends ContentCatalogCategory,
  const PackageId extends string,
>(input: {
  readonly category: Category;
  readonly packageId: PackageId;
  readonly definitionId: string;
}): ContentCatalogValue<Category> {
  return createLowLevelCatalogReference(input).definitionId;
}

export type { ContentCatalogValue } from './catalogs.js';
