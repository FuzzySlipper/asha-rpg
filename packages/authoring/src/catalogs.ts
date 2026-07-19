import type {
  RpgDamageType,
  RpgDefenseId,
  RpgModifierId,
  RpgResourceId,
  RpgStatId,
} from '@asha-rpg/ir';

import { immutable } from './canonical.js';
import type { RulesetSupportDefinition } from './ruleset-types.js';

export type RulesetCatalogCategory =
  | 'stat'
  | 'defense'
  | 'resource'
  | 'modifier'
  | 'damageType';

declare const catalogReferenceBrand: unique symbol;

type CatalogValue<Category extends RulesetCatalogCategory> =
  Category extends 'stat'
    ? RpgStatId
    : Category extends 'defense'
      ? RpgDefenseId
      : Category extends 'resource'
        ? RpgResourceId
        : Category extends 'modifier'
          ? RpgModifierId
          : RpgDamageType;

/** A nominal authored ID bound to both its catalog category and owner package. */
export type RulesetCatalogReference<
  Category extends RulesetCatalogCategory,
  PackageId extends string,
> = CatalogValue<Category> & {
  readonly [catalogReferenceBrand]: {
    readonly category: Category;
    readonly packageId: PackageId;
  };
};

export interface RulesetCatalogEntry<
  Category extends RulesetCatalogCategory = RulesetCatalogCategory,
> {
  readonly definitionId: string;
  readonly category: Category;
  readonly id: string;
  readonly label: string;
  readonly description?: string;
  readonly tags?: readonly string[];
}

export interface RulesetCatalog<
  PackageId extends string,
  Entries extends Readonly<Record<string, RulesetCatalogEntry>>,
> {
  readonly packageId: PackageId;
  readonly definitions: readonly RulesetSupportDefinition[];
  readonly references: {
    readonly [Key in keyof Entries]: RulesetCatalogReference<
      Entries[Key]['category'],
      PackageId
    >;
  };
}

export function defineRulesetCatalog<
  const PackageId extends string,
  const Entries extends Readonly<Record<string, RulesetCatalogEntry>>,
>(input: {
  readonly packageId: PackageId;
  readonly sourceModule: string;
  readonly entries: Entries;
}): RulesetCatalog<PackageId, Entries> {
  assertIdentifier(input.packageId, 'catalog package id');
  if (input.sourceModule.length === 0) {
    throw new Error('catalog source module must not be empty');
  }

  const definitions: RulesetSupportDefinition[] = [];
  const references: Record<string, string> = {};
  for (const [name, entry] of Object.entries(input.entries)) {
    assertIdentifier(name, 'catalog entry name');
    assertIdentifier(entry.definitionId, 'catalog definition id');
    assertIdentifier(entry.id, 'catalog semantic id');
    if (entry.label.length === 0) throw new Error('catalog label must not be empty');
    definitions.push(
      immutable({
        kind: 'support' as const,
        id: entry.definitionId,
        visibility: 'public' as const,
        extensionPolicy: 'sealed' as const,
        source: {
          module: input.sourceModule,
          declaration: name,
        },
        presentation: {
          label: entry.label,
          ...(entry.description === undefined
            ? {}
            : { description: entry.description }),
          ...(entry.tags === undefined ? {} : { tags: [...entry.tags] }),
        },
        semantic: { catalog: entry.category, id: entry.id },
      }),
    );
    references[name] = entry.definitionId;
  }

  definitions.sort((left, right) => left.id.localeCompare(right.id));
  return immutable({
    packageId: input.packageId,
    definitions: immutable(definitions),
    references: immutable(references),
  }) as unknown as RulesetCatalog<PackageId, Entries>;
}

function assertIdentifier(value: string, label: string): void {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)) {
    throw new Error(`${label} must be a non-empty portable identifier`);
  }
}
