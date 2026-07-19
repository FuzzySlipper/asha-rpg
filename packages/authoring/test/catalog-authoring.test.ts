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
  defineRulesetCatalog,
  defineRulesetPackage,
  noRoll,
  onCheck,
  prepareRulesetCompilation,
  readStat,
  rulesetDependency,
  rulesetPackageRequest,
  rulesetPackageSource,
  spend,
} from '@asha-rpg/authoring';
import type { RulesetCatalogReference } from '@asha-rpg/authoring';

const primitives = defineRulesetCatalog({
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
  const first = defineRulesetCatalog({
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
  const second = defineRulesetCatalog({
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
    targets: { team: 'ally', maximumRange: 0, maximumTargets: 1 },
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
  const content = defineRulesetPackage({
    identity: { id: 'sample.same-id-content', version: '1.0.0' },
    entry: { module: 'sample/content.ts', declaration: 'default' },
    dependencies: [
      rulesetDependency({
        id: 'sample.first-primitives',
        version: '1.0.0',
        importAs: 'first',
      }),
      rulesetDependency({
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
  const result = prepareRulesetCompilation({
    composition: {
      identity: { id: 'sample.same-id-composition', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({
        id: 'sample.same-id-content',
        version: '1.0.0',
      }),
      add: [],
      overlays: [],
      configure: {},
    },
    packages: [
      rulesetPackageSource(content),
      rulesetPackageSource(
        defineRulesetPackage({
          identity: { id: first.packageId, version: '1.0.0' },
          entry: { module: 'sample/first-primitives.ts', declaration: 'default' },
          definitions: first.definitions,
        }),
      ),
      rulesetPackageSource(
        defineRulesetPackage({
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
  const other = defineRulesetCatalog({
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
    _reference: RulesetCatalogReference<'stat', 'sample.primitives'>,
  ): void => undefined;

  requirePrimitiveStat(primitives.references.power);
  // @ts-expect-error package ownership is nominal
  requirePrimitiveStat(other.references.power);
  // @ts-expect-error a stat reference is not a defense reference
  attack({ modifier: constant(1), defense: primitives.references.power });
  // @ts-expect-error a defense reference is not a stat reference
  readStat('actor', primitives.references.guard);
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
  if (false) {
    // @ts-expect-error presentation labels do not support numeric adjustment
    actionPatch.presentation.label.adjust({ add: 1 });
  }
});

function preparedForStat(
  stat: RulesetCatalogReference<'stat', 'sample.primitives'>,
) {
  const strike = action({
    id: actionId('sample.strike'),
    name: 'Strike',
    sourcePath: 'sample/strike.ts',
    targets: { team: 'hostile', maximumRange: 1, maximumTargets: 1 },
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
  const primitivePackage = defineRulesetPackage({
    identity: { id: 'sample.primitives', version: '1.0.0' },
    entry: { module: 'sample/primitives.ts', declaration: 'default' },
    definitions: primitives.definitions,
  });
  const contentPackage = defineRulesetPackage({
    identity: { id: 'sample.content', version: '1.0.0' },
    entry: { module: 'sample/content.ts', declaration: 'default' },
    dependencies: [
      rulesetDependency({
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
  return prepareRulesetCompilation({
    composition: {
      identity: { id: 'sample.composition', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.content', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    },
    packages: [
      rulesetPackageSource(contentPackage),
      rulesetPackageSource(primitivePackage),
    ],
  });
}
