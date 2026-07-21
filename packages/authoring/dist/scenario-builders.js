import { immutable } from './canonical.js';
export function defineScenario(input) {
    return immutable({
        ...input,
        schema: { id: 'asha.rpg.scenario', version: 1 },
    });
}
//# sourceMappingURL=scenario-builders.js.map