import { immutable, stableFingerprint } from './canonical.js';
export function rulesetDependency(input) {
    return immutable({ ...input, relationship: 'dependsOn' });
}
export function rulesetPackageRequest(input) {
    return immutable({ ...input });
}
export function definitionReference(input) {
    return immutable({ ...input });
}
export function defineActionDefinition(input) {
    return immutable({ ...input });
}
export function defineSupportDefinition(input) {
    return immutable({ ...input });
}
export function defineTemplateDefinition(input) {
    return immutable({ ...input });
}
export function defineDerivedDefinition(input) {
    return immutable({ ...input });
}
export function defineMixinDefinition(input) {
    return immutable({ ...input });
}
export function defineRulesetPatch(input) {
    return immutable({ ...input });
}
export function definePolicyBinding(input) {
    return immutable({ ...input });
}
export function defineRulesetRelationship(input) {
    return immutable({ ...input });
}
export function defineRulesetPackage(input) {
    return immutable({ ...input });
}
export function rulesetPackageSource(manifest) {
    return immutable({
        manifest,
        sourceFingerprint: stableFingerprint(manifest),
    });
}
export function composeRuleset(input) {
    return immutable({ ...input });
}
//# sourceMappingURL=ruleset-builders.js.map