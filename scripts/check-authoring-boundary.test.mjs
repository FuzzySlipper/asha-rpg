import assert from 'node:assert/strict';
import test from 'node:test';

import { inspectAuthoringBoundary } from './check-authoring-boundary.mjs';

test('accepts a pure authoring-time combinator over published operations', () => {
  const source = `
    import { damage, sequence } from '@asha-rpg/authoring';
    import type { RpgIrFormula } from '@asha-rpg/ir';
    export const doubledDamage = (amount: RpgIrFormula) =>
      sequence(damage({ amount, type: damageType('force') }), damage({ amount, type: damageType('force') }));
  `;

  assert.deepEqual(inspectAuthoringBoundary(source), []);
});

test('rejects executable callbacks and semantic evaluation', () => {
  const source = `
    const operation = { execute: (gameplayContext) => gameplayContext.hitPoints -= rollDice(6) };
    const legal = testLegality(actor, target);
    const value = evaluateFormula(formula, authority);
  `;

  const diagnostics = inspectAuthoringBoundary(source);
  assert.ok(diagnostics.some((entry) => entry.includes('semantic callback execute')));
  assert.ok(diagnostics.some((entry) => entry.includes('rollDice')));
  assert.ok(diagnostics.some((entry) => entry.includes('testLegality')));
  assert.ok(diagnostics.some((entry) => entry.includes('evaluateFormula')));
  assert.ok(diagnostics.some((entry) => entry.includes('mutate private authority')));
});

test('rejects capability-store, browser, Angular, transport, and product access', () => {
  const source = `
    import { Component } from '@angular/core';
    import { store } from '@asha-rulebench/store';
    capabilityStore.get('vitality');
    window.fetch('/authority');
    new WebSocket('/events');
  `;

  const diagnostics = inspectAuthoringBoundary(source);
  assert.ok(diagnostics.some((entry) => entry.includes('@angular/core')));
  assert.ok(diagnostics.some((entry) => entry.includes('@asha-rulebench/store')));
  assert.ok(diagnostics.some((entry) => entry.includes('capabilityStore')));
  assert.ok(diagnostics.some((entry) => entry.includes('window')));
  assert.ok(diagnostics.some((entry) => entry.includes('WebSocket')));
});

test('rejects normalized IR that mirrors private Rust runtime layout', () => {
  const source = `
    export interface LeakedIr {
      readonly compiledProgram: readonly number[];
      readonly capabilityStore: Record<string, unknown>;
      readonly stagedState: unknown;
    }
  `;

  const diagnostics = inspectAuthoringBoundary(source, 'ir-fixture.ts', {
    normalizedIr: true,
  });
  assert.equal(diagnostics.length, 3);
  assert.ok(diagnostics.every((entry) => entry.includes('private Rust runtime')));
});
