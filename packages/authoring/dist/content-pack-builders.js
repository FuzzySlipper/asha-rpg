import { immutable, stableFingerprint } from './canonical.js';
export function contentPackDependency(input) {
    return immutable({ ...input, relationship: 'dependsOn' });
}
export function contentPackRequest(input) {
    return immutable({ ...input });
}
export function definitionReference(input) {
    return immutable({ ...input });
}
export function defineActionDefinition(input) {
    return immutable({ ...input, kind: 'action' });
}
export function defineSupportDefinition(input) {
    return immutable({ ...input, kind: 'support' });
}
export function defineParticipantProfileDefinition(input) {
    const { profileId, profile, ...definition } = input;
    return immutable({
        ...definition,
        kind: 'support',
        lowLevelReferences: profile.definitionIds.map((definitionId) => ({
            definitionId,
        })),
        semantic: {
            catalog: 'participantProfile',
            id: profileId,
            data: profile,
        },
    });
}
export function defineTemplateDefinition(input) {
    return immutable({ ...input, kind: 'template' });
}
export function defineDerivedDefinition(input) {
    return immutable({ ...input, kind: 'derived' });
}
export function defineMixinDefinition(input) {
    return immutable({ ...input, kind: 'mixin' });
}
/** Explicit escape hatch for compiler fixtures that cannot express an AST edge. */
export function withLowLevelDefinitionReferences(definition, references) {
    return immutable({
        ...definition,
        lowLevelReferences: [...references],
    });
}
/** Low-level patch AST entrypoint. Prefer actionPatch schema builders. */
export function defineLowLevelContentPatch(input) {
    return immutable({ ...input });
}
export function definePolicyBinding(input) {
    return immutable({ ...input });
}
/** Low-level relationship entrypoint used when no schema builder exists. */
export function defineContentRelationship(input) {
    return immutable({ ...input });
}
export function deriveAction(input) {
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
            kind: 'derivesFrom',
            definitionId: definition.id,
            target: input.base,
            mixins: [...(input.mixins ?? [])],
            localPatch: input.patch ?? emptyPatch(),
            version: 1,
        }),
    });
}
export function defineContentOverlay(input) {
    return immutable({
        kind: 'patches',
        definitionId: input.definitionId,
        target: input.target,
        targetPackage: input.targetPackage,
        expectedFingerprint: input.expectedFingerprint,
        patch: input.patch,
        plane: patchPlane(input.patch),
        conflictPolicy: input.conflictPolicy ?? 'reject',
        version: 1,
    });
}
export function defineContentConfiguration(input) {
    return immutable({
        kind: 'configures',
        ...input,
        version: 1,
    });
}
export function defineContentPack(input) {
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
        exports: input.exports ??
            input.definitions
                .filter((definition) => definition.visibility === 'public')
                .map((definition) => definition.id),
        policyBindings: [...(input.policyBindings ?? [])],
        relationships: [...(input.relationships ?? [])],
    });
}
export function contentPackSource(manifest) {
    return immutable({
        manifest,
        sourceFingerprint: stableFingerprint(manifest),
    });
}
export function composePlayBundle(input) {
    return immutable({ ...input });
}
function emptyPatch() {
    return immutable({ version: 1, operations: [] });
}
function patchPlane(patch) {
    const planes = new Set(patch.operations.map((operation) => operation.plane));
    if (planes.size !== 1)
        return 'both';
    return planes.has('semantic') ? 'semantic' : 'presentation';
}
//# sourceMappingURL=content-pack-builders.js.map