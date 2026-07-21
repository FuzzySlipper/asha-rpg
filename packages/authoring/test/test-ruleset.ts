import {
  RPG_CAPABILITY_VERSIONS,
  RPG_OPERATION_VERSIONS,
} from '@asha-rpg/ir';

import { defineRuleset } from '@asha-rpg/authoring';
import type { Ruleset } from '@asha-rpg/authoring';

/** Contract fixture only. Product content defines its own deliberately bounded Ruleset. */
export const contractTestRuleset: Ruleset = defineRuleset({
  schema: { identity: 'asha.rpg.ruleset', major: 1 },
  identity: { id: 'asha-rpg.contract-test', version: '1.0.0' },
  language: { id: 'asha-rpg', version: '1.0.0' },
  models: {
    checks: { id: 'check.d20-roll-over', version: 1 },
    turns: { id: 'turn.ordered-one-action', version: 1 },
    initiative: { id: 'initiative.scenario-ordered', version: 1 },
    reactions: { id: 'reaction.before-damage-choice', version: 1 },
    actionEconomy: { id: 'action-economy.one-action-plus-reaction', version: 1 },
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
    values: [
      { kind: 'stat', id: 'power', label: 'Power', numericDomainId: 'integer' },
      { kind: 'stat', id: 'direct-overlay-target', label: 'Direct overlay target', numericDomainId: 'integer' },
      { kind: 'stat', id: 'catalog.stat.power', label: 'Power catalog fixture', numericDomainId: 'integer' },
      { kind: 'stat', id: 'catalog.stat.agility', label: 'Agility catalog fixture', numericDomainId: 'integer' },
      { kind: 'defense', id: 'guard', label: 'Guard', numericDomainId: 'integer' },
      { kind: 'defense', id: 'resolve', label: 'Resolve', numericDomainId: 'integer' },
      { kind: 'defense', id: 'catalog.defense.guard', label: 'Guard catalog fixture', numericDomainId: 'integer' },
      { kind: 'defense', id: 'catalog.defense.resolve', label: 'Resolve catalog fixture', numericDomainId: 'integer' },
    ],
    numericDomains: [{ id: 'integer', minimum: -1_000_000, maximum: 1_000_000 }],
  },
});
