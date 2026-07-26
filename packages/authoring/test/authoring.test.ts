import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import {
  action,
  actionId,
  always,
  canonicalRpgJson,
  cells,
  definePackage,
  diamondArea,
  dice,
  forEachTarget,
  heal,
  moveToCell,
  noRoll,
  not,
  normalizePackage,
  onCheck,
  orthogonalLineArea,
  repeat,
  sequence,
  targets,
  when,
} from '@asha-rpg/authoring';
import {
  bindingStrike,
  emberJab,
  frostJab,
  representativePackage,
  stormBurst,
  tacticalShift,
} from '../../../examples/representative-actions.ts';

test('normalization is canonical across consumer bundle order', () => {
  const first = normalizePackage(representativePackage);
  assert.equal(first.ok, true);
  if (!first.ok) return;

  const reversed = definePackage({
    id: representativePackage.id,
    version: representativePackage.version,
    sources: [...representativePackage.sources].reverse(),
  });
  const second = normalizePackage(reversed);
  assert.equal(second.ok, true);
  if (!second.ok) return;

  assert.equal(canonicalRpgJson(first.artifact), canonicalRpgJson(second.artifact));
  assert.deepEqual(
    first.artifact.actions.map((action) => action.id),
    [
      'example.binding-strike',
      'example.ember-jab',
      'example.frost-jab',
      'example.storm-burst',
      'example.tactical-shift',
    ],
  );
  assert.deepEqual(
    first.artifact.requirements
      .filter((requirement) => requirement.kind === 'operation')
      .map((requirement) => requirement.id),
    [
      'operation.applyModifier',
      'operation.damage',
      'operation.move',
    ],
  );
  assert.equal(Object.isFrozen(first.artifact), true);
  assert.equal(Object.isFrozen(first.artifact.actions), true);
});

test('helper and authoring-only timing identity disappear from normalized IR', () => {
  const result = normalizePackage(representativePackage);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  const serialized = canonicalRpgJson(result.artifact);

  assert.equal(serialized.includes('typedStrike'), false);
  assert.equal(serialized.includes('timing'), false);
  assert.equal(serialized.includes('function'), false);
  assert.equal(serialized.includes('callback'), false);
});

test('normalization diagnostics retain semantic path and authored source path', () => {
  const source = definePackage({
    id: 'invalid.package',
    version: '1.0.0',
    sources: [
      {
        kind: 'actions',
        id: 'duplicates',
        actions: [bindingStrike, bindingStrike],
      },
    ],
  });
  const result = normalizePackage(source);

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => ({
      code: diagnostic.code,
      path: diagnostic.path,
      sourcePath: diagnostic.sourcePath,
    })),
    [
      {
        code: 'normalization.duplicateActionId',
        path: '$.actions[1].id',
        sourcePath: 'examples/actions/binding-strike',
      },
    ],
  );
});

test('runtime-authored invalid roll combinations remain diagnostics instead of defaults', () => {
  const invalidAction = { ...bindingStrike, rollScope: undefined };
  const source = definePackage({
    id: 'invalid.package',
    version: '1.0.0',
    sources: [{ kind: 'actions', id: 'invalid', actions: [invalidAction] }],
  });
  const result = normalizePackage(source);

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => diagnostic.code),
    ['normalization.rollScopeInvalid'],
  );
  assert.equal(result.diagnostics[0]?.sourcePath, 'examples/actions/binding-strike');
});

test('stored executable values are rejected before artifact emission', () => {
  const withExecutableValue = {
    ...representativePackage,
    callback: () => bindingStrike,
  };
  const result = normalizePackage(withExecutableValue);

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => ({ code: diagnostic.code, path: diagnostic.path })),
    [{ code: 'normalization.executableValueForbidden', path: '$.callback' }],
  );
});

test('check-specific branch mistakes receive a local structural diagnostic', () => {
  assert.equal(bindingStrike.program.kind, 'onCheck');
  if (bindingStrike.program.kind !== 'onCheck' || bindingStrike.program.hit === undefined) return;
  const invalidAction = {
    ...bindingStrike,
    program: { kind: 'onCheck' as const, saved: bindingStrike.program.hit },
  };
  const source = definePackage({
    id: 'invalid.package',
    version: '1.0.0',
    sources: [{ kind: 'actions', id: 'invalid', actions: [invalidAction] }],
  });
  const result = normalizePackage(source);

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => diagnostic.code),
    ['normalization.checkBranchIncompatible'],
  );
});

test('cell movement shape is one unconditional evidence-free destination operation', () => {
  const movement = action({
    id: actionId('example.move'),
    name: 'Move',
    sourcePath: 'examples/actions/move',
    targets: cells({ range: 6 }),
    check: noRoll(),
    program: onCheck({
      noRoll: moveToCell({ maximumDistance: 6, provokes: true }),
    }),
  });
  const valid = normalizePackage(
    definePackage({
      id: 'movement.package',
      version: '1.0.0',
      sources: [{ kind: 'actions', id: 'movement', actions: [movement] }],
    }),
  );
  assert.equal(valid.ok, true);

  const missingMovement = {
    ...movement,
    program: bindingStrike.program,
  };
  const participantMovement = {
    ...movement,
    targets: targets({ team: 'ally', maximumRange: 6 }),
  };
  const invalid = normalizePackage(
    definePackage({
      id: 'invalid.movement.package',
      version: '1.0.0',
      sources: [
        {
          kind: 'actions',
          id: 'invalid-movement',
          actions: [missingMovement, participantMovement],
        },
      ],
    }),
  );
  assert.equal(invalid.ok, false);
  if (invalid.ok) return;
  assert.deepEqual(
    invalid.diagnostics.map((diagnostic) => diagnostic.code),
    [
      'normalization.cellProgramInvalid',
      'normalization.checkBranchIncompatible',
      'normalization.moveToCellTargetInvalid',
      'normalization.duplicateActionId',
    ],
  );

  const repeated = {
    ...movement,
    id: actionId('example.move.repeated'),
    program: onCheck({
      noRoll: repeat(
        2,
        moveToCell({ maximumDistance: 6, provokes: true }),
      ),
    }),
  };
  const conditional = {
    ...movement,
    id: actionId('example.move.conditional'),
    program: onCheck({
      noRoll: when(
        not(always()),
        moveToCell({ maximumDistance: 6, provokes: true }),
      ),
    }),
  };
  const randomComposed = {
    ...movement,
    id: actionId('example.move.random-composed'),
    program: onCheck({
      noRoll: sequence(
        moveToCell({ maximumDistance: 6, provokes: true }),
        heal({ amount: dice({ count: 1, sides: 4 }) }),
      ),
    }),
  };
  const unsupportedShapes = normalizePackage(
    definePackage({
      id: 'unsupported.movement-shapes.package',
      version: '1.0.0',
      sources: [
        {
          kind: 'actions',
          id: 'unsupported-movement-shapes',
          actions: [repeated, conditional, randomComposed],
        },
      ],
    }),
  );
  assert.equal(unsupportedShapes.ok, false);
  if (unsupportedShapes.ok) return;
  assert.deepEqual(
    unsupportedShapes.diagnostics.map((diagnostic) => diagnostic.code),
    [
      'normalization.cellProgramInvalid',
      'normalization.cellProgramInvalid',
      'normalization.cellProgramInvalid',
    ],
  );
});

test('area selector builders emit only bounded immutable declarations accepted by Rust', () => {
  const diamondTargets = diamondArea({
    range: 6,
    radius: 2,
    team: 'hostile',
    livingRequired: true,
    minimumTargets: 0,
    maximumTargets: 3,
    lineOfEffect: 'required',
  });
  const lineTargets = orthogonalLineArea({
    range: 4,
    length: 4,
    team: 'any',
    minimumTargets: 1,
    maximumTargets: 4,
    lineOfEffect: 'required',
  });
  const diamondSelector = diamondTargets.area;
  const lineSelector = lineTargets.area;
  assert.notEqual(diamondSelector, undefined);
  assert.notEqual(lineSelector, undefined);
  if (diamondSelector === undefined || lineSelector === undefined) return;
  const diamond = action({
    id: actionId('example.area-diamond'),
    name: 'Area diamond',
    sourcePath: 'examples/actions/area-diamond',
    targets: diamondTargets,
    check: noRoll(),
    program: forEachTarget(
      3,
      onCheck({ noRoll: heal({ amount: { kind: 'constant', value: 0 } }) }),
    ),
  });
  const line = action({
    id: actionId('example.area-line'),
    name: 'Area line',
    sourcePath: 'examples/actions/area-line',
    targets: lineTargets,
    check: noRoll(),
    program: forEachTarget(
      4,
      onCheck({ noRoll: heal({ amount: { kind: 'constant', value: 0 } }) }),
    ),
  });
  const result = normalizePackage(
    definePackage({
      id: 'area.package',
      version: '1.0.0',
      sources: [{ kind: 'actions', id: 'areas', actions: [line, diamond] }],
    }),
  );
  assert.equal(result.ok, true, JSON.stringify(result));
  if (!result.ok) return;
  assert.equal(Object.isFrozen(diamond.targets), true);
  assert.equal(Object.isFrozen(diamond.targets.area), true);
  assert.equal(diamond.targets.lineOfEffect, 'required');
  assert.equal(line.targets.lineOfEffect, 'required');
  assert.deepEqual(
    result.artifact.actions.map((candidate) => candidate.targets),
    [diamond.targets, line.targets],
  );

  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const validation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_ir',
    ],
    {
      cwd: root,
      encoding: 'utf8',
      input: canonicalRpgJson(result.artifact),
    },
  );
  assert.equal(validation.status, 0, validation.stderr);
  assert.equal(validation.stdout.trim(), 'accepted area.package@1.0.0 actions=2');

  const invalid = normalizePackage(
    definePackage({
      id: 'invalid.area.package',
      version: '1.0.0',
      sources: [
        {
          kind: 'actions',
          id: 'invalid-areas',
          actions: [
            {
              ...diamond,
              id: actionId('example.area-too-large'),
              targets: {
                ...diamond.targets,
                area: {
                  ...diamondSelector,
                  shape: { kind: 'diamond', radius: 12 },
                },
              },
            },
            {
              ...line,
              id: actionId('example.area-inverted-cardinality'),
              program: forEachTarget(
                1,
                onCheck({
                  noRoll: heal({
                    amount: { kind: 'constant', value: 0 },
                  }),
                }),
              ),
              targets: {
                ...line.targets,
                maximumTargets: 1,
                area: {
                  ...lineSelector,
                  minimumTargets: 2,
                },
              },
            },
            {
              ...diamond,
              id: actionId('example.area-program-too-small'),
              program: forEachTarget(
                2,
                onCheck({
                  noRoll: heal({
                    amount: { kind: 'constant', value: 0 },
                  }),
                }),
              ),
            },
          ],
        },
      ],
    }),
  );
  assert.equal(invalid.ok, false);
  if (invalid.ok) return;
  assert.deepEqual(
    invalid.diagnostics.map((diagnostic) => diagnostic.code),
    [
      'normalization.areaTargetInvalid',
      'normalization.areaTargetInvalid',
      'normalization.areaProgramBoundInvalid',
    ],
  );
});

test('normalized representative actions are accepted by the Rust compiler', () => {
  const result = normalizePackage(representativePackage);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const validation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_ir',
    ],
    {
      cwd: root,
      encoding: 'utf8',
      input: canonicalRpgJson(result.artifact),
    },
  );

  assert.equal(validation.status, 0, validation.stderr);
  assert.equal(validation.stdout.trim(), 'accepted example.rules@1.0.0 actions=5');
});

test('representative sources remain ordinary immutable authored data', () => {
  for (const action of [bindingStrike, stormBurst, tacticalShift, frostJab, emberJab]) {
    assert.equal(Object.isFrozen(action), true);
    assert.equal(typeof action.program, 'object');
  }
});
