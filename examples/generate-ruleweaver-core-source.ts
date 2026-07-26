import {
  action,
  actionId,
  actionProcedureParameterReference,
  activation,
  ally,
  applyEffect,
  canonicalJson,
  cells,
  composePlayBundle,
  constant,
  contentPackRequest,
  contentPackSource,
  defineActionDefinition,
  defineActionInvocationDefinition,
  defineActionProcedureDefinition,
  defineCharacterClassDefinition,
  defineCharacterFeatureDefinition,
  defineContentCatalog,
  defineContentPack,
  defineEffectDefinition,
  defineItemDefinition,
  defineRuleset,
  diamondArea,
  dice,
  equippedItemAttribute,
  forEachTarget,
  heal,
  hostile,
  itemBoundedIntegerAttribute,
  itemCatalogReferenceAttribute,
  itemDiceAttribute,
  moveToCell,
  noRoll,
  onCheck,
  onOutcome,
  preparePlayBundle,
  readStat,
  rulesetActivationBudget,
  rulesetCalculationSelector,
  rulesetContributionStackingGroup,
  rulesetDefense,
  rulesetScalarTestProfile,
  rulesetStat,
  scalarTest,
  sequence,
  spend,
} from '@asha-rpg/authoring';
import {
  RPG_CAPABILITY_VERSIONS,
  RPG_OPERATION_VERSIONS,
} from '@asha-rpg/ir';

const sourceModule = 'examples/generate-ruleweaver-core-source.ts';
const packageId = 'core-witness.content';

const ruleset = defineRuleset({
  schema: { identity: 'asha.rpg.ruleset', major: 1 },
  identity: { id: 'core-witness.rules', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '1.0.0' },
  models: {
    checks: { id: 'check.d20-roll-over', version: 1 },
    turns: { id: 'turn.ordered-one-action', version: 1 },
    initiative: { id: 'initiative.scenario-ordered', version: 1 },
    reactions: { id: 'reaction.before-damage-choice', version: 1 },
    actionEconomy: {
      id: 'action-economy.variable-activation-budgets',
      version: 1,
      acceptedActivationCeiling: 8,
    },
    lineOfEffect: {
      id: 'line-of-effect.square-grid-supercover',
      version: 1,
    },
  },
  provides: {
    operations: Object.entries(RPG_OPERATION_VERSIONS).map(([id, version]) => ({
      id,
      version,
    })),
    capabilities: Object.entries(RPG_CAPABILITY_VERSIONS).map(([id, version]) => ({
      id,
      version,
    })),
    numericDomains: [
      { id: 'budget', minimum: 0, maximum: 2 },
      { id: 'score', minimum: -100, maximum: 100 },
    ],
    values: [
      {
        kind: 'stat',
        id: 'acuity',
        label: 'Acuity',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'stat',
        id: 'conviction',
        label: 'Conviction',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'stat',
        id: 'finesse',
        label: 'Finesse',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'stat',
        id: 'intellect',
        label: 'Intellect',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'stat',
        id: 'might',
        label: 'Might',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'stat',
        id: 'spirit',
        label: 'Spirit',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'defense',
        id: 'armor',
        label: 'Armor',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'defense',
        id: 'grit',
        label: 'Grit',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'defense',
        id: 'nerve',
        label: 'Nerve',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
      {
        kind: 'defense',
        id: 'wits',
        label: 'Wits',
        numericDomainId: 'score',
        source: { kind: 'input' },
      },
    ],
    calculationSelectors: [
      {
        id: 'attack-total',
        version: 1,
        label: 'Attack total',
        numericDomainId: 'score',
      },
    ],
    contributionStackingGroups: [
      {
        id: 'typed-accuracy',
        version: 1,
        label: 'Typed accuracy',
        policy: 'greatest',
      },
      {
        id: 'untyped',
        version: 1,
        label: 'Untyped',
        policy: 'sum',
      },
    ],
    scalarTestProfiles: [
      {
        id: 'attack',
        version: 1,
        label: 'Attack',
        numericDomainId: 'score',
        dieSides: 20,
        contributionSelectorId: 'attack-total',
        bands: [
          { id: 'miss', label: 'Miss' },
          { id: 'hit', label: 'Hit' },
          { id: 'critical', label: 'Critical' },
        ],
        marginRules: [
          { minimum: null, maximum: -1, bandId: 'miss' },
          { minimum: 0, maximum: null, bandId: 'hit' },
        ],
        naturalDieRules: [
          {
            id: 'natural-20',
            minimum: 20,
            maximum: 20,
            effect: { kind: 'setBand', bandId: 'critical' },
          },
        ],
      },
    ],
    activationBudgets: [
      {
        id: 'bonus',
        version: 1,
        label: 'Bonus',
        numericDomainId: 'budget',
        timing: 'action',
        resetBoundary: 'ownerTurnStart',
        initialAmount: 1,
      },
      {
        id: 'reaction',
        version: 1,
        label: 'Reaction',
        numericDomainId: 'budget',
        timing: 'reaction',
        resetBoundary: 'ownerTurnStart',
        initialAmount: 1,
      },
      {
        id: 'standard',
        version: 1,
        label: 'Standard',
        numericDomainId: 'budget',
        timing: 'action',
        resetBoundary: 'ownerTurnStart',
        initialAmount: 1,
      },
    ],
  },
});

const catalogs = defineContentCatalog({
  packageId,
  sourceModule,
  entries: {
    focus: {
      definitionId: 'resource.focus',
      category: 'resource',
      id: 'focus',
      label: 'Focus',
    },
    impact: {
      definitionId: 'damage.impact',
      category: 'damageType',
      id: 'impact',
      label: 'Impact',
    },
  },
});

const attackSelector = rulesetCalculationSelector(ruleset, 'attack-total');
const typedAccuracy = rulesetContributionStackingGroup(
  ruleset,
  'typed-accuracy',
);
const untyped = rulesetContributionStackingGroup(ruleset, 'untyped');
const attackProfile = rulesetScalarTestProfile(ruleset, 'attack');
const standardBudget = rulesetActivationBudget(ruleset, 'standard');
const bonusBudget = rulesetActivationBudget(ruleset, 'bonus');

const exposedTarget = defineEffectDefinition({
  id: 'effect.exposed',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'exposedTarget' },
  presentation: { label: 'Exposed' },
  effect: {
    rankMinimum: 1,
    rankMaximum: 1,
    stackingId: 'exposed',
    stacking: 'refresh',
    tenure: {
      kind: 'fixed',
      anchor: 'targetTurnStart',
      count: 2,
    },
    contributions: [
      {
        id: 'exposed-opening',
        subject: 'target',
        selector: attackSelector,
        stackingGroup: untyped,
        value: { kind: 'constant', value: 1 },
        predicate: {
          kind: 'effectActive',
          subject: 'target',
          definition: { definitionId: 'effect.exposed' },
        },
      },
    ],
  },
});

const saveEndsRestricted = defineEffectDefinition({
  id: 'effect.save-ends-restricted',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'saveEndsRestricted' },
  presentation: { label: 'Restricted until save' },
  effect: {
    rankMinimum: 1,
    rankMaximum: 1,
    stackingId: 'save-ends-restricted',
    stacking: 'refresh',
    tenure: { kind: 'targetTurnEndSave' },
    condition: {
      clauses: [
        { kind: 'forbidMovement' },
        { kind: 'forbidActionTag', actionTag: 'restricted' },
      ],
    },
    contributions: [
      {
        id: 'actor-pressure',
        subject: 'actor',
        selector: attackSelector,
        stackingGroup: untyped,
        value: { kind: 'constant', value: 3 },
        predicate: { kind: 'always' },
      },
      {
        id: 'target-opening',
        subject: 'target',
        selector: attackSelector,
        stackingGroup: untyped,
        value: { kind: 'constant', value: 2 },
        predicate: { kind: 'always' },
      },
    ],
  },
});

const saveEndsAuxiliary = defineEffectDefinition({
  id: 'effect.save-ends-auxiliary',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'saveEndsAuxiliary' },
  presentation: { label: 'Auxiliary save' },
  effect: {
    rankMinimum: 1,
    rankMaximum: 1,
    stackingId: 'save-ends-auxiliary',
    stacking: 'refresh',
    tenure: { kind: 'targetTurnEndSave' },
    contributions: [
      {
        id: 'auxiliary-opening',
        subject: 'target',
        selector: attackSelector,
        stackingGroup: untyped,
        value: { kind: 'constant', value: 1 },
        predicate: { kind: 'always' },
      },
    ],
  },
});

const tacticalTraining = defineCharacterFeatureDefinition({
  id: 'feature.tactical-training',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'tacticalTraining' },
  presentation: { label: 'Tactical Training' },
  characterFeature: {
    contributions: [
      {
        id: 'training',
        selector: attackSelector,
        stackingGroup: typedAccuracy,
        value: { kind: 'constant', value: 2 },
        predicate: { kind: 'always' },
      },
      {
        id: 'surrounded-resolve',
        selector: attackSelector,
        stackingGroup: untyped,
        value: { kind: 'constant', value: 5 },
        predicate: { kind: 'actorSurrounded', minimumHostiles: 3 },
      },
    ],
  },
});
const vanguardClass = defineCharacterClassDefinition({
  id: 'class.vanguard',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'vanguardClass' },
  presentation: { label: 'Vanguard' },
  characterClass: {
    featureDefinitions: [{ definitionId: tacticalTraining.id }],
  },
});

const rangeParameter = {
  id: 'range',
  type: 'boundedInteger',
  minimum: 1,
  maximum: 8,
} as const;
const damageParameter = { id: 'damage', type: 'formula' } as const;
const damageTypeParameter = {
  id: 'damage-type',
  type: 'catalogReference',
} as const;
const costsParameter = { id: 'costs', type: 'costs' } as const;
const damageReference = actionProcedureParameterReference(damageParameter);
const damageTypeReference =
  actionProcedureParameterReference(damageTypeParameter);

const coreAttackProcedure = defineActionProcedureDefinition({
  id: 'procedure.core-attack',
  ownerPackageId: packageId,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'coreAttackProcedure' },
  presentation: { label: 'Core attack procedure' },
  parameters: [
    costsParameter,
    damageParameter,
    damageTypeParameter,
    rangeParameter,
  ] as const,
  implementation: {
    kind: 'inline',
    template: {
      targets: {
        kind: 'participant',
        team: 'hostile',
        maximumRange: actionProcedureParameterReference(rangeParameter),
        maximumTargets: 2,
        lineOfEffect: 'required',
      },
      check: scalarTest({
        profile: attackProfile,
        base: readStat('actor', rulesetStat(ruleset, 'might')),
        difficulty: {
          kind: 'targetDefense',
          defense: rulesetDefense(ruleset, 'armor'),
        },
      }),
      rollScope: 'perTarget',
      costs: actionProcedureParameterReference(costsParameter),
      activation: activation({
        timing: 'action',
        costs: [{ budget: standardBudget, amount: 1 }],
      }),
      program: {
        kind: 'atomic',
        body: {
          kind: 'forEachTarget',
          maximum: 2,
          body: {
            kind: 'onOutcome',
            branches: {
              critical: {
                kind: 'sequence',
                steps: [
                  {
                    kind: 'operation',
                    operation: {
                      kind: 'damage',
                      parts: [
                        {
                          id: 'primary',
                          amount: damageReference,
                          damageType: damageTypeReference,
                          tags: ['item-bound'],
                        },
                      ],
                    },
                  },
                  {
                    kind: 'operation',
                    operation: {
                      kind: 'damage',
                      parts: [
                        {
                          id: 'follow-on',
                          amount: damageReference,
                          damageType: damageTypeReference,
                          tags: ['critical-follow-on'],
                        },
                      ],
                    },
                  },
                ],
              },
              hit: {
                kind: 'operation',
                operation: {
                  kind: 'damage',
                  parts: [
                    {
                      id: 'primary',
                      amount: damageReference,
                      damageType: damageTypeReference,
                      tags: ['item-bound'],
                    },
                  ],
                },
              },
            },
            default: {
              kind: 'operation',
              operation: {
                kind: 'damage',
                parts: [
                  {
                    id: 'miss',
                    amount: { kind: 'constant', value: 1 },
                    damageType: damageTypeReference,
                    tags: ['miss'],
                  },
                ],
              },
            },
          },
        },
      },
    },
  },
});

const weaponBinding = {
  id: 'weapon',
  requiredTags: ['weapon'],
  requiredTraits: ['melee'],
  slotIds: ['hand.main', 'hand.off'],
} as const;

const coreAttack = defineActionInvocationDefinition({
  id: 'action.core-attack',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'coreAttack' },
  presentation: { label: 'Core Attack' },
  tags: ['attack', 'item-bound'],
  procedure: coreAttackProcedure,
  binding: weaponBinding,
  arguments: {
    costs: [spend(catalogs.references.focus, 2)],
    damage: equippedItemAttribute(damageParameter, {
      bindingId: weaponBinding.id,
      attributeId: 'damage',
    }),
    'damage-type': equippedItemAttribute(damageTypeParameter, {
      bindingId: weaponBinding.id,
      attributeId: 'damage-type',
    }),
    range: equippedItemAttribute(rangeParameter, {
      bindingId: weaponBinding.id,
      attributeId: 'range',
    }),
  },
});

function weapon(options: {
  readonly id: string;
  readonly label: string;
  readonly damageCount: number;
  readonly damageSides: number;
  readonly range: number;
  readonly accuracy: number;
}) {
  return defineItemDefinition({
    id: options.id,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: sourceModule, declaration: options.id.replaceAll('.', '_') },
    presentation: { label: options.label },
    item: {
      tags: ['weapon'],
      traits: ['melee'],
      allowedSlots: ['hand.main', 'hand.off'],
      attributes: [
        itemDiceAttribute({
          id: 'damage',
          count: options.damageCount,
          sides: options.damageSides,
        }),
        itemCatalogReferenceAttribute(
          'damage-type',
          catalogs.references.impact,
        ),
        itemBoundedIntegerAttribute({
          id: 'range',
          value: options.range,
          minimum: 1,
          maximum: 8,
        }),
      ],
      contributions: [
        {
          id: 'weapon-accuracy',
          selector: attackSelector,
          stackingGroup: typedAccuracy,
          value: { kind: 'constant', value: options.accuracy },
          predicate: {
            kind: 'boundItemDefinition',
            definition: { definitionId: options.id },
          },
        },
      ],
    },
  });
}

const shortBlade = weapon({
  id: 'item.short-blade',
  label: 'Short Blade',
  damageCount: 1,
  damageSides: 6,
  range: 2,
  accuracy: 1,
});
const longSpear = weapon({
  id: 'item.long-spear',
  label: 'Long Spear',
  damageCount: 1,
  damageSides: 8,
  range: 4,
  accuracy: 1,
});

const exposeAction = action({
  id: actionId('action.expose'),
  name: 'Expose',
  sourcePath: `${sourceModule}#exposeAction`,
  targets: hostile({ range: 4, lineOfEffect: 'required' }),
  check: noRoll(),
  activation: activation({
    timing: 'action',
    costs: [{ budget: bonusBudget, amount: 1 }],
  }),
  program: onCheck({
    noRoll: applyEffect({
      effect: { definitionId: exposedTarget.id },
      rank: constant(1),
    }),
  }),
});
const exposeDefinition = defineActionDefinition({
  id: exposeAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'exposeAction' },
  presentation: { label: exposeAction.name },
  action: exposeAction,
});

const applyConditionAction = action({
  id: actionId('action.apply-condition'),
  name: 'Apply condition',
  sourcePath: `${sourceModule}#applyConditionAction`,
  targets: hostile({ range: 4, lineOfEffect: 'required' }),
  check: noRoll(),
  activation: activation({
    timing: 'action',
    costs: [{ budget: standardBudget, amount: 1 }],
  }),
  program: onCheck({
    noRoll: sequence(
      applyEffect({
        effect: { definitionId: saveEndsAuxiliary.id },
        rank: constant(1),
      }),
      applyEffect({
        effect: { definitionId: saveEndsRestricted.id },
        rank: constant(1),
      }),
    ),
  }),
});
const applyConditionDefinition = defineActionDefinition({
  id: applyConditionAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'applyConditionAction' },
  presentation: { label: applyConditionAction.name },
  action: applyConditionAction,
});

const conditionProbeAction = action({
  id: actionId('action.condition-probe'),
  name: 'Condition probe',
  sourcePath: `${sourceModule}#conditionProbeAction`,
  tags: ['probe'],
  targets: hostile({ range: 4, lineOfEffect: 'required' }),
  check: scalarTest({
    profile: attackProfile,
    base: readStat('actor', rulesetStat(ruleset, 'might')),
    difficulty: {
      kind: 'targetDefense',
      defense: rulesetDefense(ruleset, 'armor'),
    },
  }),
  rollScope: 'perTarget',
  activation: activation({ timing: 'action', costs: [] }),
  program: onOutcome({
    branches: {
      hit: heal({ amount: constant(0) }),
    },
    default: heal({ amount: constant(0) }),
  }),
});
const conditionProbeDefinition = defineActionDefinition({
  id: conditionProbeAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'conditionProbeAction' },
  presentation: { label: conditionProbeAction.name },
  action: conditionProbeAction,
});

const restrictedAction = action({
  id: actionId('action.restricted'),
  name: 'Restricted action',
  sourcePath: `${sourceModule}#restrictedAction`,
  tags: ['restricted'],
  targets: hostile({ range: 4, lineOfEffect: 'required' }),
  check: noRoll(),
  activation: activation({ timing: 'action', costs: [] }),
  program: onCheck({
    noRoll: heal({ amount: constant(0) }),
  }),
});
const restrictedDefinition = defineActionDefinition({
  id: restrictedAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'restrictedAction' },
  presentation: { label: restrictedAction.name },
  action: restrictedAction,
});

const burstAction = action({
  id: actionId('action.burst'),
  name: 'Burst',
  sourcePath: `${sourceModule}#burstAction`,
  targets: diamondArea({
    range: 0,
    radius: 2,
    team: 'hostile',
    maximumTargets: 4,
    lineOfEffect: 'required',
  }),
  check: noRoll(),
  activation: activation({
    timing: 'action',
    costs: [{ budget: standardBudget, amount: 1 }],
  }),
  program: forEachTarget(
    4,
    applyEffect({
      effect: { definitionId: exposedTarget.id },
      rank: constant(1),
    }),
  ),
});
const burstDefinition = defineActionDefinition({
  id: burstAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'burstAction' },
  presentation: { label: burstAction.name },
  action: burstAction,
});

const shiftAction = action({
  id: actionId('action.shift'),
  name: 'Shift',
  sourcePath: `${sourceModule}#shiftAction`,
  targets: cells({ range: 5, lineOfEffect: 'required' }),
  check: noRoll(),
  activation: activation({
    timing: 'action',
    costs: [{ budget: standardBudget, amount: 1 }],
  }),
  program: onCheck({
    noRoll: moveToCell({ maximumDistance: 5, provokes: false }),
  }),
});
const shiftDefinition = defineActionDefinition({
  id: shiftAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'shiftAction' },
  presentation: { label: shiftAction.name },
  action: shiftAction,
});

const rallyAction = action({
  id: actionId('action.rally'),
  name: 'Rally',
  sourcePath: `${sourceModule}#rallyAction`,
  targets: ally({ range: 0 }),
  check: noRoll(),
  costs: [spend(catalogs.references.focus, 1)],
  activation: activation({
    timing: 'action',
    costs: [{ budget: bonusBudget, amount: 1 }],
  }),
  program: onCheck({
    noRoll: heal({ amount: dice({ count: 1, sides: 4 }) }),
  }),
});
const rallyDefinition = defineActionDefinition({
  id: rallyAction.id,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: sourceModule, declaration: 'rallyAction' },
  presentation: { label: rallyAction.name },
  action: rallyAction,
});

const contentPack = defineContentPack({
  identity: { id: packageId, version: '1.0.0' },
  entry: { module: sourceModule, declaration: 'contentPack' },
  requirements: {
    operations: [
      { id: 'operation.applyEffect', version: 1 },
      { id: 'operation.damage', version: 2 },
      { id: 'operation.heal', version: 1 },
    ],
    capabilities: [
      { id: 'capability.activation-budgets', version: 1 },
      { id: 'capability.defenses', version: 1 },
      { id: 'capability.effects', version: 1 },
      { id: 'capability.position', version: 1 },
      { id: 'capability.random', version: 1 },
      { id: 'capability.resources', version: 1 },
      { id: 'capability.stats', version: 1 },
      { id: 'capability.vitality', version: 1 },
    ],
    numericDomains: ['budget', 'score'],
  },
  definitions: [
    ...catalogs.definitions,
    coreAttackProcedure,
    coreAttack,
    applyConditionDefinition,
    burstDefinition,
    conditionProbeDefinition,
    exposeDefinition,
    restrictedDefinition,
    shiftDefinition,
    rallyDefinition,
    exposedTarget,
    saveEndsAuxiliary,
    saveEndsRestricted,
    vanguardClass,
    tacticalTraining,
    shortBlade,
    longSpear,
  ],
  exports: [
    ...catalogs.definitions.map((definition) => definition.id),
    coreAttack.id,
    applyConditionDefinition.id,
    burstDefinition.id,
    conditionProbeDefinition.id,
    exposeDefinition.id,
    restrictedDefinition.id,
    shiftDefinition.id,
    rallyDefinition.id,
    exposedTarget.id,
    saveEndsAuxiliary.id,
    saveEndsRestricted.id,
    vanguardClass.id,
    tacticalTraining.id,
    shortBlade.id,
    longSpear.id,
  ],
});

const playBundle = composePlayBundle({
  identity: { id: 'core-witness.bundle', version: '1.0.0' },
  ruleset,
  base: contentPackRequest({
    id: contentPack.identity.id,
    version: contentPack.identity.version,
  }),
  add: [],
  overlays: [],
  configure: {},
});

const prepared = preparePlayBundle({
  bundle: playBundle,
  contentPacks: [contentPackSource(contentPack)],
});
if (!prepared.ok) {
  throw new Error(canonicalJson(prepared.diagnostics));
}

process.stdout.write(canonicalJson(prepared.prepared));
