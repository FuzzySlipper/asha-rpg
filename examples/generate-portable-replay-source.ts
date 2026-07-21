import {
  action,
  actionId,
  canonicalJson,
  composePlayBundle,
  damage,
  defineActionDefinition,
  defineContentCatalog,
  defineContentPack,
  defineRuleset,
  dice,
  hostile,
  noRoll,
  onCheck,
  openReaction,
  preparePlayBundle,
  reactionId,
  reactionOptionId,
  contentPackRequest,
  contentPackSource,
  sequence,
} from '@asha-rpg/authoring';

const portableRuleset = defineRuleset({
  schema: { identity: 'asha.rpg.ruleset', major: 1 },
  identity: { id: 'portable.replay-rules', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '1.0.0' },
  models: {
    checks: { id: 'check.d20-roll-over', version: 1 },
    turns: { id: 'turn.ordered-one-action', version: 1 },
    reactions: { id: 'reaction.before-damage-choice', version: 1 },
    actionEconomy: { id: 'action-economy.one-action-plus-reaction', version: 1 },
  },
  provides: {
    operations: [
      { id: 'operation.damage', version: 1 },
      { id: 'operation.openReaction', version: 1 },
    ],
    capabilities: [
      { id: 'capability.random', version: 1 },
      { id: 'capability.reactions', version: 1 },
      { id: 'capability.vitality', version: 1 },
    ],
    values: [],
    numericDomains: [],
  },
});

const catalogs = defineContentCatalog({
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

const contentPackage = defineContentPack({
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

const playBundle = composePlayBundle({
  identity: { id: 'portable.replay-consumer', version: '1.0.0' },
  ruleset: portableRuleset,
  base: contentPackRequest({
    id: contentPackage.identity.id,
    version: contentPackage.identity.version,
  }),
  add: [],
  overlays: [],
  configure: {},
});

const prepared = preparePlayBundle({
  bundle: playBundle,
  contentPacks: [contentPackSource(contentPackage)],
});
if (!prepared.ok) {
  throw new Error(canonicalJson(prepared.diagnostics));
}

process.stdout.write(canonicalJson(prepared.prepared));
