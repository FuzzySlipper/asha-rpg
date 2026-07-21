import assert from 'node:assert/strict';
import test from 'node:test';

import { inspectAuthoringBoundary } from './check-authoring-boundary.mjs';

test('accepts a pure authoring-time combinator over published operations', () => {
  const source = `
    import { damage, sequence } from '@asha-rpg/authoring';
    import type { RpgIrFormula, ContentCatalogReference } from '@asha-rpg/authoring';
    export const doubledDamage = (
      amount: RpgIrFormula,
      force: ContentCatalogReference<'damageType', 'sample.primitives'>,
    ) => sequence(damage({ amount, type: force }), damage({ amount, type: force }));
  `;

  assert.deepEqual(inspectAuthoringBoundary(source), []);
});

test('rejects legacy bare catalog constructors', () => {
  const source = `
    export const stat = statId('power');
    export const defense = defenseId('guard');
    export const resource = resourceId('focus');
    export const modifier = modifierId('slow');
    export const type = damageType('force');
  `;

  const diagnostics = inspectAuthoringBoundary(source);
  assert.equal(diagnostics.length, 5);
  assert.ok(
    diagnostics.every((entry) => entry.includes('owner-bound catalog reference')),
  );
});

test('rejects restoration of the union high-level catalog input', () => {
  const source = `
    export type ContentCatalogInput<Category> =
      ContentCatalogValue<Category> | ContentCatalogReference<Category, string>;
  `;

  const diagnostics = inspectAuthoringBoundary(source);
  assert.equal(diagnostics.length, 1);
  assert.ok(diagnostics[0]?.includes('parallel bare-ID high-level API'));
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
