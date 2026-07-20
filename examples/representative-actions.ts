import {
  action,
  actionId,
  add,
  ally,
  applyModifier,
  attack,
  constant,
  damage,
  defineArchetype,
  defineItem,
  definePackage,
  defineRulesetCatalog,
  defineScenario,
  dice,
  forEachTarget,
  half,
  hostile,
  immediate,
  moveEntity,
  noRoll,
  onCheck,
  readStat,
  refresh,
  savingThrow,
  sequence,
  spend,
  stackingGroup,
  turns,
} from '@asha-rpg/authoring';
import type {
  AuthoredAction,
  RpgActionId,
  RulesetCatalogReference,
} from '@asha-rpg/authoring';

const catalogs = defineRulesetCatalog({
  packageId: 'example.rules',
  sourceModule: 'examples/catalogs.ts',
  entries: {
    guard: { definitionId: 'guard', category: 'defense', id: 'guard', label: 'Guard' },
    resolve: { definitionId: 'resolve', category: 'defense', id: 'resolve', label: 'Resolve' },
    power: { definitionId: 'power', category: 'stat', id: 'power', label: 'Power' },
    focus: { definitionId: 'focus', category: 'resource', id: 'focus', label: 'Focus' },
    kinetic: { definitionId: 'kinetic', category: 'damageType', id: 'kinetic', label: 'Kinetic' },
    storm: { definitionId: 'storm', category: 'damageType', id: 'storm', label: 'Storm' },
    bound: { definitionId: 'example.bound', category: 'modifier', id: 'bound', label: 'Bound' },
    cold: { definitionId: 'cold', category: 'damageType', id: 'cold', label: 'Cold' },
    slowed: { definitionId: 'example.slowed', category: 'modifier', id: 'slowed', label: 'Slowed' },
    fire: { definitionId: 'fire', category: 'damageType', id: 'fire', label: 'Fire' },
    singed: { definitionId: 'example.singed', category: 'modifier', id: 'singed', label: 'Singed' },
  },
});

const { guard, resolve, power, focus } = catalogs.references;

export const bindingStrike = action({
  id: actionId('example.binding-strike'),
  name: 'Binding Strike',
  sourcePath: 'examples/actions/binding-strike',
  targets: hostile({ range: 2 }),
  check: attack({ modifier: readActorPower(), defense: guard }),
  rollScope: 'perTarget',
  costs: [spend(focus, 1)],
  program: onCheck({
    hit: sequence(
      damage({
        amount: dice({ count: 1, sides: 8, bonus: 2 }),
        type: catalogs.references.kinetic,
        timing: immediate(),
      }),
      applyModifier({
        modifier: catalogs.references.bound,
        value: constant(-2),
        duration: turns(2),
        stacking: refresh(stackingGroup('movement-control')),
      }),
    ),
  }),
});

export const stormBurst = action({
  id: actionId('example.storm-burst'),
  name: 'Storm Burst',
  sourcePath: 'examples/items/storm-burst',
  targets: hostile({ range: 6, maximum: 4 }),
  check: savingThrow({ difficulty: constant(14), defense: resolve }),
  rollScope: 'perTarget',
  program: forEachTarget(
    4,
    onCheck({
      failed: damage({
        amount: dice({ count: 2, sides: 6 }),
        type: catalogs.references.storm,
      }),
      saved: damage({
        amount: half(dice({ count: 2, sides: 6 })),
        type: catalogs.references.storm,
      }),
    }),
  ),
});

export const tacticalShift = action({
  id: actionId('example.tactical-shift'),
  name: 'Tactical Shift',
  sourcePath: 'examples/scenarios/tactical-shift',
  targets: ally({ range: 4 }),
  check: noRoll(),
  program: onCheck({
    noRoll: moveEntity({
      subject: 'target',
      deltaX: constant(2),
      deltaY: constant(0),
      maximumDistance: 2,
      provokes: false,
    }),
  }),
});

/** A consumer-owned helper. Its identity disappears during normalization. */
export function typedStrike(
  id: RpgActionId,
  type: RulesetCatalogReference<'damageType', 'example.rules'>,
  modifier: RulesetCatalogReference<'modifier', 'example.rules'>,
): AuthoredAction {
  return action({
    id,
    name: id,
    sourcePath: `examples/helpers/${id}`,
    targets: hostile({ range: 1 }),
    check: attack({ modifier: readActorPower(), defense: guard }),
    rollScope: 'perTarget',
    program: onCheck({
      hit: sequence(
        damage({ amount: dice({ count: 1, sides: 6, bonus: 1 }), type }),
        applyModifier({
          modifier,
          value: constant(-1),
          duration: turns(1),
          stacking: refresh(stackingGroup(modifier.definitionId)),
        }),
      ),
    }),
  });
}

export const frostJab = typedStrike(
  actionId('example.frost-jab'),
  catalogs.references.cold,
  catalogs.references.slowed,
);
export const emberJab = typedStrike(
  actionId('example.ember-jab'),
  catalogs.references.fire,
  catalogs.references.singed,
);

export const representativePackage = definePackage({
  id: 'example.rules',
  version: '1.0.0',
  sources: [
    defineArchetype('example.vanguard', [bindingStrike, frostJab, emberJab]),
    defineItem('example.storm-focus', [stormBurst]),
    defineScenario('example.bridge-crossing', [tacticalShift]),
  ],
});

function readActorPower() {
  return add(constant(1), readStat('actor', power));
}
