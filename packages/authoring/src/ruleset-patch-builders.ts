import { immutable } from './canonical.js';
import { catalogDefinitionId } from './catalogs.js';
import type { RulesetCatalogInput } from './catalogs.js';
import type {
  RulesetPatch,
  RulesetPatchNumber,
  RulesetPatchOperation,
  RulesetPatchPathSegment,
  RulesetPatchScalar,
} from './ruleset-types.js';

export interface NumberAdjustment {
  readonly multiply?: RulesetPatchNumber;
  readonly add?: RulesetPatchNumber;
}

type PatchPathInput =
  | string
  | Extract<RulesetPatchPathSegment, { readonly kind: 'member' }>;

export function patchParameter(id: string): { readonly parameter: string } {
  return immutable({ parameter: id });
}

export function combineRulesetPatches(
  ...patches: readonly RulesetPatch[]
): RulesetPatch {
  return patch(patches.flatMap((entry) => entry.operations));
}

export const actionPatch = immutable({
  semantic: immutable({
    maximumRange: numberField('semantic', ['targets', 'maximumRange']),
    maximumTargets: numberField('semantic', ['targets', 'maximumTargets']),
    cost(resource: RulesetCatalogInput<'resource'>) {
      const member = {
        kind: 'member' as const,
        key: 'resourceId' as const,
        value: catalogDefinitionId(resource),
      };
      return immutable({
        amount: numberField('semantic', ['costs', member, 'amount']),
        remove(): RulesetPatch {
          return patch([
            {
              kind: 'removeMember',
              plane: 'semantic',
              path: [field('costs')],
              identity: member,
            },
          ]);
        },
      });
    },
  }),
  presentation: immutable({
    label: scalarField<string>('presentation', ['label']),
    description: upsertScalarField<string>('presentation', ['description']),
  }),
});

function numberField(
  plane: 'semantic' | 'presentation',
  path: readonly PatchPathInput[],
) {
  return immutable({
    set(value: number | { readonly parameter: string }): RulesetPatch {
      return patch([
        { kind: 'setScalar', plane, path: segments(path), value },
      ]);
    },
    adjust(options: NumberAdjustment): RulesetPatch {
      return patch([
        {
          kind: 'adjustNumber',
          plane,
          path: segments(path),
          multiply: options.multiply ?? 1,
          add: options.add ?? 0,
        },
      ]);
    },
  });
}

function scalarField<Value extends RulesetPatchScalar>(
  plane: 'semantic' | 'presentation',
  path: readonly string[],
) {
  return immutable({
    set(value: Value | { readonly parameter: string }): RulesetPatch {
      return patch([
        { kind: 'setScalar', plane, path: segments(path), value },
      ]);
    },
  });
}

function upsertScalarField<Value extends RulesetPatchScalar>(
  plane: 'semantic' | 'presentation',
  path: readonly string[],
) {
  return immutable({
    set(value: Value | { readonly parameter: string }): RulesetPatch {
      return patch([
        { kind: 'upsertScalar', plane, path: segments(path), value },
      ]);
    },
  });
}

function patch(operations: readonly RulesetPatchOperation[]): RulesetPatch {
  return immutable({ version: 1, operations: [...operations] });
}

function segments(
  values: readonly PatchPathInput[],
) {
  return values.map((value) =>
    typeof value === 'string' ? field(value) : value,
  );
}

function field(name: string) {
  return { kind: 'field' as const, name };
}
