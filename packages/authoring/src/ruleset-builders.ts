import { immutable, stableFingerprint } from './canonical.js';
import type {
  RulesetActionDefinition,
  RulesetCompositionManifest,
  RulesetDefinitionReference,
  RulesetDependency,
  RulesetPackageManifest,
  RulesetPackageRequest,
  RulesetPackageSource,
  RulesetPolicyBinding,
  RulesetReservedRelationship,
  RulesetSupportDefinition,
  RulesetTemplateDefinition,
} from './ruleset-types.js';

export function rulesetDependency(input: Omit<RulesetDependency, 'relationship'>): RulesetDependency {
  return immutable({ ...input, relationship: 'dependsOn' as const });
}

export function rulesetPackageRequest(input: RulesetPackageRequest): RulesetPackageRequest {
  return immutable({ ...input });
}

export function definitionReference(input: RulesetDefinitionReference): RulesetDefinitionReference {
  return immutable({ ...input });
}

export function defineActionDefinition(input: RulesetActionDefinition): RulesetActionDefinition {
  return immutable({ ...input });
}

export function defineSupportDefinition(input: RulesetSupportDefinition): RulesetSupportDefinition {
  return immutable({ ...input });
}

export function defineTemplateDefinition(input: RulesetTemplateDefinition): RulesetTemplateDefinition {
  return immutable({ ...input });
}

export function definePolicyBinding(input: RulesetPolicyBinding): RulesetPolicyBinding {
  return immutable({ ...input });
}

export function defineRulesetRelationship(
  input: RulesetReservedRelationship,
): RulesetReservedRelationship {
  return immutable({ ...input });
}

export function defineRulesetPackage(input: RulesetPackageManifest): RulesetPackageManifest {
  return immutable({ ...input });
}

export function rulesetPackageSource(manifest: RulesetPackageManifest): RulesetPackageSource {
  return immutable({
    manifest,
    sourceFingerprint: stableFingerprint(manifest),
  });
}

export function composeRuleset(input: RulesetCompositionManifest): RulesetCompositionManifest {
  return immutable({ ...input });
}
