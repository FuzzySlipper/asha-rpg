import { immutable } from "./canonical.js";
export function defineScenario(input) {
    return immutable({
        ...input,
        schema: { id: "asha.rpg.scenario", version: 2 },
    });
}
export function defineScenarioTemplate(input) {
    return immutable({
        ...input,
        schema: {
            id: "asha.rpg.scenario-template",
            version: 1,
        },
    });
}
export function instantiateScenarioTemplate(template, playBundleId) {
    return defineScenario({
        playBundleId,
        board: template.board,
        participants: template.participants,
        turn: template.turn,
        randomSource: template.randomSource,
    });
}
//# sourceMappingURL=scenario-builders.js.map