import { immutable, stableFingerprint } from './canonical.js';
import type {
  RulesetActionDefinition,
  RulesetCompositionManifest,
  RulesetDefinition,
  RulesetDefinitionReference,
  RulesetDerivedDefinition,
  RulesetDependency,
  RulesetIdentity,
  RulesetMixinApplication,
  RulesetMixinDefinition,
  RulesetPackageManifest,
  RulesetPatch,
  RulesetPackageRequest,
  RulesetPackageSource,
  RulesetPolicyBinding,
  RulesetReservedRelationship,
  RulesetSupportDefinition,
  RulesetTemplateDefinition,
} from './ruleset-types.js';

type OrdinaryDefinitionInput<Definition extends RulesetDefinition> = Omit<
  Definition,
  'kind' | 'lowLevelReferences'
> & {
  readonly kind?: Definition['kind'];
};

type RulesetPackageInput = Omit<
  RulesetPackageManifest,
  | 'language'
  | 'dependencies'
  | 'requirements'
  | 'exports'
  | 'policyBindings'
  | 'relationships'
> & {
  readonly language?: RulesetPackageManifest['language'];
  readonly dependencies?: RulesetPackageManifest['dependencies'];
  readonly requirements?: RulesetPackageManifest['requirements'];
  readonly exports?: RulesetPackageManifest['exports'];
  readonly policyBindings?: RulesetPackageManifest['policyBindings'];
  readonly relationships?: RulesetPackageManifest['relationships'];
};

export interface RulesetDerivationDeclaration {
  readonly definition: RulesetDerivedDefinition;
  readonly relationship: Extract<
    RulesetReservedRelationship,
    { readonly kind: 'derivesFrom' }
  >;
}

export function rulesetDependency(
  input: Omit<RulesetDependency, 'relationship'>,
): RulesetDependency {
  return immutable({ ...input, relationship: 'dependsOn' as const });
}

export function rulesetPackageRequest(
  input: RulesetPackageRequest,
): RulesetPackageRequest {
  return immutable({ ...input });
}

export function definitionReference(
  input: RulesetDefinitionReference,
): RulesetDefinitionReference {
  return immutable({ ...input });
}

export function defineActionDefinition(
  input: OrdinaryDefinitionInput<RulesetActionDefinition>,
): RulesetActionDefinition {
  return immutable({ ...input, kind: 'action' as const });
}

export function defineSupportDefinition(
  input: OrdinaryDefinitionInput<RulesetSupportDefinition>,
): RulesetSupportDefinition {
  return immutable({ ...input, kind: 'support' as const });
}

export function defineTemplateDefinition(
  input: OrdinaryDefinitionInput<RulesetTemplateDefinition>,
): RulesetTemplateDefinition {
  return immutable({ ...input, kind: 'template' as const });
}

export function defineDerivedDefinition(
  input: OrdinaryDefinitionInput<RulesetDerivedDefinition>,
): RulesetDerivedDefinition {
  return immutable({ ...input, kind: 'derived' as const });
}

export function defineMixinDefinition(
  input: OrdinaryDefinitionInput<RulesetMixinDefinition>,
): RulesetMixinDefinition {
  return immutable({ ...input, kind: 'mixin' as const });
}

/** Explicit escape hatch for compiler fixtures that cannot express an AST edge. */
export function withLowLevelDefinitionReferences<Definition extends RulesetDefinition>(
  definition: Definition,
  references: readonly RulesetDefinitionReference[],
): Definition {
  return immutable({
    ...definition,
    lowLevelReferences: [...references],
  });
}

/** Low-level patch AST entrypoint. Prefer actionPatch schema builders. */
export function defineLowLevelRulesetPatch(input: RulesetPatch): RulesetPatch {
  return immutable({ ...input });
}

export function definePolicyBinding(
  input: RulesetPolicyBinding,
): RulesetPolicyBinding {
  return immutable({ ...input });
}

/** Low-level relationship entrypoint used when no schema builder exists. */
export function defineRulesetRelationship(
  input: RulesetReservedRelationship,
): RulesetReservedRelationship {
  return immutable({ ...input });
}

export function deriveAction(input: {
  readonly id: string;
  readonly visibility: RulesetDerivedDefinition['visibility'];
  readonly extensionPolicy: RulesetDerivedDefinition['extensionPolicy'];
  readonly source: RulesetDerivedDefinition['source'];
  readonly presentation?: RulesetDerivedDefinition['presentation'];
  readonly base: RulesetDefinitionReference;
  readonly mixins?: readonly RulesetMixinApplication[];
  readonly patch?: RulesetPatch;
}): RulesetDerivationDeclaration {
  const definition = defineDerivedDefinition({
    id: input.id,
    materializesAs: 'action',
    visibility: input.visibility,
    extensionPolicy: input.extensionPolicy,
    source: input.source,
    ...(input.presentation === undefined
      ? {}
      : { presentation: input.presentation }),
  });
  return immutable({
    definition,
    relationship: immutable({
      kind: 'derivesFrom' as const,
      definitionId: definition.id,
      target: input.base,
      mixins: [...(input.mixins ?? [])],
      localPatch: input.patch ?? emptyPatch(),
      version: 1 as const,
    }),
  });
}

export function defineRulesetOverlay(input: {
  readonly definitionId: string;
  readonly target: RulesetDefinitionReference;
  readonly targetPackage: RulesetIdentity;
  readonly expectedFingerprint: string;
  readonly patch: RulesetPatch;
  readonly conflictPolicy?: 'reject' | 'replace';
}): Extract<RulesetReservedRelationship, { readonly kind: 'patches' }> {
  return immutable({
    kind: 'patches' as const,
    definitionId: input.definitionId,
    target: input.target,
    targetPackage: input.targetPackage,
    expectedFingerprint: input.expectedFingerprint,
    patch: input.patch,
    plane: patchPlane(input.patch),
    conflictPolicy: input.conflictPolicy ?? 'reject',
    version: 1 as const,
  });
}

export function defineRulesetConfiguration(input: {
  readonly optionId: string;
  readonly target: RulesetDefinitionReference;
  readonly value: string | number | boolean;
  readonly patch: RulesetPatch;
}): Extract<RulesetReservedRelationship, { readonly kind: 'configures' }> {
  return immutable({
    kind: 'configures' as const,
    ...input,
    version: 1 as const,
  });
}

export function defineRulesetPackage(
  input: RulesetPackageInput,
): RulesetPackageManifest {
  return immutable({
    ...input,
    language: input.language ?? { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [...(input.dependencies ?? [])],
    requirements: input.requirements ?? { operations: [], capabilities: [] },
    exports:
      input.exports ??
      input.definitions
        .filter((definition) => definition.visibility === 'public')
        .map((definition) => definition.id),
    policyBindings: [...(input.policyBindings ?? [])],
    relationships: [...(input.relationships ?? [])],
  });
}

export function rulesetPackageSource(
  manifest: RulesetPackageManifest,
): RulesetPackageSource {
  return immutable({
    manifest,
    sourceFingerprint: stableFingerprint(manifest),
  });
}

export function composeRuleset(
  input: RulesetCompositionManifest,
): RulesetCompositionManifest {
  return immutable({ ...input });
}

function emptyPatch(): RulesetPatch {
  return immutable({ version: 1, operations: [] });
}

function patchPlane(
  patch: RulesetPatch,
): 'semantic' | 'presentation' | 'both' {
  const planes = new Set(patch.operations.map((operation) => operation.plane));
  if (planes.size !== 1) return 'both';
  return planes.has('semantic') ? 'semantic' : 'presentation';
}
