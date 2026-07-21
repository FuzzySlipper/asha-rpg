import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  composePlayBundle,
  contentPackRequest,
  contentPackSource,
  defineContentPack,
  defineRuleset,
  defineScenario,
  preparePlayBundle,
  rulesetDefense,
  rulesetStat,
  rulesetValueId,
} from '@asha-rpg/authoring';

const semanticRuleset = defineRuleset({
  schema: { identity: 'asha.rpg.ruleset', major: 1 },
  identity: { id: 'contract.named-values', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '1.0.0' },
  models: {
    checks: { id: 'check.d20-roll-over', version: 1 },
    turns: { id: 'turn.ordered-one-action', version: 1 },
    reactions: { id: 'reaction.before-damage-choice', version: 1 },
    actionEconomy: { id: 'action-economy.one-action-plus-reaction', version: 1 },
  },
  provides: {
    operations: [],
    capabilities: [],
    values: [
      { kind: 'defense', id: 'armor-class', label: 'Armor Class', numericDomainId: 'score' },
      { kind: 'stat', id: 'strength', label: 'Strength', numericDomainId: 'score' },
    ],
    numericDomains: [{ id: 'score', minimum: 1, maximum: 30 }],
  },
});

test('Ruleset named values are owner-bound ergonomic references', () => {
  const strength = rulesetStat(semanticRuleset, 'strength');
  const armorClass = rulesetDefense(semanticRuleset, 'armor-class');

  assert.equal(rulesetValueId(strength), 'strength');
  assert.equal(rulesetValueId(armorClass), 'armor-class');
  assert.equal(strength.rulesetId, semanticRuleset.identity.id);
  assert.equal(Object.isFrozen(strength), true);
  assert.throws(() => rulesetStat(semanticRuleset, 'dexterity'));
});

test('Content Pack requirements are checked directly against Ruleset provisions', () => {
  const contentPack = defineContentPack({
    identity: { id: 'contract.incompatible-content', version: '1.0.0' },
    entry: { module: 'contract/content.ts', declaration: 'content' },
    requirements: {
      values: [{ kind: 'stat', id: 'dexterity' }],
    },
    definitions: [],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'contract.incompatible-bundle', version: '1.0.0' },
      ruleset: semanticRuleset,
      base: contentPackRequest({ id: contentPack.identity.id, version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => diagnostic.code),
    ['CONTENT_PACK_VALUE_REQUIREMENT_MISSING'],
  );
});

test('Scenario builder emits setup-only immutable data', () => {
  const scenario = defineScenario({
    playBundleId: 'contract.bundle@1.0.0:fnv1a64:test',
    board: { width: 2, height: 2, cells: [] },
    participants: [],
    turn: {
      initiativeOrder: [],
      currentActorId: '',
      round: 1,
      turn: 1,
    },
    randomSource: {
      policyId: 'random.automatic',
      policyVersion: 1,
      sourceId: 'random.system',
      sourceVersion: 1,
    },
  });

  assert.deepEqual(scenario.schema, { id: 'asha.rpg.scenario', version: 1 });
  assert.equal(Object.isFrozen(scenario.board), true);
  assert.equal('commands' in scenario, false);
  assert.equal('rolls' in scenario, false);
  assert.equal('tester' in scenario, false);
});
