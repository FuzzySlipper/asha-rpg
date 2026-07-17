import {
  action,
  actionId,
  attack,
  constant,
  defenseId,
  hostile,
  moveEntity,
  noRoll,
  onCheck,
  statId,
} from '@asha-rpg/authoring';

const guard = defenseId('guard');
const power = statId('power');

attack({ modifier: constant(1), defense: guard });

// @ts-expect-error Stat ids cannot be passed where a defense id is required.
attack({ modifier: constant(1), defense: power });

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
