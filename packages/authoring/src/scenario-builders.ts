import { immutable } from './canonical.js';
import type { Scenario } from './play-bundle-types.js';

export function defineScenario(
  input: Omit<Scenario, 'schema'>,
): Scenario {
  return immutable({
    ...input,
    schema: { id: 'asha.rpg.scenario' as const, version: 1 as const },
  });
}
