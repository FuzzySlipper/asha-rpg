import {
  action,
  actionId,
  add,
  ally,
  applyModifier,
  attack,
  constant,
  damage,
  damageType,
  defenseId,
  defineArchetype,
  defineItem,
  definePackage,
  defineScenario,
  dice,
  forEachTarget,
  half,
  hostile,
  immediate,
  modifierId,
  moveEntity,
  noRoll,
  onCheck,
  readStat,
  refresh,
  resourceId,
  savingThrow,
  sequence,
  spend,
  stackingGroup,
  statId,
  turns,
} from '@asha-rpg/authoring';
import type {
  AuthoredAction,
  RpgActionId,
  RpgDamageType,
  RpgModifierId,
} from '@asha-rpg/authoring';

const guard = defenseId('guard');
const resolve = defenseId('resolve');
const power = statId('power');
const focus = resourceId('focus');

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
        type: damageType('kinetic'),
        timing: immediate(),
      }),
      applyModifier({
        modifier: modifierId('example.bound'),
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
        type: damageType('storm'),
      }),
      saved: damage({
        amount: half(dice({ count: 2, sides: 6 })),
        type: damageType('storm'),
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
  type: RpgDamageType,
  modifier: RpgModifierId,
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
          stacking: refresh(stackingGroup(modifier)),
        }),
      ),
    }),
  });
}

export const frostJab = typedStrike(
  actionId('example.frost-jab'),
  damageType('cold'),
  modifierId('example.slowed'),
);
export const emberJab = typedStrike(
  actionId('example.ember-jab'),
  damageType('fire'),
  modifierId('example.singed'),
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
