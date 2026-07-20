import {
  action,
  actionId,
  canonicalJson,
  composeRuleset,
  damage,
  defineActionDefinition,
  defineRulesetCatalog,
  defineRulesetPackage,
  dice,
  hostile,
  noRoll,
  onCheck,
  openReaction,
  prepareRulesetCompilation,
  reactionId,
  reactionOptionId,
  rulesetPackageRequest,
  rulesetPackageSource,
  sequence,
} from '@asha-rpg/authoring';

const catalogs = defineRulesetCatalog({
  packageId: 'portable.replay-content',
  sourceModule: 'examples/generate-portable-replay-source.ts',
  entries: {
    force: {
      definitionId: 'catalog.damage.force',
      category: 'damageType',
      id: 'force',
      label: 'Force',
    },
  },
});

const reactiveStrike = action({
  id: actionId('portable.reactive-strike'),
  name: 'Portable Reactive Strike',
  sourcePath: 'examples/generate-portable-replay-source.ts#reactiveStrike',
  targets: hostile({ range: 3 }),
  check: noRoll(),
  program: onCheck({
    noRoll: sequence(
      openReaction({
        id: reactionId('portable.ward'),
        options: [
          {
            id: reactionOptionId('ward'),
            label: 'Raise ward',
            damageReduction: 3,
          },
        ],
      }),
      damage({
        amount: dice({ count: 2, sides: 6 }),
        type: catalogs.references.force,
      }),
    ),
  }),
});

const actionDefinition = defineActionDefinition({
  kind: 'action',
  id: reactiveStrike.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'examples/generate-portable-replay-source.ts',
    declaration: 'reactiveStrike',
  },
  presentation: {
    label: reactiveStrike.name,
    description: 'Independent portable checkpoint and replay consumer source.',
    tags: ['portable', 'replay'],
  },
  action: reactiveStrike,
});

const contentPackage = defineRulesetPackage({
  identity: { id: 'portable.replay-content', version: '1.0.0' },
  entry: {
    module: 'examples/generate-portable-replay-source.ts',
    declaration: 'contentPackage',
  },
  language: { id: 'asha-rpg', version: '^1.0.0' },
  dependencies: [],
  requirements: {
    operations: [
      { id: 'operation.damage', version: 1 },
      { id: 'operation.openReaction', version: 1 },
    ],
    capabilities: [
      { id: 'capability.random', version: 1 },
      { id: 'capability.reactions', version: 1 },
      { id: 'capability.vitality', version: 1 },
    ],
  },
  definitions: [actionDefinition, ...catalogs.definitions],
  exports: [actionDefinition.id, catalogs.references.force.definitionId],
  policyBindings: [],
  relationships: [],
});

const composition = composeRuleset({
  identity: { id: 'portable.replay-consumer', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '^1.0.0' },
  base: rulesetPackageRequest({
    id: contentPackage.identity.id,
    version: contentPackage.identity.version,
  }),
  add: [],
  overlays: [],
  configure: {},
});

const prepared = prepareRulesetCompilation({
  composition,
  packages: [rulesetPackageSource(contentPackage)],
});
if (!prepared.ok) {
  throw new Error(canonicalJson(prepared.diagnostics));
}

process.stdout.write(canonicalJson(prepared.prepared));
