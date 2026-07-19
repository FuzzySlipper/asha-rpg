import type { RulesetActionDefinition, RulesetCompositionManifest, RulesetDefinition, RulesetDefinitionReference, RulesetDerivedDefinition, RulesetDependency, RulesetIdentity, RulesetMixinApplication, RulesetMixinDefinition, RulesetPackageManifest, RulesetPatch, RulesetPackageRequest, RulesetPackageSource, RulesetPolicyBinding, RulesetReservedRelationship, RulesetSupportDefinition, RulesetTemplateDefinition } from './ruleset-types.js';
type OrdinaryDefinitionInput<Definition extends RulesetDefinition> = Omit<Definition, 'kind' | 'lowLevelReferences'> & {
    readonly kind?: Definition['kind'];
};
type RulesetPackageInput = Omit<RulesetPackageManifest, 'language' | 'dependencies' | 'requirements' | 'exports' | 'policyBindings' | 'relationships'> & {
    readonly language?: RulesetPackageManifest['language'];
    readonly dependencies?: RulesetPackageManifest['dependencies'];
    readonly requirements?: RulesetPackageManifest['requirements'];
    readonly exports?: RulesetPackageManifest['exports'];
    readonly policyBindings?: RulesetPackageManifest['policyBindings'];
    readonly relationships?: RulesetPackageManifest['relationships'];
};
export interface RulesetDerivationDeclaration {
    readonly definition: RulesetDerivedDefinition;
    readonly relationship: Extract<RulesetReservedRelationship, {
        readonly kind: 'derivesFrom';
    }>;
}
export declare function rulesetDependency(input: Omit<RulesetDependency, 'relationship'>): RulesetDependency;
export declare function rulesetPackageRequest(input: RulesetPackageRequest): RulesetPackageRequest;
export declare function definitionReference(input: RulesetDefinitionReference): RulesetDefinitionReference;
export declare function defineActionDefinition(input: OrdinaryDefinitionInput<RulesetActionDefinition>): RulesetActionDefinition;
export declare function defineSupportDefinition(input: OrdinaryDefinitionInput<RulesetSupportDefinition>): RulesetSupportDefinition;
export declare function defineTemplateDefinition(input: OrdinaryDefinitionInput<RulesetTemplateDefinition>): RulesetTemplateDefinition;
export declare function defineDerivedDefinition(input: OrdinaryDefinitionInput<RulesetDerivedDefinition>): RulesetDerivedDefinition;
export declare function defineMixinDefinition(input: OrdinaryDefinitionInput<RulesetMixinDefinition>): RulesetMixinDefinition;
/** Explicit escape hatch for compiler fixtures that cannot express an AST edge. */
export declare function withLowLevelDefinitionReferences<Definition extends RulesetDefinition>(definition: Definition, references: readonly RulesetDefinitionReference[]): Definition;
/** Low-level patch AST entrypoint. Prefer actionPatch schema builders. */
export declare function defineLowLevelRulesetPatch(input: RulesetPatch): RulesetPatch;
export declare function definePolicyBinding(input: RulesetPolicyBinding): RulesetPolicyBinding;
/** Low-level relationship entrypoint used when no schema builder exists. */
export declare function defineRulesetRelationship(input: RulesetReservedRelationship): RulesetReservedRelationship;
export declare function deriveAction(input: {
    readonly id: string;
    readonly visibility: RulesetDerivedDefinition['visibility'];
    readonly extensionPolicy: RulesetDerivedDefinition['extensionPolicy'];
    readonly source: RulesetDerivedDefinition['source'];
    readonly presentation?: RulesetDerivedDefinition['presentation'];
    readonly base: RulesetDefinitionReference;
    readonly mixins?: readonly RulesetMixinApplication[];
    readonly patch?: RulesetPatch;
}): RulesetDerivationDeclaration;
export declare function defineRulesetOverlay(input: {
    readonly definitionId: string;
    readonly target: RulesetDefinitionReference;
    readonly targetPackage: RulesetIdentity;
    readonly expectedFingerprint: string;
    readonly patch: RulesetPatch;
    readonly conflictPolicy?: 'reject' | 'replace';
}): Extract<RulesetReservedRelationship, {
    readonly kind: 'patches';
}>;
export declare function defineRulesetConfiguration(input: {
    readonly optionId: string;
    readonly target: RulesetDefinitionReference;
    readonly value: string | number | boolean;
    readonly patch: RulesetPatch;
}): Extract<RulesetReservedRelationship, {
    readonly kind: 'configures';
}>;
export declare function defineRulesetPackage(input: RulesetPackageInput): RulesetPackageManifest;
export declare function rulesetPackageSource(manifest: RulesetPackageManifest): RulesetPackageSource;
export declare function composeRuleset(input: RulesetCompositionManifest): RulesetCompositionManifest;
export {};
//# sourceMappingURL=ruleset-builders.d.ts.map