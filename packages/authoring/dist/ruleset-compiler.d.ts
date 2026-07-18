import type { PrepareRulesetResult, RulesetCompilerTarget, RulesetDefinition, RulesetPackageSource } from './ruleset-types.js';
export declare const ASHA_RPG_COMPILER_TARGET: RulesetCompilerTarget;
export declare function prepareRulesetCompilation(options: {
    readonly composition: import('./ruleset-types.js').RulesetCompositionManifest;
    readonly packages: readonly RulesetPackageSource[];
    readonly target?: RulesetCompilerTarget;
}): PrepareRulesetResult;
export declare function rulesetDefinitionMaterializationFingerprint(definition: Extract<RulesetDefinition, {
    readonly kind: 'action' | 'support';
}>): string;
//# sourceMappingURL=ruleset-compiler.d.ts.map