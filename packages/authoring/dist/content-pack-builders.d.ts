import type { ContentCatalogReference } from './catalogs.js';
import type { RulesetValueReference } from './ruleset-builders.js';
import type { ContentActionDefinition, PlayBundleManifest, ContentDefinition, ContentDefinitionReference, ContentParticipantProfileData, ContentParticipantProfileCapability, ContentDerivedDefinition, ContentPackDependency, ContentPackIdentity, ContentMixinApplication, ContentMixinDefinition, ContentPackManifest, ContentPatch, ContentPackRequest, ContentPackSource, ContentPolicyBinding, ContentReservedRelationship, ContentSupportDefinition, ContentTemplateDefinition, ScenarioBoundedValue } from './play-bundle-types.js';
type OrdinaryDefinitionInput<Definition extends ContentDefinition> = Omit<Definition, 'kind' | 'lowLevelReferences'> & {
    readonly kind?: Definition['kind'];
};
type ContentPackInput = Omit<ContentPackManifest, 'language' | 'dependencies' | 'requirements' | 'exports' | 'policyBindings' | 'relationships'> & {
    readonly language?: ContentPackManifest['language'];
    readonly dependencies?: ContentPackManifest['dependencies'];
    readonly requirements?: Partial<ContentPackManifest['requirements']>;
    readonly exports?: ContentPackManifest['exports'];
    readonly policyBindings?: ContentPackManifest['policyBindings'];
    readonly relationships?: ContentPackManifest['relationships'];
};
export interface ContentDerivationDeclaration {
    readonly definition: ContentDerivedDefinition;
    readonly relationship: Extract<ContentReservedRelationship, {
        readonly kind: 'derivesFrom';
    }>;
}
export declare function contentPackDependency(input: Omit<ContentPackDependency, 'relationship'>): ContentPackDependency;
export declare function contentPackRequest(input: ContentPackRequest): ContentPackRequest;
export declare function definitionReference(input: ContentDefinitionReference): ContentDefinitionReference;
export declare function defineActionDefinition(input: OrdinaryDefinitionInput<ContentActionDefinition>): ContentActionDefinition;
export declare function defineSupportDefinition(input: OrdinaryDefinitionInput<ContentSupportDefinition>): ContentSupportDefinition;
export declare function defineParticipantProfileDefinition(input: Omit<OrdinaryDefinitionInput<ContentSupportDefinition>, 'semantic'> & {
    readonly profileId: string;
    readonly profile: ContentParticipantProfileData;
}): ContentSupportDefinition;
export declare function defineParticipantProfileData(input: Omit<ContentParticipantProfileData, 'schema'>): ContentParticipantProfileData;
export declare function participantProfileVitality(value: ScenarioBoundedValue): ContentParticipantProfileCapability;
export declare function participantProfileStat(reference: RulesetValueReference<'stat', string, string>, value: number): ContentParticipantProfileCapability;
export declare function participantProfileDefense(reference: RulesetValueReference<'defense', string, string>, value: number): ContentParticipantProfileCapability;
export declare function participantProfileResource(reference: ContentCatalogReference<'resource', string>, value: ScenarioBoundedValue): ContentParticipantProfileCapability;
export declare function participantProfileModifier(reference: ContentCatalogReference<'modifier', string>, input: {
    readonly stackingGroup: string;
    readonly value: number;
    readonly remainingTurns: number;
}): ContentParticipantProfileCapability;
export declare function defineTemplateDefinition(input: OrdinaryDefinitionInput<ContentTemplateDefinition>): ContentTemplateDefinition;
export declare function defineDerivedDefinition(input: OrdinaryDefinitionInput<ContentDerivedDefinition>): ContentDerivedDefinition;
export declare function defineMixinDefinition(input: OrdinaryDefinitionInput<ContentMixinDefinition>): ContentMixinDefinition;
/** Explicit escape hatch for compiler fixtures that cannot express an AST edge. */
export declare function withLowLevelDefinitionReferences<Definition extends ContentDefinition>(definition: Definition, references: readonly ContentDefinitionReference[]): Definition;
/** Low-level patch AST entrypoint. Prefer actionPatch schema builders. */
export declare function defineLowLevelContentPatch(input: ContentPatch): ContentPatch;
export declare function definePolicyBinding(input: ContentPolicyBinding): ContentPolicyBinding;
/** Low-level relationship entrypoint used when no schema builder exists. */
export declare function defineContentRelationship(input: ContentReservedRelationship): ContentReservedRelationship;
export declare function deriveAction(input: {
    readonly id: string;
    readonly visibility: ContentDerivedDefinition['visibility'];
    readonly extensionPolicy: ContentDerivedDefinition['extensionPolicy'];
    readonly source: ContentDerivedDefinition['source'];
    readonly presentation?: ContentDerivedDefinition['presentation'];
    readonly base: ContentDefinitionReference;
    readonly mixins?: readonly ContentMixinApplication[];
    readonly patch?: ContentPatch;
}): ContentDerivationDeclaration;
export declare function defineContentOverlay(input: {
    readonly definitionId: string;
    readonly target: ContentDefinitionReference;
    readonly targetPackage: ContentPackIdentity;
    readonly expectedFingerprint: string;
    readonly patch: ContentPatch;
    readonly conflictPolicy?: 'reject' | 'replace';
}): Extract<ContentReservedRelationship, {
    readonly kind: 'patches';
}>;
export declare function defineContentConfiguration(input: {
    readonly optionId: string;
    readonly target: ContentDefinitionReference;
    readonly value: string | number | boolean;
    readonly patch: ContentPatch;
}): Extract<ContentReservedRelationship, {
    readonly kind: 'configures';
}>;
export declare function defineContentPack(input: ContentPackInput): ContentPackManifest;
export declare function contentPackSource(manifest: ContentPackManifest): ContentPackSource;
export declare function composePlayBundle(input: PlayBundleManifest): PlayBundleManifest;
export {};
//# sourceMappingURL=content-pack-builders.d.ts.map