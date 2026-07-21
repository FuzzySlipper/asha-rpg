import { immutable, stableFingerprint } from './canonical.js';
import { retainCatalogOwnership } from './catalogs.js';
import type { ContentCatalogReference } from './catalogs.js';
import { retainRulesetValueOwnership, rulesetValueId } from './ruleset-builders.js';
import type { RulesetValueReference } from './ruleset-builders.js';
import type {
  ContentActionDefinition,
  PlayBundleManifest,
  ContentDefinition,
  ContentDefinitionReference,
  ContentParticipantProfileData,
  ContentParticipantProfileCapability,
  ContentDerivedDefinition,
  ContentPackDependency,
  ContentPackIdentity,
  ContentMixinApplication,
  ContentMixinDefinition,
  ContentPackManifest,
  ContentPatch,
  ContentPackRequest,
  ContentPackSource,
  ContentPolicyBinding,
  ContentReservedRelationship,
  ContentSupportDefinition,
  ContentTemplateDefinition,
  ScenarioBoundedValue,
} from './play-bundle-types.js';

const participantProfileCapabilityBrand: unique symbol = Symbol(
  'asha-rpg.participant-profile-capability-builder',
);

type OrdinaryDefinitionInput<Definition extends ContentDefinition> = Omit<
  Definition,
  'kind' | 'lowLevelReferences'
> & {
  readonly kind?: Definition['kind'];
};

type ContentPackInput = Omit<
  ContentPackManifest,
  | 'language'
  | 'dependencies'
  | 'requirements'
  | 'exports'
  | 'policyBindings'
  | 'relationships'
> & {
  readonly language?: ContentPackManifest['language'];
  readonly dependencies?: ContentPackManifest['dependencies'];
  readonly requirements?: Partial<ContentPackManifest['requirements']>;
  readonly exports?: ContentPackManifest['exports'];
  readonly policyBindings?: ContentPackManifest['policyBindings'];
  readonly relationships?: ContentPackManifest['relationships'];
};

export interface ContentDerivationDeclaration {
  readonly definition: ContentDerivedDefinition;
  readonly relationship: Extract<
    ContentReservedRelationship,
    { readonly kind: 'derivesFrom' }
  >;
}

export function contentPackDependency(
  input: Omit<ContentPackDependency, 'relationship'>,
): ContentPackDependency {
  return immutable({ ...input, relationship: 'dependsOn' as const });
}

export function contentPackRequest(
  input: ContentPackRequest,
): ContentPackRequest {
  return immutable({ ...input });
}

export function definitionReference(
  input: ContentDefinitionReference,
): ContentDefinitionReference {
  return immutable({ ...input });
}

export function defineActionDefinition(
  input: OrdinaryDefinitionInput<ContentActionDefinition>,
): ContentActionDefinition {
  return immutable({ ...input, kind: 'action' as const });
}

export function defineSupportDefinition(
  input: OrdinaryDefinitionInput<ContentSupportDefinition>,
): ContentSupportDefinition {
  return immutable({ ...input, kind: 'support' as const });
}

export function defineParticipantProfileDefinition(
  input: Omit<
    OrdinaryDefinitionInput<ContentSupportDefinition>,
    'semantic'
  > & {
    readonly profileId: string;
    readonly profile: ContentParticipantProfileData;
  },
): ContentSupportDefinition {
  const { profileId, profile, ...definition } = input;
  return immutable({
    ...definition,
    kind: 'support' as const,
    lowLevelReferences: [...profile.definitionReferences],
    semantic: {
      catalog: 'participantProfile',
      id: profileId,
      data: profile,
    },
  });
}

export function defineParticipantProfileData(input: Omit<
  ContentParticipantProfileData,
  'schema'
>): ContentParticipantProfileData {
  return immutable({
    ...input,
    schema: {
      identity: 'asha.rpg.participant-profile' as const,
      version: 1 as const,
    },
    definitionReferences: [...input.definitionReferences],
    capabilities: [...input.capabilities],
  });
}

export function participantProfileVitality(
  value: ScenarioBoundedValue,
): ContentParticipantProfileCapability {
  return profileCapability({ owner: 'vitality' as const, value });
}

export function participantProfileStat(
  reference: RulesetValueReference<'stat', string, string>,
  value: number,
): ContentParticipantProfileCapability {
  return profileCapability(
    retainRulesetValueOwnership(
      { owner: 'stat' as const, id: rulesetValueId(reference), value },
      [{ field: 'id', reference }],
    ),
  );
}

export function participantProfileDefense(
  reference: RulesetValueReference<'defense', string, string>,
  value: number,
): ContentParticipantProfileCapability {
  return profileCapability(
    retainRulesetValueOwnership(
      { owner: 'defense' as const, id: rulesetValueId(reference), value },
      [{ field: 'id', reference }],
    ),
  );
}

export function participantProfileResource(
  reference: ContentCatalogReference<'resource', string>,
  value: ScenarioBoundedValue,
): ContentParticipantProfileCapability {
  return profileCapability(
    retainCatalogOwnership(
      { owner: 'resource' as const, id: reference.definitionId, value },
      [{ field: 'id', reference }],
    ),
  );
}

export function participantProfileModifier(
  reference: ContentCatalogReference<'modifier', string>,
  input: {
    readonly stackingGroup: string;
    readonly value: number;
    readonly remainingTurns: number;
  },
): ContentParticipantProfileCapability {
  return profileCapability(
    retainCatalogOwnership(
      {
        owner: 'modifier' as const,
        stackingGroup: input.stackingGroup,
        id: reference.definitionId,
        value: input.value,
        remainingTurns: input.remainingTurns,
      },
      [{ field: 'id', reference }],
    ),
  );
}

function profileCapability(
  capability: import('./play-bundle-types.js').ScenarioInitialCapability,
): ContentParticipantProfileCapability {
  Object.defineProperty(capability, participantProfileCapabilityBrand, {
    value: true,
    enumerable: false,
    configurable: false,
    writable: false,
  });
  return immutable(capability) as ContentParticipantProfileCapability;
}

export function defineTemplateDefinition(
  input: OrdinaryDefinitionInput<ContentTemplateDefinition>,
): ContentTemplateDefinition {
  return immutable({ ...input, kind: 'template' as const });
}

export function defineDerivedDefinition(
  input: OrdinaryDefinitionInput<ContentDerivedDefinition>,
): ContentDerivedDefinition {
  return immutable({ ...input, kind: 'derived' as const });
}

export function defineMixinDefinition(
  input: OrdinaryDefinitionInput<ContentMixinDefinition>,
): ContentMixinDefinition {
  return immutable({ ...input, kind: 'mixin' as const });
}

/** Explicit escape hatch for compiler fixtures that cannot express an AST edge. */
export function withLowLevelDefinitionReferences<Definition extends ContentDefinition>(
  definition: Definition,
  references: readonly ContentDefinitionReference[],
): Definition {
  return immutable({
    ...definition,
    lowLevelReferences: [...references],
  });
}

/** Low-level patch AST entrypoint. Prefer actionPatch schema builders. */
export function defineLowLevelContentPatch(input: ContentPatch): ContentPatch {
  return immutable({ ...input });
}

export function definePolicyBinding(
  input: ContentPolicyBinding,
): ContentPolicyBinding {
  return immutable({ ...input });
}

/** Low-level relationship entrypoint used when no schema builder exists. */
export function defineContentRelationship(
  input: ContentReservedRelationship,
): ContentReservedRelationship {
  return immutable({ ...input });
}

export function deriveAction(input: {
  readonly id: string;
  readonly visibility: ContentDerivedDefinition['visibility'];
  readonly extensionPolicy: ContentDerivedDefinition['extensionPolicy'];
  readonly source: ContentDerivedDefinition['source'];
  readonly presentation?: ContentDerivedDefinition['presentation'];
  readonly base: ContentDefinitionReference;
  readonly mixins?: readonly ContentMixinApplication[];
  readonly patch?: ContentPatch;
}): ContentDerivationDeclaration {
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

export function defineContentOverlay(input: {
  readonly definitionId: string;
  readonly target: ContentDefinitionReference;
  readonly targetPackage: ContentPackIdentity;
  readonly expectedFingerprint: string;
  readonly patch: ContentPatch;
  readonly conflictPolicy?: 'reject' | 'replace';
}): Extract<ContentReservedRelationship, { readonly kind: 'patches' }> {
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

export function defineContentConfiguration(input: {
  readonly optionId: string;
  readonly target: ContentDefinitionReference;
  readonly value: string | number | boolean;
  readonly patch: ContentPatch;
}): Extract<ContentReservedRelationship, { readonly kind: 'configures' }> {
  return immutable({
    kind: 'configures' as const,
    ...input,
    version: 1 as const,
  });
}

export function defineContentPack(
  input: ContentPackInput,
): ContentPackManifest {
  return immutable({
    ...input,
    language: input.language ?? { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [...(input.dependencies ?? [])],
    requirements: {
      operations: [...(input.requirements?.operations ?? [])],
      capabilities: [...(input.requirements?.capabilities ?? [])],
      values: [...(input.requirements?.values ?? [])],
      numericDomains: [...(input.requirements?.numericDomains ?? [])],
    },
    exports:
      input.exports ??
      input.definitions
        .filter((definition) => definition.visibility === 'public')
        .map((definition) => definition.id),
    policyBindings: [...(input.policyBindings ?? [])],
    relationships: [...(input.relationships ?? [])],
  });
}

export function contentPackSource(
  manifest: ContentPackManifest,
): ContentPackSource {
  return immutable({
    manifest,
    sourceFingerprint: stableFingerprint(manifest),
  });
}

export function composePlayBundle(
  input: PlayBundleManifest,
): PlayBundleManifest {
  return immutable({ ...input });
}

function emptyPatch(): ContentPatch {
  return immutable({ version: 1, operations: [] });
}

function patchPlane(
  patch: ContentPatch,
): 'semantic' | 'presentation' | 'both' {
  const planes = new Set(patch.operations.map((operation) => operation.plane));
  if (planes.size !== 1) return 'both';
  return planes.has('semantic') ? 'semantic' : 'presentation';
}
