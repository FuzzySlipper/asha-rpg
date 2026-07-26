import { immutable } from "./canonical.js";
import type { Scenario, ScenarioTemplate } from "./play-bundle-types.js";

export function defineScenario(input: Omit<Scenario, "schema">): Scenario {
  return immutable({
    ...input,
    schema: { id: "asha.rpg.scenario" as const, version: 3 as const },
  });
}

export function lineOfEffectObstruction(
  blocks: boolean,
): Scenario["board"]["cells"][number]["capabilities"][number] {
  return immutable({
    id: "line-of-effect.obstruction",
    version: 1,
    value: {
      kind: "lineOfEffectObstruction",
      blocks,
    },
  });
}

export function defineScenarioTemplate(
  input: Omit<ScenarioTemplate, "schema">,
): ScenarioTemplate {
  return immutable({
    ...input,
    schema: {
      id: "asha.rpg.scenario-template" as const,
      version: 1 as const,
    },
  });
}

export function instantiateScenarioTemplate(
  template: ScenarioTemplate,
  playBundleId: string,
): Scenario {
  return defineScenario({
    playBundleId,
    board: template.board,
    participants: template.participants,
    turn: template.turn,
    randomSource: template.randomSource,
  });
}
