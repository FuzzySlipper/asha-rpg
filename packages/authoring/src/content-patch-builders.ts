import { immutable } from './canonical.js';
import {
  catalogDefinitionId,
  retainCatalogOwnership,
} from './catalogs.js';
import type { ContentCatalogReference } from './catalogs.js';
import type {
  ContentPatch,
  ContentPatchNumber,
  ContentPatchOperation,
  ContentPatchPathSegment,
  ContentPatchScalar,
} from './play-bundle-types.js';

export interface NumberAdjustment {
  readonly multiply?: ContentPatchNumber;
  readonly add?: ContentPatchNumber;
}

type PatchPathInput =
  | string
  | Extract<ContentPatchPathSegment, { readonly kind: 'member' }>;

export function patchParameter(id: string): { readonly parameter: string } {
  return immutable({ parameter: id });
}

export function combineContentPatches(
  ...patches: readonly ContentPatch[]
): ContentPatch {
  return patch(patches.flatMap((entry) => entry.operations));
}

export const actionPatch = immutable({
  semantic: immutable({
    maximumRange: numberField('semantic', ['targets', 'maximumRange']),
    maximumTargets: numberField('semantic', ['targets', 'maximumTargets']),
    cost(resource: ContentCatalogReference<'resource', string>) {
      const member = retainCatalogOwnership(
        {
          kind: 'member' as const,
          key: 'resourceId' as const,
          value: catalogDefinitionId(resource),
        },
        [{ field: 'value', reference: resource }],
      );
      return immutable({
        amount: numberField('semantic', ['costs', member, 'amount']),
        remove(): ContentPatch {
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
    set(value: number | { readonly parameter: string }): ContentPatch {
      return patch([
        { kind: 'setScalar', plane, path: segments(path), value },
      ]);
    },
    adjust(options: NumberAdjustment): ContentPatch {
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

function scalarField<Value extends ContentPatchScalar>(
  plane: 'semantic' | 'presentation',
  path: readonly string[],
) {
  return immutable({
    set(value: Value | { readonly parameter: string }): ContentPatch {
      return patch([
        { kind: 'setScalar', plane, path: segments(path), value },
      ]);
    },
  });
}

function upsertScalarField<Value extends ContentPatchScalar>(
  plane: 'semantic' | 'presentation',
  path: readonly string[],
) {
  return immutable({
    set(value: Value | { readonly parameter: string }): ContentPatch {
      return patch([
        { kind: 'upsertScalar', plane, path: segments(path), value },
      ]);
    },
  });
}

function patch(operations: readonly ContentPatchOperation[]): ContentPatch {
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
