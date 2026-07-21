import type { RpgDamageType, RpgDefenseId, RpgModifierId, RpgResourceId, RpgStatId } from '@asha-rpg/ir';
import type { ContentSupportDefinition } from './play-bundle-types.js';
export type ContentCatalogCategory = 'stat' | 'defense' | 'resource' | 'modifier' | 'damageType';
declare const catalogReferenceBrand: unique symbol;
export type ContentCatalogValue<Category extends ContentCatalogCategory> = Category extends 'stat' ? RpgStatId : Category extends 'defense' ? RpgDefenseId : Category extends 'resource' ? RpgResourceId : Category extends 'modifier' ? RpgModifierId : RpgDamageType;
/** A nominal authored ID bound to both its catalog category and owner package. */
export type ContentCatalogReference<Category extends ContentCatalogCategory, PackageId extends string> = Readonly<{
    readonly definitionId: ContentCatalogValue<Category>;
    readonly category: Category;
    readonly packageId: PackageId;
    readonly [catalogReferenceBrand]: true;
}>;
export interface AuthoredCatalogOwnership {
    readonly field: string;
    readonly definitionId: string;
    readonly category: ContentCatalogCategory;
    readonly packageId: string;
}
export interface ContentCatalogEntry<Category extends ContentCatalogCategory = ContentCatalogCategory> {
    readonly definitionId: string;
    readonly category: Category;
    readonly id: string;
    readonly label: string;
    readonly description?: string;
    readonly tags?: readonly string[];
}
export interface ContentCatalog<PackageId extends string, Entries extends Readonly<Record<string, ContentCatalogEntry>>> {
    readonly packageId: PackageId;
    readonly definitions: readonly ContentSupportDefinition[];
    readonly references: {
        readonly [Key in keyof Entries]: ContentCatalogReference<Entries[Key]['category'], PackageId>;
    };
}
export declare function defineContentCatalog<const PackageId extends string, const Entries extends Readonly<Record<string, ContentCatalogEntry>>>(input: {
    readonly packageId: PackageId;
    readonly sourceModule: string;
    readonly entries: Entries;
}): ContentCatalog<PackageId, Entries>;
export declare function catalogDefinitionId<Category extends ContentCatalogCategory>(reference: ContentCatalogReference<Category, string>): ContentCatalogValue<Category>;
/** @internal Used only by the explicit low-level authoring subpath. */
export declare function createLowLevelCatalogReference<const Category extends ContentCatalogCategory, const PackageId extends string>(input: {
    readonly category: Category;
    readonly packageId: PackageId;
    readonly definitionId: string;
}): ContentCatalogReference<Category, PackageId>;
/** @internal Retains authored owner identity on an AST node without serializing it. */
export declare function retainCatalogOwnership<Value extends object>(value: Value, fields: readonly {
    readonly field: string;
    readonly reference: unknown;
}[]): Value;
/** @internal Reads owner identity retained by the typed authoring builders. */
export declare function catalogOwnershipOf(value: object): readonly AuthoredCatalogOwnership[];
export {};
//# sourceMappingURL=catalogs.d.ts.map