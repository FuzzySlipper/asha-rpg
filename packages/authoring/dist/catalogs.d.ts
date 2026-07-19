import type { RpgDamageType, RpgDefenseId, RpgModifierId, RpgResourceId, RpgStatId } from '@asha-rpg/ir';
import type { RulesetSupportDefinition } from './ruleset-types.js';
export type RulesetCatalogCategory = 'stat' | 'defense' | 'resource' | 'modifier' | 'damageType';
declare const catalogReferenceBrand: unique symbol;
export type RulesetCatalogValue<Category extends RulesetCatalogCategory> = Category extends 'stat' ? RpgStatId : Category extends 'defense' ? RpgDefenseId : Category extends 'resource' ? RpgResourceId : Category extends 'modifier' ? RpgModifierId : RpgDamageType;
/** A nominal authored ID bound to both its catalog category and owner package. */
export type RulesetCatalogReference<Category extends RulesetCatalogCategory, PackageId extends string> = Readonly<{
    readonly definitionId: RulesetCatalogValue<Category>;
    readonly category: Category;
    readonly packageId: PackageId;
    readonly [catalogReferenceBrand]: true;
}>;
export type RulesetCatalogInput<Category extends RulesetCatalogCategory> = RulesetCatalogValue<Category> | RulesetCatalogReference<Category, string>;
export interface AuthoredCatalogOwnership {
    readonly field: string;
    readonly definitionId: string;
    readonly category: RulesetCatalogCategory;
    readonly packageId: string;
}
export interface RulesetCatalogEntry<Category extends RulesetCatalogCategory = RulesetCatalogCategory> {
    readonly definitionId: string;
    readonly category: Category;
    readonly id: string;
    readonly label: string;
    readonly description?: string;
    readonly tags?: readonly string[];
}
export interface RulesetCatalog<PackageId extends string, Entries extends Readonly<Record<string, RulesetCatalogEntry>>> {
    readonly packageId: PackageId;
    readonly definitions: readonly RulesetSupportDefinition[];
    readonly references: {
        readonly [Key in keyof Entries]: RulesetCatalogReference<Entries[Key]['category'], PackageId>;
    };
}
export declare function defineRulesetCatalog<const PackageId extends string, const Entries extends Readonly<Record<string, RulesetCatalogEntry>>>(input: {
    readonly packageId: PackageId;
    readonly sourceModule: string;
    readonly entries: Entries;
}): RulesetCatalog<PackageId, Entries>;
export declare function catalogDefinitionId<Category extends RulesetCatalogCategory>(reference: RulesetCatalogInput<Category>): RulesetCatalogValue<Category>;
/** @internal Retains authored owner identity on an AST node without serializing it. */
export declare function retainCatalogOwnership<Value extends object>(value: Value, fields: readonly {
    readonly field: string;
    readonly reference: unknown;
}[]): Value;
/** @internal Reads owner identity retained by the typed authoring builders. */
export declare function catalogOwnershipOf(value: object): readonly AuthoredCatalogOwnership[];
export {};
//# sourceMappingURL=catalogs.d.ts.map