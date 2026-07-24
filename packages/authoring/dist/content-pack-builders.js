import { immutable, stableFingerprint } from './canonical.js';
import { retainCatalogOwnership } from './catalogs.js';
import { retainRulesetValueOwnership, rulesetValueId } from './ruleset-builders.js';
const participantProfileCapabilityBrand = Symbol('asha-rpg.participant-profile-capability-builder');
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
export function defineActionProcedureDefinition(input) {
    return immutable({
        ...input,
        kind: 'actionProcedure',
        parameters: input.parameters,
    });
}
export function defineActionInvocationDefinition(input) {
    const { procedure, importAs, arguments: invocationArguments, binding, ...definition } = input;
    return immutable({
        ...definition,
        kind: 'action',
        invocation: {
            procedure: {
                definitionId: procedure.id,
                ...(importAs === undefined ? {} : { importAs }),
            },
            procedureOwnerPackageId: procedure.ownerPackageId,
            arguments: invocationArguments,
            ...(binding === undefined
                ? {}
                : {
                    binding: {
                        ...binding,
                        requiredTags: [...binding.requiredTags].sort(),
                        requiredTraits: [...binding.requiredTraits].sort(),
                        slotIds: [...binding.slotIds].sort(),
                    },
                }),
        },
    });
}
export function actionProcedureParameterReference(parameter) {
    return immutable({
        kind: 'parameter',
        parameterId: parameter.id,
        parameterType: parameter.type,
    });
}
export function equippedItemAttribute(parameter, input) {
    return immutable({
        kind: 'equippedItemAttribute',
        bindingId: input.bindingId,
        attributeId: input.attributeId,
        parameterType: parameter.type,
    });
}
export function actionProcedureInvocation(procedure, argumentsById, importAs) {
    return immutable({
        kind: 'invocation',
        invocation: {
            procedure: {
                definitionId: procedure.id,
                ...(importAs === undefined ? {} : { importAs }),
            },
            procedureOwnerPackageId: procedure.ownerPackageId,
            arguments: argumentsById,
        },
    });
}
export function defineSupportDefinition(input) {
    return immutable({ ...input, kind: 'support' });
}
export function defineItemDefinition(input) {
    return immutable({
        ...input,
        kind: 'item',
        item: {
            ...input.item,
            schema: {
                identity: 'asha.rpg.item',
                version: 1,
            },
            tags: [...input.item.tags].sort(),
            traits: [...input.item.traits].sort(),
            allowedSlots: [...input.item.allowedSlots].sort(),
            attributes: [...input.item.attributes].sort((left, right) => left.id.localeCompare(right.id)),
        },
    });
}
export function defineCharacterFeatureDefinition(input) {
    return immutable({
        ...input,
        kind: 'characterFeature',
        characterFeature: {
            schema: {
                identity: 'asha.rpg.character-feature',
                version: 1,
            },
            rollContributions: [...input.characterFeature.rollContributions].sort((left, right) => left.id.localeCompare(right.id)),
        },
    });
}
export function defineCharacterClassDefinition(input) {
    return immutable({
        ...input,
        kind: 'characterClass',
        lowLevelReferences: [...input.characterClass.featureDefinitions],
        characterClass: {
            schema: {
                identity: 'asha.rpg.character-class',
                version: 1,
            },
            featureDefinitions: [...input.characterClass.featureDefinitions].sort((left, right) => `${left.importAs ?? ''}#${left.definitionId}`.localeCompare(`${right.importAs ?? ''}#${right.definitionId}`)),
        },
    });
}
export function itemBoundedIntegerAttribute(input) {
    return immutable({ ...input, type: 'boundedInteger' });
}
export function itemIdentifierAttribute(input) {
    return immutable({ ...input, type: 'identifier' });
}
export function itemDiceAttribute(input) {
    return immutable({
        ...input,
        type: 'dice',
        bonus: input.bonus ?? 0,
    });
}
export function itemCatalogReferenceAttribute(id, reference) {
    return immutable(retainCatalogOwnership({ id, type: 'catalogReference', value: reference }, [{ field: 'value', reference }]));
}
export function itemRulesetValueReferenceAttribute(id, reference) {
    return immutable(retainRulesetValueOwnership({ id, type: 'rulesetValueReference', value: reference }, [{ field: 'value', reference }]));
}
export function defineParticipantProfileDefinition(input) {
    const { profileId, profile, ...definition } = input;
    return immutable({
        ...definition,
        kind: 'support',
        lowLevelReferences: [
            ...profile.definitionReferences,
            ...(profile.classDefinition === null
                ? []
                : [profile.classDefinition]),
            ...profile.featureDefinitions,
            ...profile.items.map((item) => item.definition),
        ],
        semantic: {
            catalog: 'participantProfile',
            id: profileId,
            data: profile,
        },
    });
}
export function defineParticipantProfileData(input) {
    return immutable({
        ...input,
        schema: {
            identity: 'asha.rpg.participant-profile',
            version: 2,
        },
        definitionReferences: [...input.definitionReferences],
        classDefinition: input.classDefinition ?? null,
        featureDefinitions: [...(input.featureDefinitions ?? [])].sort((left, right) => `${left.importAs ?? ''}#${left.definitionId}`.localeCompare(`${right.importAs ?? ''}#${right.definitionId}`)),
        items: [...(input.items ?? [])],
        equipment: [...(input.equipment ?? [])],
        capabilities: [...input.capabilities],
    });
}
export function participantProfileVitality(value) {
    return profileCapability({ owner: 'vitality', value });
}
export function participantProfileStat(reference, value) {
    return profileCapability(retainRulesetValueOwnership({ owner: 'stat', id: rulesetValueId(reference), value }, [{ field: 'id', reference }]));
}
export function participantProfileDefense(reference, value) {
    return profileCapability(retainRulesetValueOwnership({ owner: 'defense', id: rulesetValueId(reference), value }, [{ field: 'id', reference }]));
}
export function participantProfileResource(reference, value) {
    return profileCapability(retainCatalogOwnership({ owner: 'resource', id: reference.definitionId, value }, [{ field: 'id', reference }]));
}
export function participantProfileModifier(reference, input) {
    return profileCapability(retainCatalogOwnership({
        owner: 'modifier',
        stackingGroup: input.stackingGroup,
        id: reference.definitionId,
        value: input.value,
        remainingTurns: input.remainingTurns,
    }, [{ field: 'id', reference }]));
}
function profileCapability(capability) {
    Object.defineProperty(capability, participantProfileCapabilityBrand, {
        value: true,
        enumerable: false,
        configurable: false,
        writable: false,
    });
    return immutable(capability);
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