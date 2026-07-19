import type { RpgDamageType, RpgDefenseId, RpgModifierId, RpgResourceId, RpgStatId } from '@asha-rpg/ir';
import type { RulesetSupportDefinition } from './ruleset-types.js';
export type RulesetCatalogCategory = 'stat' | 'defense' | 'resource' | 'modifier' | 'damageType';
declare const catalogReferenceBrand: unique symbol;
type CatalogValue<Category extends RulesetCatalogCategory> = Category extends 'stat' ? RpgStatId : Category extends 'defense' ? RpgDefenseId : Category extends 'resource' ? RpgResourceId : Category extends 'modifier' ? RpgModifierId : RpgDamageType;
/** A nominal authored ID bound to both its catalog category and owner package. */
export type RulesetCatalogReference<Category extends RulesetCatalogCategory, PackageId extends string> = CatalogValue<Category> & {
    readonly [catalogReferenceBrand]: {
        readonly category: Category;
        readonly packageId: PackageId;
    };
};
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
export {};
//# sourceMappingURL=catalogs.d.ts.map