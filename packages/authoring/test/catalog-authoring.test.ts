import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  action,
  actionId,
  actionPatch,
  attack,
  changeResource,
  canonicalJson,
  constant,
  defineActionDefinition,
  defineContentCatalog,
  defineContentPack,
  definitionReference,
  noRoll,
  onCheck,
  preparePlayBundle,
  readStat,
  contentPackDependency,
  contentPackRequest,
  contentPackSource,
  spend,
  withLowLevelDefinitionReferences,
} from '@asha-rpg/authoring';
import type { ContentCatalogReference } from '@asha-rpg/authoring';
import { unsafeNormalizedCatalogId } from '@asha-rpg/authoring/low-level';
import { contractTestRuleset } from './test-ruleset.ts';

const primitives = defineContentCatalog({
  packageId: 'sample.primitives',
  sourceModule: 'sample/primitives.ts',
  entries: {
    power: {
      definitionId: 'catalog.stat.power',
      category: 'stat',
      id: 'power',
      label: 'Power',
    },
    agility: {
      definitionId: 'catalog.stat.agility',
      category: 'stat',
      id: 'agility',
      label: 'Agility',
    },
    guard: {
      definitionId: 'catalog.defense.guard',
      category: 'defense',
      id: 'guard',
      label: 'Guard',
    },
    focus: {
      definitionId: 'catalog.resource.focus',
      category: 'resource',
      id: 'focus',
      label: 'Focus',
    },
  },
});

test('action AST references close the package graph without a second ledger', () => {
  const power = preparedForStat(primitives.references.power);
  const agility = preparedForStat(primitives.references.agility);

  assert.equal(power.ok, true, JSON.stringify(power));
  assert.equal(agility.ok, true, JSON.stringify(agility));
  if (!power.ok || !agility.ok) return;
  assert.deepEqual(
    power.prepared.materializedDefinitions.map((definition) => definition.id),
    [
      'catalog.defense.guard',
      'catalog.resource.focus',
      'catalog.stat.power',
      'sample.strike',
    ],
  );
  assert.deepEqual(
    agility.prepared.materializedDefinitions.map((definition) => definition.id),
    [
      'catalog.defense.guard',
      'catalog.resource.focus',
      'catalog.stat.agility',
      'sample.strike',
    ],
  );
});

test('catalog owner identity selects one same-ID dependency deterministically', () => {
  const first = defineContentCatalog({
    packageId: 'sample.first-primitives',
    sourceModule: 'sample/first-primitives.ts',
    entries: {
      focus: {
        definitionId: 'catalog.resource.focus',
        category: 'resource',
        id: 'first-focus',
        label: 'First focus',
      },
    },
  });
  const second = defineContentCatalog({
    packageId: 'sample.second-primitives',
    sourceModule: 'sample/second-primitives.ts',
    entries: {
      focus: {
        definitionId: 'catalog.resource.focus',
        category: 'resource',
        id: 'second-focus',
        label: 'Second focus',
      },
    },
  });
  const focused = action({
    id: actionId('sample.focused'),
    name: 'Focused',
    sourcePath: 'sample/focused.ts',
    targets: { kind: 'participant', team: 'ally', maximumRange: 0, maximumTargets: 1 },
    check: noRoll(),
    costs: [spend(first.references.focus, 1)],
    program: onCheck({
      noRoll: changeResource({
        subject: 'actor',
        resource: first.references.focus,
        delta: constant(1),
      }),
    }),
  });
  const content = defineContentPack({
    identity: { id: 'sample.same-id-content', version: '1.0.0' },
    entry: { module: 'sample/content.ts', declaration: 'default' },
    dependencies: [
      contentPackDependency({
        id: 'sample.first-primitives',
        version: '1.0.0',
        importAs: 'first',
      }),
      contentPackDependency({
        id: 'sample.second-primitives',
        version: '1.0.0',
        importAs: 'second',
      }),
    ],
    definitions: [
      defineActionDefinition({
        id: focused.id,
        visibility: 'public',
        extensionPolicy: 'sealed',
        source: { module: 'sample/focused.ts', declaration: 'focused' },
        action: focused,
      }),
    ],
  });
  assert.equal(canonicalJson(focused).includes('sample.first-primitives'), false);
  const result = preparePlayBundle({
    bundle: {
      identity: { id: 'sample.same-id-bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: 'sample.same-id-content',
        version: '1.0.0',
      }),
      add: [],
      overlays: [],
      configure: {},
    },
    contentPacks: [
      contentPackSource(content),
      contentPackSource(
        defineContentPack({
          identity: { id: first.packageId, version: '1.0.0' },
          entry: { module: 'sample/first-primitives.ts', declaration: 'default' },
          definitions: first.definitions,
        }),
      ),
      contentPackSource(
        defineContentPack({
          identity: { id: second.packageId, version: '1.0.0' },
          entry: { module: 'sample/second-primitives.ts', declaration: 'default' },
          definitions: second.definitions,
        }),
      ),
    ],
  });

  assert.equal(result.ok, true, JSON.stringify(result));
  if (!result.ok) return;
  const selected = result.prepared.materializedDefinitions.find(
    (definition) => definition.id === 'catalog.resource.focus',
  );
  assert.deepEqual(selected?.semantic, {
    catalog: 'resource',
    id: 'first-focus',
  });
});

test('catalog references retain nominal category and package ownership', () => {
  const other = defineContentCatalog({
    packageId: 'sample.other',
    sourceModule: 'sample/other.ts',
    entries: {
      power: {
        definitionId: 'other.stat.power',
        category: 'stat',
        id: 'power',
        label: 'Other power',
      },
    },
  });
  const requirePrimitiveStat = (
    _reference: ContentCatalogReference<'stat', 'sample.primitives'>,
  ): void => undefined;

  requirePrimitiveStat(primitives.references.power);
  // @ts-expect-error package ownership is nominal
  requirePrimitiveStat(other.references.power);
  // @ts-expect-error a stat reference is not a defense reference
  attack({ modifier: constant(1), defense: primitives.references.power });
  // @ts-expect-error a defense reference is not a stat reference
  readStat('actor', primitives.references.guard);
});

test('bare normalized catalog IDs require an explicit low-level graph edge', () => {
  const rawFocus = unsafeNormalizedCatalogId({
    category: 'resource',
    packageId: 'sample.primitives',
    definitionId: 'catalog.resource.focus',
  });
  const rawAction = action({
    id: actionId('sample.raw-focus'),
    name: 'Raw focus',
    sourcePath: 'sample/raw-focus.ts',
    targets: { kind: 'participant', team: 'ally', maximumRange: 0, maximumTargets: 1 },
    check: noRoll(),
    program: {
      kind: 'operation',
      timing: { kind: 'immediate' },
      operation: {
        kind: 'changeResource',
        subject: 'actor',
        resourceId: rawFocus,
        delta: { kind: 'constant', value: 1 },
      },
    },
  });
  const definition = defineActionDefinition({
    id: rawAction.id,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'sample/raw-focus.ts', declaration: 'rawAction' },
    action: rawAction,
  });

  const compile = (explicit: boolean) => preparePlayBundle({
    bundle: {
      identity: { id: 'sample.raw-bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({ id: 'sample.raw-content', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    },
    contentPacks: [
      contentPackSource(defineContentPack({
        identity: { id: 'sample.raw-content', version: '1.0.0' },
        entry: { module: 'sample/raw-content.ts', declaration: 'default' },
        dependencies: [contentPackDependency({
          id: 'sample.primitives',
          version: '1.0.0',
          importAs: 'primitives',
        })],
        definitions: [
          explicit
            ? withLowLevelDefinitionReferences(definition, [
                definitionReference({
                  importAs: 'primitives',
                  definitionId: rawFocus,
                }),
              ])
            : definition,
        ],
      })),
      contentPackSource(defineContentPack({
        identity: { id: primitives.packageId, version: '1.0.0' },
        entry: { module: 'sample/primitives.ts', declaration: 'default' },
        definitions: primitives.definitions,
      })),
    ],
  });

  const rejected = compile(false);
  assert.equal(rejected.ok, false);
  if (!rejected.ok) {
    assert.ok(rejected.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_CATALOG_REFERENCE_OWNER_REQUIRED',
    ));
  }

  const accepted = compile(true);
  assert.equal(accepted.ok, true, JSON.stringify(accepted));
});

test('schema-aware action patches own paths, planes, and valid operations', () => {
  const range = actionPatch.semantic.maximumRange.adjust({ multiply: 2, add: 1 });
  const label = actionPatch.presentation.label.set('Stormfront');
  const cost = actionPatch.semantic.cost(primitives.references.focus).amount.set(2);

  assert.equal(Object.isFrozen(range), true);
  assert.deepEqual(range.operations[0], {
    kind: 'adjustNumber',
    plane: 'semantic',
    path: [
      { kind: 'field', name: 'targets' },
      { kind: 'field', name: 'maximumRange' },
    ],
    multiply: 2,
    add: 1,
  });
  assert.equal(label.operations[0]?.plane, 'presentation');
  assert.deepEqual(cost.operations[0]?.path, [
    { kind: 'field', name: 'costs' },
    { kind: 'member', key: 'resourceId', value: 'catalog.resource.focus' },
    { kind: 'field', name: 'amount' },
  ]);
  const costMember = cost.operations[0]?.path[1];
  assert.ok(costMember);
  assert.equal(Object.getOwnPropertySymbols(costMember).length, 1);
  if (false) {
    // @ts-expect-error presentation labels do not support numeric adjustment
    actionPatch.presentation.label.adjust({ add: 1 });
  }
});

function preparedForStat(
  stat: ContentCatalogReference<'stat', 'sample.primitives'>,
) {
  const strike = action({
    id: actionId('sample.strike'),
    name: 'Strike',
    sourcePath: 'sample/strike.ts',
    targets: { kind: 'participant', team: 'hostile', maximumRange: 1, maximumTargets: 1 },
    check: attack({ modifier: readStat('actor', stat), defense: primitives.references.guard }),
    rollScope: 'shared',
    costs: [spend(primitives.references.focus, 1)],
    program: onCheck({
      hit: changeResource({
        subject: 'actor',
        resource: primitives.references.focus,
        delta: constant(0),
      }),
    }),
  });
  const primitivePackage = defineContentPack({
    identity: { id: 'sample.primitives', version: '1.0.0' },
    entry: { module: 'sample/primitives.ts', declaration: 'default' },
    definitions: primitives.definitions,
  });
  const contentPackage = defineContentPack({
    identity: { id: 'sample.content', version: '1.0.0' },
    entry: { module: 'sample/content.ts', declaration: 'default' },
    dependencies: [
      contentPackDependency({
        id: 'sample.primitives',
        version: '1.0.0',
        importAs: 'primitives',
      }),
    ],
    definitions: [
      defineActionDefinition({
        id: strike.id,
        visibility: 'public',
        extensionPolicy: 'sealed',
        source: { module: 'sample/strike.ts', declaration: 'strike' },
        action: strike,
      }),
    ],
  });
  return preparePlayBundle({
    bundle: {
      identity: { id: 'sample.bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({ id: 'sample.content', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    },
    contentPacks: [
      contentPackSource(contentPackage),
      contentPackSource(primitivePackage),
    ],
  });
}
