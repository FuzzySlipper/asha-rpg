import {
  action,
  actionId,
  actionPatch,
  applyModifier,
  attack,
  changeResource,
  constant,
  damage,
  actionProcedureParameterReference,
  defineActionInvocationDefinition,
  defineActionProcedureDefinition,
  defineRuleset,
  defineScenario,
  defineContentCatalog,
  hostile,
  moveEntity,
  noRoll,
  onCheck,
  readStat,
  refresh,
  spend,
  stackingGroup,
  turns,
} from '@asha-rpg/authoring';
import { unsafeNormalizedCatalogId } from '@asha-rpg/authoring/low-level';
// @ts-expect-error Bare stat constructors are not exported by the high-level package.
import { statId as removedStatId } from '@asha-rpg/authoring';
// @ts-expect-error Bare defense constructors are not exported by the high-level package.
import { defenseId as removedDefenseId } from '@asha-rpg/authoring';
// @ts-expect-error Bare resource constructors are not exported by the high-level package.
import { resourceId as removedResourceId } from '@asha-rpg/authoring';
// @ts-expect-error Bare modifier constructors are not exported by the high-level package.
import { modifierId as removedModifierId } from '@asha-rpg/authoring';
// @ts-expect-error Bare damage-type constructors are not exported by the high-level package.
import { damageType as removedDamageType } from '@asha-rpg/authoring';

defineRuleset({
  schema: { identity: 'asha.rpg.ruleset', major: 1 },
  identity: { id: 'invalid.ruleset-content', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '1.0.0' },
  models: {
    checks: { id: 'check.d20-roll-over', version: 1 },
    turns: { id: 'turn.ordered-one-action', version: 1 },
    initiative: { id: 'initiative.scenario-ordered', version: 1 },
    reactions: { id: 'reaction.before-damage-choice', version: 1 },
    actionEconomy: { id: 'action-economy.one-action-plus-reaction', version: 1 },
  },
  provides: { operations: [], capabilities: [], values: [], numericDomains: [] },
  // @ts-expect-error Ruleset is a semantic contract and cannot contain authored definitions.
  definitions: [],
});

defineScenario({
  playBundleId: 'type-contract.bundle',
  board: { width: 1, height: 1, cells: [] },
  participants: [],
  turn: { initiativeOrder: [], currentActorId: '', round: 1, turn: 1 },
  randomSource: {
    policyId: 'random.automatic',
    policyVersion: 1,
    sourceId: 'random.system',
    sourceVersion: 1,
  },
  // @ts-expect-error Scenario is setup-only and cannot prescribe commands.
  commands: [],
});

const catalogs = defineContentCatalog({
  packageId: 'type-contracts.catalog',
  sourceModule: 'test/type-contracts.ts',
  entries: {
    guard: { definitionId: 'guard', category: 'defense', id: 'guard', label: 'Guard' },
    power: { definitionId: 'power', category: 'stat', id: 'power', label: 'Power' },
    focus: { definitionId: 'focus', category: 'resource', id: 'focus', label: 'Focus' },
    force: { definitionId: 'force', category: 'damageType', id: 'force', label: 'Force' },
    slowed: { definitionId: 'slowed', category: 'modifier', id: 'slowed', label: 'Slowed' },
  },
});
const { guard, power, focus, force, slowed } = catalogs.references;

attack({ modifier: constant(1), defense: guard });

// @ts-expect-error Stat ids cannot be passed where a defense id is required.
attack({ modifier: constant(1), defense: power });

// @ts-expect-error A defense reference cannot be read as a stat.
readStat('actor', guard);

const rawStat = unsafeNormalizedCatalogId({ category: 'stat', packageId: 'type-contracts.catalog', definitionId: 'power' });
const rawDefense = unsafeNormalizedCatalogId({ category: 'defense', packageId: 'type-contracts.catalog', definitionId: 'guard' });
const rawResource = unsafeNormalizedCatalogId({ category: 'resource', packageId: 'type-contracts.catalog', definitionId: 'focus' });
const rawDamageType = unsafeNormalizedCatalogId({ category: 'damageType', packageId: 'type-contracts.catalog', definitionId: 'force' });
const rawModifier = unsafeNormalizedCatalogId({ category: 'modifier', packageId: 'type-contracts.catalog', definitionId: 'slowed' });

// @ts-expect-error Bare normalized stat IDs are not high-level authoring references.
readStat('actor', rawStat);
// @ts-expect-error Bare normalized defense IDs are not high-level authoring references.
attack({ modifier: constant(1), defense: rawDefense });
// @ts-expect-error Bare normalized resource IDs are not high-level authoring references.
spend(rawResource, 1);
// @ts-expect-error Bare normalized damage types are not high-level authoring references.
damage({ amount: constant(1), type: rawDamageType });
// @ts-expect-error Bare normalized resource IDs cannot select a mutation target.
changeResource({ subject: 'actor', resource: rawResource, delta: constant(1) });
// @ts-expect-error Bare normalized modifier IDs are not high-level authoring references.
applyModifier({ modifier: rawModifier, value: constant(-1), duration: turns(1), stacking: refresh(stackingGroup('slow')) });
// @ts-expect-error Bare normalized resource IDs cannot select a patch member.
actionPatch.semantic.cost(rawResource);

spend(focus, 1);
damage({ amount: constant(1), type: force });
applyModifier({ modifier: slowed, value: constant(-1), duration: turns(1), stacking: refresh(stackingGroup('slow')) });

// @ts-expect-error Rolled actions require an explicit shared or per-target scope.
action({
  id: actionId('invalid.missing-scope'),
  name: 'Invalid',
  sourcePath: 'invalid/missing-scope',
  targets: hostile({ range: 1 }),
  check: attack({ modifier: constant(1), defense: guard }),
  program: onCheck({}),
});

// @ts-expect-error No-roll actions cannot claim a random roll scope.
action({
  id: actionId('invalid.no-roll-scope'),
  name: 'Invalid',
  sourcePath: 'invalid/no-roll-scope',
  targets: hostile({ range: 1 }),
  check: noRoll(),
  rollScope: 'shared',
  program: onCheck({}),
});

moveEntity({
  subject: 'target',
  deltaX: constant(1),
  deltaY: constant(0),
  // @ts-expect-error Movement bounds are numeric data.
  maximumDistance: '2',
  provokes: false,
});

const distanceParameter = {
  id: 'distance',
  type: 'boundedInteger',
  minimum: 1,
  maximum: 12,
} as const;
const movementProcedure = defineActionProcedureDefinition({
  id: 'procedure.move',
  ownerPackageId: 'type-contracts.procedures',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'test/type-contracts.ts',
    declaration: 'movementProcedure',
  },
  parameters: [distanceParameter] as const,
  implementation: {
    kind: 'inline',
    template: {
      targets: {
        kind: 'cell',
        team: 'any',
        maximumRange:
          actionProcedureParameterReference(distanceParameter),
        maximumTargets: 1,
      },
      check: { kind: 'noRoll' },
      rollScope: 'none',
      costs: [],
      program: {
        kind: 'atomic',
        body: {
          kind: 'onCheck',
          noRoll: {
            kind: 'operation',
            operation: {
              kind: 'moveToCell',
              maximumDistance:
                actionProcedureParameterReference(distanceParameter),
              provokes: false,
            },
          },
        },
      },
    },
  },
});

defineActionInvocationDefinition({
  id: 'action.move',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: { module: 'test/type-contracts.ts', declaration: 'move' },
  procedure: movementProcedure,
  arguments: { distance: 6 },
});

defineActionInvocationDefinition({
  id: 'action.invalid-move',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'test/type-contracts.ts',
    declaration: 'invalidMove',
  },
  procedure: movementProcedure,
  // @ts-expect-error Procedure arguments are derived from the parameter schema.
  arguments: { distance: 'six' },
});
