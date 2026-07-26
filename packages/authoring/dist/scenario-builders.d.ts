import type { Scenario, ScenarioTemplate } from "./play-bundle-types.js";
export declare function defineScenario(input: Omit<Scenario, "schema">): Scenario;
export declare function lineOfEffectObstruction(blocks: boolean): Scenario["board"]["cells"][number]["capabilities"][number];
export declare function defineScenarioTemplate(input: Omit<ScenarioTemplate, "schema">): ScenarioTemplate;
export declare function instantiateScenarioTemplate(template: ScenarioTemplate, playBundleId: string): Scenario;
//# sourceMappingURL=scenario-builders.d.ts.map