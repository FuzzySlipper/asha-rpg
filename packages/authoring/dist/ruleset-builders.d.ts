import type { RulesetActionDefinition, RulesetCompositionManifest, RulesetDefinitionReference, RulesetDependency, RulesetPackageManifest, RulesetPackageRequest, RulesetPackageSource, RulesetPolicyBinding, RulesetReservedRelationship, RulesetSupportDefinition, RulesetTemplateDefinition } from './ruleset-types.js';
export declare function rulesetDependency(input: Omit<RulesetDependency, 'relationship'>): RulesetDependency;
export declare function rulesetPackageRequest(input: RulesetPackageRequest): RulesetPackageRequest;
export declare function definitionReference(input: RulesetDefinitionReference): RulesetDefinitionReference;
export declare function defineActionDefinition(input: RulesetActionDefinition): RulesetActionDefinition;
export declare function defineSupportDefinition(input: RulesetSupportDefinition): RulesetSupportDefinition;
export declare function defineTemplateDefinition(input: RulesetTemplateDefinition): RulesetTemplateDefinition;
export declare function definePolicyBinding(input: RulesetPolicyBinding): RulesetPolicyBinding;
export declare function defineRulesetRelationship(input: RulesetReservedRelationship): RulesetReservedRelationship;
export declare function defineRulesetPackage(input: RulesetPackageManifest): RulesetPackageManifest;
export declare function rulesetPackageSource(manifest: RulesetPackageManifest): RulesetPackageSource;
export declare function composeRuleset(input: RulesetCompositionManifest): RulesetCompositionManifest;
//# sourceMappingURL=ruleset-builders.d.ts.map