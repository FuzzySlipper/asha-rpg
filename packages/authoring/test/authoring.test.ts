import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import {
  canonicalRpgJson,
  definePackage,
  normalizePackage,
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
