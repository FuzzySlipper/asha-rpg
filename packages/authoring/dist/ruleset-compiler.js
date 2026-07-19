import { RPG_CAPABILITY_VERSIONS, RPG_OPERATION_VERSIONS, } from '@asha-rpg/ir';
import { canonicalJson, immutable, stableFingerprint } from './canonical.js';
import { defineActions, definePackage } from './builders.js';
import { normalizeAction, normalizePackage } from './normalize.js';
const NO_DIAGNOSTICS = Object.freeze([]);
export const ASHA_RPG_COMPILER_TARGET = immutable({
    language: { id: 'asha-rpg', version: '1.0.0' },
    operations: { ...RPG_OPERATION_VERSIONS },
    capabilities: { ...RPG_CAPABILITY_VERSIONS },
});
export function prepareRulesetCompilation(options) {
    const target = options.target ?? ASHA_RPG_COMPILER_TARGET;
    const diagnostics = [];
    rejectExecutableValues(options, '$', diagnostics, new WeakSet());
    validateComposition(options.composition, target, diagnostics);
    validateUniquePackageSources(options.packages, diagnostics);
    const context = {
        diagnostics,
        availableById: indexAvailablePackages(options.packages, diagnostics),
        selected: new Map(),
        lock: [],
        relationships: [],
    };
    const compositionKey = identityKey(options.composition.identity.id, options.composition.identity.version);
    const resolved = resolvePackageGraph(context, options.composition, compositionKey);
    if (resolved === undefined)
        return failed(diagnostics);
    const { base, additions } = resolved;
    validateSelectedPackages(context, target);
    const rootKeys = new Set([base, ...additions]
        .filter((entry) => entry !== undefined)
        .map((entry) => entry.key));
    const materialization = materializeSelectedDefinitions(context, options.composition, resolved.overlays);
    if (materialization === undefined)
        return failed(diagnostics);
    const graph = closeDefinitionGraph(context, rootKeys, materialization.records);
    if (diagnostics.length > 0 || graph === undefined)
        return failed(diagnostics);
    const normalized = normalizeMaterializedActions(options.composition, graph, diagnostics);
    if (normalized === undefined)
        return failed(diagnostics);
    const requirements = collectRequirements(context, normalized.requirements, target);
    const policyBindings = [...context.selected.values()]
        .flatMap((entry) => entry.source.manifest.policyBindings)
        .sort((left, right) => left.id.localeCompare(right.id));
    rejectDuplicateValues(policyBindings.map((binding) => binding.id), 'RULESET_DUPLICATE_POLICY_BINDING', '$.compiledPolicyBindings', 'policy binding', diagnostics);
    if (diagnostics.length > 0 || requirements === undefined)
        return failed(diagnostics);
    const definitionProvenance = graph.materialized
        .map((record) => provenance(record))
        .sort((left, right) => left.definitionId.localeCompare(right.definitionId));
    const materializedDefinitions = materializeDefinitions(graph.materialized, graph.resolvedReferences, graph.exportedRoots, normalized.actions);
    const relationships = [
        ...context.relationships,
        ...materialization.relationships,
        ...graph.exportedRoots.map((definitionId, order) => ({
            kind: 'exports',
            source: graph.byGlobalId.get(definitionId)?.package.key ?? compositionKey,
            target: definitionId,
            order,
        })),
        ...Object.entries(options.composition.configure)
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([optionId, value], order) => ({
            kind: 'configures',
            source: compositionKey,
            target: `${optionId}=${String(value)}`,
            order,
        })),
    ].sort(compareRelationship);
    const prepared = immutable({
        schema: { identity: 'asha.rpg.ruleset.prepared', major: 1 },
        compositionIdentity: options.composition.identity,
        languageIdentity: target.language,
        sourcePackages: [...context.selected.values()]
            .map((entry) => ({
            id: entry.source.manifest.identity.id,
            version: entry.source.manifest.identity.version,
            sourceFingerprint: entry.source.sourceFingerprint,
        }))
            .sort(compareIdentity),
        dependencyLock: [...context.lock].sort(compareLock),
        requiredOperations: requirements.operations,
        requiredCapabilities: requirements.capabilities,
        exportedRoots: graph.exportedRoots,
        materializedDefinitions,
        compiledPolicyBindings: policyBindings,
        definitionProvenance,
        relationships,
        derivationProvenance: materialization.derivationProvenance,
        overlayProvenance: materialization.overlayProvenance,
    });
    return immutable({ ok: true, prepared, diagnostics: NO_DIAGNOSTICS });
}
function validateComposition(composition, target, diagnostics) {
    requireIdentifier(composition.identity.id, '$.composition.identity.id', diagnostics);
    requireExactVersion(composition.identity.version, '$.composition.identity.version', diagnostics);
    if (composition.language.id !== 'asha-rpg') {
        diagnostics.push(diagnostic('compatibility', 'RULESET_LANGUAGE_ID_UNSUPPORTED', '$.composition.language.id', 'the composition language must be asha-rpg', { expected: 'asha-rpg', actual: composition.language.id }));
    }
    if (composition.language.id !== target.language.id ||
        !satisfiesVersion(target.language.version, composition.language.version)) {
        diagnostics.push(diagnostic('compatibility', 'RULESET_COMPOSITION_LANGUAGE_INCOMPATIBLE', '$.composition.language', `the composition requires ${composition.language.id}@${composition.language.version}`, {
            expected: `${target.language.id}@${target.language.version}`,
            actual: `${composition.language.id}@${composition.language.version}`,
        }));
    }
    for (const optionId of Object.keys(composition.configure).sort()) {
        requireIdentifier(optionId, `$.composition.configure.${optionId}`, diagnostics);
    }
}
function indexAvailablePackages(sources, diagnostics) {
    const byId = new Map();
    for (const [index, source] of sources.entries()) {
        const path = `$.packages[${index}]`;
        const manifest = source.manifest;
        requireIdentifier(manifest.identity.id, `${path}.manifest.identity.id`, diagnostics);
        requireExactVersion(manifest.identity.version, `${path}.manifest.identity.version`, diagnostics);
        requireText(source.sourceFingerprint, `${path}.sourceFingerprint`, 'source fingerprint', diagnostics);
        const versions = byId.get(manifest.identity.id) ?? [];
        versions.push(source);
        byId.set(manifest.identity.id, versions);
    }
    for (const versions of byId.values()) {
        versions.sort((left, right) => compareVersion(right.manifest.identity.version, left.manifest.identity.version));
    }
    return byId;
}
function validateUniquePackageSources(sources, diagnostics) {
    const identities = new Set();
    for (const [index, source] of sources.entries()) {
        const identity = source.manifest.identity;
        const key = identityKey(identity.id, identity.version);
        if (!identities.has(key)) {
            identities.add(key);
            continue;
        }
        diagnostics.push(diagnostic('source', 'RULESET_DUPLICATE_PACKAGE_IDENTITY', `$.packages[${index}]`, `duplicate package source ${key}`, { packageId: identity.id }));
    }
}
function resolvePackageGraph(context, composition, compositionKey) {
    const rootConstraints = [
        {
            request: composition.base,
            requester: compositionKey,
            importAs: 'base',
            relationship: 'contributes',
            path: '$.composition.base',
        },
        ...composition.add.map((request, index) => ({
            request,
            requester: compositionKey,
            importAs: `add:${request.id}`,
            relationship: 'contributes',
            path: `$.composition.add[${index}]`,
        })),
        ...composition.overlays.map((request, index) => ({
            request,
            requester: compositionKey,
            importAs: `overlay:${request.id}`,
            relationship: 'patches',
            path: `$.composition.overlays[${index}]`,
        })),
    ];
    for (const constraint of rootConstraints) {
        validateSupportedRange(constraint, context.diagnostics);
    }
    if (context.diagnostics.length > 0)
        return undefined;
    let failedConstraints = rootConstraints;
    const search = (selectedById) => {
        const constraints = collectPackageConstraints(rootConstraints, selectedById);
        for (const constraint of constraints) {
            if (!supportedRange(constraint.request.version)) {
                failedConstraints = [constraint];
                return undefined;
            }
        }
        const constraintsById = groupConstraints(constraints);
        for (const [packageId, selected] of selectedById) {
            const compatible = (constraintsById.get(packageId) ?? []).every((constraint) => satisfiesVersion(selected.manifest.identity.version, constraint.request.version));
            if (!compatible) {
                failedConstraints = constraintsById.get(packageId) ?? [];
                return undefined;
            }
        }
        const unresolvedId = [...constraintsById.keys()]
            .filter((packageId) => !selectedById.has(packageId))
            .sort(compareText)[0];
        if (unresolvedId === undefined)
            return selectedById;
        const packageConstraints = constraintsById.get(unresolvedId) ?? [];
        const candidates = (context.availableById.get(unresolvedId) ?? []).filter((source) => packageConstraints.every((constraint) => satisfiesVersion(source.manifest.identity.version, constraint.request.version)));
        if (candidates.length === 0) {
            failedConstraints = packageConstraints;
            return undefined;
        }
        for (const candidate of candidates) {
            const branch = new Map(selectedById);
            branch.set(unresolvedId, candidate);
            const solved = search(branch);
            if (solved !== undefined)
                return solved;
        }
        return undefined;
    };
    const selectedById = search(new Map());
    if (selectedById === undefined) {
        const first = failedConstraints[0] ?? rootConstraints[0];
        if (first !== undefined) {
            const expected = failedConstraints
                .map((constraint) => constraint.request.version)
                .sort(compareText)
                .join(' & ');
            context.diagnostics.push(diagnostic('resolution', 'RULESET_PACKAGE_UNRESOLVED', first.path, `no package ${first.request.id} satisfies all constraints: ${expected}`, { packageId: first.request.id, expected }));
        }
        return undefined;
    }
    for (const source of [...selectedById.values()].sort((left, right) => compareIdentity(left.manifest.identity, right.manifest.identity))) {
        const identity = source.manifest.identity;
        const key = identityKey(identity.id, identity.version);
        context.selected.set(key, { key, source, aliases: new Map() });
    }
    const allConstraints = collectPackageConstraints(rootConstraints, selectedById);
    for (const constraint of allConstraints) {
        const source = selectedById.get(constraint.request.id);
        if (source === undefined)
            continue;
        const version = source.manifest.identity.version;
        const targetKey = identityKey(constraint.request.id, version);
        context.lock.push({
            requester: constraint.requester,
            packageId: constraint.request.id,
            requestedVersion: constraint.request.version,
            resolvedVersion: version,
            sourceFingerprint: source.sourceFingerprint,
            importAs: constraint.importAs,
            relationship: constraint.relationship,
        });
        context.relationships.push({
            kind: constraint.relationship,
            source: constraint.requester,
            target: targetKey,
            order: context.relationships.length,
        });
    }
    const selectedForRequest = (request) => {
        const source = selectedById.get(request.id);
        if (source === undefined)
            throw new Error(`resolved package ${request.id} is absent`);
        const key = identityKey(request.id, source.manifest.identity.version);
        const selected = context.selected.get(key);
        if (selected === undefined)
            throw new Error(`resolved package ${key} is absent`);
        return selected;
    };
    const base = selectedForRequest(composition.base);
    const additions = composition.add.map(selectedForRequest);
    const overlays = composition.overlays.map(selectedForRequest);
    resolveDependencies(context, [base, ...additions, ...overlays], selectedById);
    return { base, additions, overlays };
}
function collectPackageConstraints(roots, selectedById) {
    const constraints = [...roots];
    for (const source of [...selectedById.values()].sort((left, right) => compareIdentity(left.manifest.identity, right.manifest.identity))) {
        const requester = identityKey(source.manifest.identity.id, source.manifest.identity.version);
        for (const [index, dependency] of source.manifest.dependencies.entries()) {
            constraints.push({
                request: dependency,
                requester,
                importAs: dependency.importAs,
                relationship: 'dependsOn',
                path: `$.packages[${requester}].dependencies[${index}]`,
            });
        }
    }
    return constraints;
}
function groupConstraints(constraints) {
    const grouped = new Map();
    for (const constraint of constraints) {
        const group = grouped.get(constraint.request.id) ?? [];
        group.push(constraint);
        grouped.set(constraint.request.id, group);
    }
    return grouped;
}
function validateSupportedRange(constraint, diagnostics) {
    if (supportedRange(constraint.request.version))
        return;
    diagnostics.push(diagnostic('resolution', 'RULESET_VERSION_RANGE_UNSUPPORTED', `${constraint.path}.version`, `unsupported version range ${constraint.request.version}`, { packageId: constraint.request.id }));
}
function resolveDependencies(context, roots, selectedById) {
    const visiting = [];
    const visited = new Set();
    const visit = (entry) => {
        const cycleStart = visiting.indexOf(entry.key);
        if (cycleStart >= 0) {
            const graphPath = [...visiting.slice(cycleStart), entry.key];
            context.diagnostics.push(diagnostic('resolution', 'RULESET_DEPENDENCY_CYCLE', '$.dependencyGraph', `dependency cycle: ${graphPath.join(' -> ')}`, { graphPath }));
            return;
        }
        if (visited.has(entry.key))
            return;
        visiting.push(entry.key);
        const aliases = new Set();
        const dependencies = [...entry.source.manifest.dependencies].sort((left, right) => left.importAs.localeCompare(right.importAs));
        for (const [index, dependency] of dependencies.entries()) {
            const path = `$.packages[${entry.key}].dependencies[${index}]`;
            if (aliases.has(dependency.importAs)) {
                context.diagnostics.push(diagnostic('source', 'RULESET_DUPLICATE_IMPORT_ALIAS', `${path}.importAs`, `duplicate import alias ${dependency.importAs}`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
                continue;
            }
            aliases.add(dependency.importAs);
            const source = selectedById.get(dependency.id);
            if (source === undefined)
                continue;
            const targetKey = identityKey(dependency.id, source.manifest.identity.version);
            const resolved = context.selected.get(targetKey);
            if (resolved === undefined)
                continue;
            entry.aliases.set(dependency.importAs, resolved.key);
            visit(resolved);
        }
        visiting.pop();
        visited.add(entry.key);
    };
    for (const root of roots)
        visit(root);
}
function validateSelectedPackages(context, target) {
    for (const entry of context.selected.values()) {
        const manifest = entry.source.manifest;
        if (manifest.language.id !== target.language.id ||
            !satisfiesVersion(target.language.version, manifest.language.version)) {
            context.diagnostics.push(diagnostic('compatibility', 'RULESET_LANGUAGE_INCOMPATIBLE', `$.packages[${entry.key}].language`, `${entry.key} requires ${manifest.language.id}@${manifest.language.version}`, {
                packageId: manifest.identity.id,
                expected: `${target.language.id}@${target.language.version}`,
                actual: `${manifest.language.id}@${manifest.language.version}`,
                source: manifest.entry,
            }));
        }
        validateRequirements(entry, target, context.diagnostics);
    }
}
function validateRequirements(entry, target, diagnostics) {
    for (const [index, requirement] of entry.source.manifest.requirements.operations.entries()) {
        const supported = target.operations[requirement.id];
        if (supported !== requirement.version) {
            diagnostics.push(diagnostic('compatibility', 'RULESET_OPERATION_INCOMPATIBLE', `$.packages[${entry.key}].requirements.operations[${index}]`, `operation ${requirement.id}@${requirement.version} is unsupported`, {
                packageId: entry.source.manifest.identity.id,
                expected: supported === undefined ? 'unavailable' : String(supported),
                actual: String(requirement.version),
                source: entry.source.manifest.entry,
            }));
        }
    }
    for (const [index, requirement] of entry.source.manifest.requirements.capabilities.entries()) {
        const supported = target.capabilities[requirement.id];
        if (supported !== requirement.version) {
            diagnostics.push(diagnostic('compatibility', 'RULESET_CAPABILITY_INCOMPATIBLE', `$.packages[${entry.key}].requirements.capabilities[${index}]`, `capability ${requirement.id}@${requirement.version} is unsupported`, {
                packageId: entry.source.manifest.identity.id,
                expected: supported === undefined ? 'unavailable' : String(supported),
                actual: String(requirement.version),
                source: entry.source.manifest.entry,
            }));
        }
    }
}
function materializeSelectedDefinitions(context, composition, overlayPackages) {
    const definitionsByPackage = new Map();
    for (const entry of context.selected.values()) {
        rejectDuplicateValues(entry.source.manifest.definitions.map((definition) => definition.id), 'RULESET_DUPLICATE_LOCAL_DEFINITION', `$.packages[${entry.key}].definitions`, 'definition', context.diagnostics);
        const exports = new Set(entry.source.manifest.exports);
        const definitions = new Map();
        for (const definition of entry.source.manifest.definitions) {
            definitions.set(definition.id, {
                package: entry,
                definition,
                exported: exports.has(definition.id),
            });
        }
        for (const [index, definitionId] of entry.source.manifest.exports.entries()) {
            if (definitions.has(definitionId))
                continue;
            context.diagnostics.push(diagnostic('graph', 'RULESET_EXPORT_MISSING', `$.packages[${entry.key}].exports[${index}]`, `export ${definitionId} has no declaration`, { packageId: entry.source.manifest.identity.id, definitionId, source: entry.source.manifest.entry }));
        }
        definitionsByPackage.set(entry.key, definitions);
    }
    const derivationsByDefinition = new Map();
    for (const entry of context.selected.values()) {
        for (const [index, relationship] of entry.source.manifest.relationships.entries()) {
            if (relationship.version !== 1) {
                context.diagnostics.push(diagnostic('compatibility', 'RULESET_RELATIONSHIP_VERSION_UNSUPPORTED', `$.packages[${entry.key}].relationships[${index}].version`, `${relationship.kind} relationship version ${String(relationship.version)} is unsupported`, { packageId: entry.source.manifest.identity.id }));
            }
            if (relationship.kind !== 'derivesFrom')
                continue;
            const key = `${entry.key}#${relationship.definitionId}`;
            const relationships = derivationsByDefinition.get(key) ?? [];
            relationships.push(relationship);
            derivationsByDefinition.set(key, relationships);
        }
    }
    const records = new Map();
    const derivationProvenance = [];
    const relationshipProvenance = [];
    const visiting = [];
    const resolveConcrete = (record) => {
        const key = globalDefinitionId(record);
        const cached = records.get(key);
        if (cached !== undefined)
            return cached;
        const cycleStart = visiting.indexOf(key);
        if (cycleStart >= 0) {
            const graphPath = [...visiting.slice(cycleStart), key];
            context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_CYCLE', '$.derivationGraph', `derivation cycle: ${graphPath.join(' -> ')}`, { definitionId: record.definition.id, source: record.definition.source, graphPath }));
            return undefined;
        }
        if (visiting.length >= 32) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_DEPTH_EXCEEDED', '$.derivationGraph', `derivation depth exceeds the supported limit of 32 at ${key}`, { definitionId: record.definition.id, source: record.definition.source, graphPath: [...visiting, key] }));
            return undefined;
        }
        if (record.definition.kind === 'action' || record.definition.kind === 'support') {
            if ((derivationsByDefinition.get(key)?.length ?? 0) > 0) {
                context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_DECLARATION_INCOMPATIBLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, 'a derivesFrom relationship must name a derived definition declaration', { definitionId: record.definition.id, source: record.definition.source }));
                return undefined;
            }
            records.set(key, record);
            return record;
        }
        if (record.definition.kind === 'mixin' || record.definition.kind === 'template') {
            return undefined;
        }
        const derivations = derivationsByDefinition.get(key) ?? [];
        if (derivations.length !== 1) {
            context.diagnostics.push(diagnostic('materialization', derivations.length === 0
                ? 'RULESET_DERIVATION_BASE_MISSING'
                : 'RULESET_DERIVATION_BASE_AMBIGUOUS', `$.packages[${record.package.key}].definitions.${record.definition.id}`, derivations.length === 0
                ? `derived definition ${record.definition.id} has no primary base`
                : `derived definition ${record.definition.id} has more than one primary base`, { definitionId: record.definition.id, source: record.definition.source }));
            return undefined;
        }
        const derivation = derivations[0];
        if (derivation?.kind !== 'derivesFrom')
            return undefined;
        visiting.push(key);
        const baseSource = resolveRelationshipReference(record.package, derivation.target, `$.packages[${record.package.key}].relationships.${record.definition.id}.target`, definitionsByPackage, context.diagnostics);
        if (baseSource !== undefined &&
            (baseSource.definition.kind === 'mixin' || baseSource.definition.kind === 'template')) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].relationships.${record.definition.id}.target`, `derived ${record.definition.materializesAs} cannot use ${baseSource.definition.kind} base ${baseSource.definition.id}`, {
                definitionId: record.definition.id,
                source: record.definition.source,
                expected: record.definition.materializesAs,
                actual: baseSource.definition.kind,
            }));
            visiting.pop();
            return undefined;
        }
        const base = baseSource === undefined ? undefined : resolveConcrete(baseSource);
        if (base === undefined) {
            visiting.pop();
            return undefined;
        }
        if (base.definition.kind !== 'action' &&
            base.definition.kind !== 'support') {
            visiting.pop();
            return undefined;
        }
        if (base.definition.extensionPolicy !== 'derivable') {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_BASE_FORBIDDEN', `$.packages[${record.package.key}].relationships.${record.definition.id}.target`, `definition ${base.definition.id} is ${base.definition.extensionPolicy}, not derivable`, { definitionId: base.definition.id, source: record.definition.source }));
            visiting.pop();
            return undefined;
        }
        if (record.definition.materializesAs !== base.definition.kind) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_DERIVATION_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].definitions.${record.definition.id}.materializesAs`, `derived ${record.definition.materializesAs} cannot use ${base.definition.kind} base ${base.definition.id}`, { definitionId: record.definition.id, source: record.definition.source }));
            visiting.pop();
            return undefined;
        }
        let current = definitionValue(base);
        const changes = [];
        const mixinProvenance = [];
        const inheritedReferenceIds = new Set(resolveMaterializationReferenceIds(base, definitionsByPackage, context.diagnostics));
        for (const [order, application] of derivation.mixins.entries()) {
            const mixinRecord = resolveRelationshipReference(record.package, application.target, `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}]`, definitionsByPackage, context.diagnostics);
            if (mixinRecord === undefined || mixinRecord.definition.kind !== 'mixin') {
                if (mixinRecord !== undefined) {
                    context.diagnostics.push(diagnostic('materialization', 'RULESET_MIXIN_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}]`, `definition ${mixinRecord.definition.id} is not a mixin`, { definitionId: mixinRecord.definition.id, source: record.definition.source }));
                }
                continue;
            }
            const parameters = resolveMixinParameters(mixinRecord.definition, application.parameters, `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}].parameters`, context.diagnostics);
            if (parameters === undefined)
                continue;
            for (const referenceId of resolveMaterializationReferenceIds(mixinRecord, definitionsByPackage, context.diagnostics)) {
                inheritedReferenceIds.add(referenceId);
            }
            const applied = applyRulesetPatch(current, mixinRecord.definition.patch, parameters, `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}].patch`, context.diagnostics);
            if (applied === undefined)
                continue;
            current = applied;
            changes.push(...applied.changes);
            mixinProvenance.push({
                definitionId: mixinRecord.definition.id,
                packageId: mixinRecord.package.source.manifest.identity.id,
                packageVersion: mixinRecord.package.source.manifest.identity.version,
                fingerprint: stableFingerprint(mixinRecord.definition.patch),
                patch: mixinRecord.definition.patch,
                parameters,
                order,
            });
            relationshipProvenance.push({
                kind: 'derivesFrom',
                source: key,
                target: globalDefinitionId(mixinRecord),
                order,
            });
        }
        const local = applyRulesetPatch(current, derivation.localPatch, {}, `$.packages[${record.package.key}].relationships.${record.definition.id}.localPatch`, context.diagnostics);
        if (local !== undefined) {
            current = local;
            changes.push(...local.changes);
        }
        const concrete = concreteDerivedRecord(record, base, current, [...inheritedReferenceIds].sort(), context.diagnostics);
        visiting.pop();
        if (concrete === undefined)
            return undefined;
        records.set(key, concrete);
        const baseIdentity = base.package.source.manifest.identity;
        const identity = record.package.source.manifest.identity;
        derivationProvenance.push({
            definitionId: record.definition.id,
            packageId: identity.id,
            packageVersion: identity.version,
            baseDefinitionId: base.definition.id,
            basePackageId: baseIdentity.id,
            basePackageVersion: baseIdentity.version,
            baseFingerprint: definitionMaterializationFingerprint(base),
            base: definitionMaterializationStage(base),
            mixins: mixinProvenance,
            localPatchFingerprint: stableFingerprint(derivation.localPatch),
            localPatch: derivation.localPatch,
            materializedFingerprint: definitionMaterializationFingerprint(concrete),
            materialized: definitionMaterializationStage(concrete),
            changes,
        });
        relationshipProvenance.push({
            kind: 'derivesFrom',
            source: key,
            target: globalDefinitionId(base),
            order: 0,
        });
        return concrete;
    };
    for (const definitions of definitionsByPackage.values()) {
        for (const record of definitions.values()) {
            if (record.definition.kind === 'action' ||
                record.definition.kind === 'support' ||
                record.definition.kind === 'derived') {
                resolveConcrete(record);
            }
            if (record.definition.kind === 'template' &&
                record.definition.visibility === 'public') {
                context.diagnostics.push(diagnostic('graph', 'RULESET_PUBLIC_DEFINITION_UNREACHABLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `public template ${record.definition.id} has no materialized definition`, {
                    packageId: record.package.source.manifest.identity.id,
                    definitionId: record.definition.id,
                    source: record.definition.source,
                }));
            }
        }
    }
    const overlayProvenance = [];
    const overlayKeys = new Set(overlayPackages.map((entry) => entry.key));
    const writes = new Set();
    for (const entry of context.selected.values()) {
        const patchRelationships = entry.source.manifest.relationships.filter((relationship) => relationship.kind === 'patches');
        if (patchRelationships.length > 0 && !overlayKeys.has(entry.key)) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_PACKAGE_NOT_SELECTED', `$.packages[${entry.key}].relationships`, `package ${entry.key} declares patches but is not selected in composition overlay order`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
        }
    }
    for (const [overlayOrder, entry] of overlayPackages.entries()) {
        const relationships = entry.source.manifest.relationships.filter((relationship) => relationship.kind === 'patches');
        if (relationships.length === 0) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_EMPTY', `$.composition.overlays[${overlayOrder}]`, `selected overlay ${entry.key} declares no patch relationships`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
        }
        for (const [relationshipOrder, relationship] of relationships.entries()) {
            if (relationship.kind !== 'patches')
                continue;
            const sourceTarget = resolveRelationshipReference(entry, relationship.target, `$.packages[${entry.key}].relationships[${relationshipOrder}].target`, definitionsByPackage, context.diagnostics);
            const target = sourceTarget === undefined ? undefined : records.get(globalDefinitionId(sourceTarget));
            if (target === undefined)
                continue;
            const targetIdentity = target.package.source.manifest.identity;
            if (relationship.targetPackage.id !== targetIdentity.id ||
                relationship.targetPackage.version !== targetIdentity.version) {
                context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_TARGET_PACKAGE_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].targetPackage`, `overlay pins ${relationship.targetPackage.id}@${relationship.targetPackage.version}, resolved ${targetIdentity.id}@${targetIdentity.version}`, { definitionId: target.definition.id, expected: `${relationship.targetPackage.id}@${relationship.targetPackage.version}`, actual: `${targetIdentity.id}@${targetIdentity.version}` }));
                continue;
            }
            if (target.definition.extensionPolicy !== 'patchable') {
                context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_TARGET_FORBIDDEN', `$.packages[${entry.key}].relationships[${relationshipOrder}].target`, `definition ${target.definition.id} is ${target.definition.extensionPolicy}, not patchable`, { definitionId: target.definition.id, source: entry.source.manifest.entry }));
                continue;
            }
            const before = definitionMaterializationStage(target);
            const beforeFingerprint = stableFingerprint(before);
            if (beforeFingerprint !== relationship.expectedFingerprint) {
                context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_EXPECTED_FINGERPRINT_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].expectedFingerprint`, `overlay expected ${relationship.expectedFingerprint}, materialized target is ${beforeFingerprint}`, { definitionId: target.definition.id, expected: relationship.expectedFingerprint, actual: beforeFingerprint }));
                continue;
            }
            if (!patchMatchesPlane(relationship.patch, relationship.plane)) {
                context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_IMPACT_PLANE_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, `overlay patch operations exceed declared ${relationship.plane} impact plane`, { definitionId: target.definition.id }));
                continue;
            }
            let conflicted = false;
            for (const operation of relationship.patch.operations) {
                const write = `${target.definition.id}:${operation.plane}:${patchPath(operation.path)}`;
                if (writes.has(write) && relationship.conflictPolicy === 'reject') {
                    conflicted = true;
                    context.diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_WRITE_CONFLICT', `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, `overlay write conflicts at ${write}`, { definitionId: target.definition.id }));
                }
                writes.add(write);
            }
            if (conflicted)
                continue;
            const applied = applyRulesetPatch(definitionValue(target), relationship.patch, {}, `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, context.diagnostics);
            if (applied === undefined)
                continue;
            const patched = replaceConcreteRecordValue(target, applied, context.diagnostics);
            if (patched === undefined)
                continue;
            records.set(globalDefinitionId(target), patched);
            const afterFingerprint = definitionMaterializationFingerprint(patched);
            overlayProvenance.push({
                overlayPackageId: entry.source.manifest.identity.id,
                overlayPackageVersion: entry.source.manifest.identity.version,
                targetDefinitionId: target.definition.id,
                targetPackageId: targetIdentity.id,
                targetPackageVersion: targetIdentity.version,
                expectedFingerprint: relationship.expectedFingerprint,
                beforeFingerprint,
                afterFingerprint,
                plane: relationship.plane,
                conflictPolicy: relationship.conflictPolicy,
                patchFingerprint: stableFingerprint(relationship.patch),
                patch: relationship.patch,
                before,
                order: overlayOrder * 1_000 + relationshipOrder,
                changes: applied.changes,
            });
            relationshipProvenance.push({
                kind: 'patches',
                source: entry.key,
                target: globalDefinitionId(target),
                order: overlayOrder * 1_000 + relationshipOrder,
            });
        }
    }
    for (const [optionOrder, [optionId, selectedValue]] of Object.entries(composition.configure)
        .sort(([left], [right]) => left.localeCompare(right))
        .entries()) {
        const matches = [...context.selected.values()].flatMap((entry) => entry.source.manifest.relationships
            .filter((relationship) => relationship.kind === 'configures' &&
            relationship.optionId === optionId &&
            relationship.value === selectedValue)
            .map((relationship) => ({ entry, relationship })));
        if (matches.length !== 1) {
            context.diagnostics.push(diagnostic('materialization', matches.length === 0
                ? 'RULESET_CONFIGURATION_OPTION_UNAVAILABLE'
                : 'RULESET_CONFIGURATION_OPTION_AMBIGUOUS', `$.composition.configure.${optionId}`, matches.length === 0
                ? `no selected package exposes ${optionId}=${String(selectedValue)}`
                : `more than one selected package exposes ${optionId}=${String(selectedValue)}`));
            continue;
        }
        const match = matches[0];
        if (match === undefined || match.relationship.kind !== 'configures')
            continue;
        const sourceTarget = resolveRelationshipReference(match.entry, match.relationship.target, `$.packages[${match.entry.key}].relationships.${optionId}.target`, definitionsByPackage, context.diagnostics);
        const target = sourceTarget === undefined ? undefined : records.get(globalDefinitionId(sourceTarget));
        if (target === undefined)
            continue;
        if (target.definition.extensionPolicy !== 'configurable') {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_CONFIGURATION_TARGET_FORBIDDEN', `$.packages[${match.entry.key}].relationships.${optionId}.target`, `definition ${target.definition.id} is ${target.definition.extensionPolicy}, not configurable`, { definitionId: target.definition.id }));
            continue;
        }
        const applied = applyRulesetPatch(definitionValue(target), match.relationship.patch, {}, `$.packages[${match.entry.key}].relationships.${optionId}.patch`, context.diagnostics);
        if (applied === undefined)
            continue;
        const configured = replaceConcreteRecordValue(target, applied, context.diagnostics);
        if (configured === undefined)
            continue;
        records.set(globalDefinitionId(target), configured);
        relationshipProvenance.push({
            kind: 'configures',
            source: match.entry.key,
            target: `${globalDefinitionId(target)}:${optionId}=${String(selectedValue)}`,
            order: optionOrder,
        });
    }
    if (context.diagnostics.length > 0)
        return undefined;
    return {
        records: [...records.values()].sort((left, right) => globalDefinitionId(left).localeCompare(globalDefinitionId(right))),
        derivationProvenance: derivationProvenance.sort((left, right) => left.definitionId.localeCompare(right.definitionId)),
        overlayProvenance: overlayProvenance.sort((left, right) => left.order - right.order),
        relationships: relationshipProvenance,
    };
}
function resolveRelationshipReference(sourcePackage, reference, path, definitionsByPackage, diagnostics) {
    const targetPackageKey = reference.importAs === undefined
        ? sourcePackage.key
        : sourcePackage.aliases.get(reference.importAs);
    if (targetPackageKey === undefined) {
        diagnostics.push(diagnostic('materialization', 'RULESET_IMPORT_ALIAS_UNRESOLVED', path, `import alias ${reference.importAs ?? ''} is not declared`, { packageId: sourcePackage.source.manifest.identity.id, source: sourcePackage.source.manifest.entry }));
        return undefined;
    }
    const target = definitionsByPackage.get(targetPackageKey)?.get(reference.definitionId);
    if (target === undefined) {
        diagnostics.push(diagnostic('materialization', 'RULESET_DEFINITION_REFERENCE_MISSING', path, `definition ${reference.definitionId} was not found in ${targetPackageKey}`, { packageId: sourcePackage.source.manifest.identity.id, definitionId: reference.definitionId, source: sourcePackage.source.manifest.entry }));
        return undefined;
    }
    if (targetPackageKey !== sourcePackage.key &&
        (!target.exported || target.definition.visibility === 'private')) {
        diagnostics.push(diagnostic('materialization', 'RULESET_PRIVATE_CROSS_PACKAGE_REFERENCE', path, `definition ${target.definition.id} is not exported for cross-package use`, { packageId: target.package.source.manifest.identity.id, definitionId: target.definition.id, source: sourcePackage.source.manifest.entry }));
        return undefined;
    }
    return target;
}
function definitionValue(record) {
    if (record.definition.kind === 'action') {
        return { semantic: record.definition.action, presentation: record.definition.presentation ?? null };
    }
    if (record.definition.kind === 'support') {
        return { semantic: record.definition.semantic, presentation: record.definition.presentation ?? null };
    }
    throw new Error(`definition ${record.definition.id} is not concrete`);
}
function concreteDerivedRecord(derived, base, value, inheritedReferenceIds, diagnostics) {
    if (derived.definition.kind !== 'derived')
        return undefined;
    const references = uniqueReferences(derived.definition.lowLevelReferences ?? []);
    if (derived.definition.materializesAs === 'action') {
        if (!isRecord(value.semantic)) {
            diagnostics.push(diagnostic('materialization', 'RULESET_DERIVED_ACTION_INVALID', '$.semantic', 'derived action semantic value must be an object', { definitionId: derived.definition.id }));
            return undefined;
        }
        const action = immutable({
            ...value.semantic,
            id: derived.definition.id,
            sourcePath: derived.definition.source.module,
        });
        return {
            package: derived.package,
            exported: derived.exported,
            inheritedReferenceIds,
            definition: immutable({
                kind: 'action',
                id: derived.definition.id,
                visibility: derived.definition.visibility,
                extensionPolicy: derived.definition.extensionPolicy,
                source: derived.definition.source,
                references,
                ...(value.presentation === null ? {} : { presentation: value.presentation }),
                action,
            }),
        };
    }
    if (!isRecord(value.semantic)) {
        diagnostics.push(diagnostic('materialization', 'RULESET_DERIVED_SUPPORT_INVALID', '$.semantic', 'derived support semantic value must be an object', { definitionId: derived.definition.id }));
        return undefined;
    }
    return {
        package: derived.package,
        exported: derived.exported,
        inheritedReferenceIds,
        definition: immutable({
            kind: 'support',
            id: derived.definition.id,
            visibility: derived.definition.visibility,
            extensionPolicy: derived.definition.extensionPolicy,
            source: derived.definition.source,
            references,
            ...(value.presentation === null ? {} : { presentation: value.presentation }),
            semantic: value.semantic,
        }),
    };
}
function replaceConcreteRecordValue(record, value, diagnostics) {
    if (record.definition.kind === 'action') {
        if (!isRecord(value.semantic))
            return undefined;
        return {
            ...record,
            definition: immutable({
                ...record.definition,
                action: value.semantic,
                ...(value.presentation === null ? {} : { presentation: value.presentation }),
            }),
        };
    }
    if (record.definition.kind === 'support') {
        if (!isRecord(value.semantic))
            return undefined;
        return {
            ...record,
            definition: immutable({
                ...record.definition,
                semantic: value.semantic,
                ...(value.presentation === null ? {} : { presentation: value.presentation }),
            }),
        };
    }
    diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_TARGET_INCOMPATIBLE', '$.patch', `definition ${record.definition.id} is not patchable materialized content`, { definitionId: record.definition.id }));
    return undefined;
}
function uniqueReferences(references) {
    const byIdentity = new Map();
    for (const reference of references) {
        byIdentity.set(`${reference.importAs ?? ''}#${reference.definitionId}`, reference);
    }
    return [...byIdentity.values()].sort((left, right) => `${left.importAs ?? ''}#${left.definitionId}`.localeCompare(`${right.importAs ?? ''}#${right.definitionId}`));
}
function resolveMixinParameters(mixin, supplied, path, diagnostics) {
    const definitions = new Map(mixin.parameters.map((parameter) => [parameter.id, parameter]));
    const resolved = {};
    for (const parameterId of Object.keys(supplied)) {
        if (definitions.has(parameterId))
            continue;
        diagnostics.push(diagnostic('materialization', 'RULESET_MIXIN_PARAMETER_UNKNOWN', `${path}.${parameterId}`, `mixin ${mixin.id} does not declare parameter ${parameterId}`, { definitionId: mixin.id }));
    }
    for (const parameter of mixin.parameters) {
        const value = supplied[parameter.id] ?? parameter.default;
        if (value === undefined) {
            diagnostics.push(diagnostic('materialization', 'RULESET_MIXIN_PARAMETER_MISSING', `${path}.${parameter.id}`, `mixin parameter ${parameter.id} is required`, { definitionId: mixin.id }));
            continue;
        }
        if (typeof value !== parameter.type) {
            diagnostics.push(diagnostic('materialization', 'RULESET_MIXIN_PARAMETER_TYPE_MISMATCH', `${path}.${parameter.id}`, `mixin parameter ${parameter.id} must be ${parameter.type}`, { definitionId: mixin.id, expected: parameter.type, actual: typeof value }));
            continue;
        }
        resolved[parameter.id] = value;
    }
    return diagnostics.length > 0 ? undefined : immutable(resolved);
}
function applyRulesetPatch(value, patch, parameters, path, diagnostics) {
    if (patch.version !== 1) {
        diagnostics.push(diagnostic('compatibility', 'RULESET_PATCH_VERSION_UNSUPPORTED', `${path}.version`, `patch version ${String(patch.version)} is unsupported`));
        return undefined;
    }
    let semantic = cloneJsonValue(value.semantic);
    let presentation = cloneJsonValue(value.presentation ?? {});
    const changes = [];
    for (const [index, operation] of patch.operations.entries()) {
        const operationPath = `${path}.operations[${index}]`;
        const root = operation.plane === 'semantic' ? semantic : presentation;
        const before = cloneJsonValue(readPatchPath(root, operation.path, operationPath, diagnostics));
        if (operation.kind === 'setScalar') {
            const replacement = resolvePatchScalar(operation.value, parameters, operationPath, diagnostics);
            if (replacement === undefined && operation.value !== null)
                continue;
            if (!writePatchPath(root, operation.path, replacement ?? null, operationPath, diagnostics))
                continue;
        }
        else if (operation.kind === 'adjustNumber') {
            const current = readPatchPath(root, operation.path, operationPath, diagnostics);
            const multiply = resolvePatchNumber(operation.multiply, parameters, operationPath, diagnostics);
            const add = resolvePatchNumber(operation.add, parameters, operationPath, diagnostics);
            if (typeof current !== 'number' || multiply === undefined || add === undefined) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_NUMBER_REQUIRED', operationPath, `adjustNumber requires a numeric target at ${patchPath(operation.path)}`));
                continue;
            }
            if (!writePatchPath(root, operation.path, current * multiply + add, operationPath, diagnostics))
                continue;
        }
        else if (operation.kind === 'appendMember') {
            const target = readPatchPath(root, operation.path, operationPath, diagnostics);
            if (!Array.isArray(target)) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_LIST_REQUIRED', operationPath, `appendMember requires a list at ${patchPath(operation.path)}`));
                continue;
            }
            if (target.some((entry) => isRecord(entry) && entry[operation.identity.key] === operation.identity.value)) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_MEMBER_DUPLICATE', operationPath, `member ${operation.identity.key}=${operation.identity.value} already exists`));
                continue;
            }
            const member = { ...operation.value, [operation.identity.key]: operation.identity.value };
            const position = operation.position;
            if (position.kind === 'start')
                target.unshift(member);
            else if (position.kind === 'end')
                target.push(member);
            else {
                const anchorIndex = target.findIndex((entry) => memberMatches(entry, position.anchor));
                if (anchorIndex < 0) {
                    diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_ANCHOR_MISSING', operationPath, `anchor ${patchSegment(position.anchor)} is missing`));
                    continue;
                }
                target.splice(position.kind === 'before' ? anchorIndex : anchorIndex + 1, 0, member);
            }
        }
        else {
            const target = readPatchPath(root, operation.path, operationPath, diagnostics);
            if (!Array.isArray(target)) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_LIST_REQUIRED', operationPath, `removeMember requires a list at ${patchPath(operation.path)}`));
                continue;
            }
            const indexes = target
                .map((entry, memberIndex) => memberMatches(entry, operation.identity) ? memberIndex : -1)
                .filter((memberIndex) => memberIndex >= 0);
            if (indexes.length !== 1) {
                diagnostics.push(diagnostic('materialization', indexes.length === 0 ? 'RULESET_PATCH_MEMBER_MISSING' : 'RULESET_PATCH_MEMBER_AMBIGUOUS', operationPath, `member ${patchSegment(operation.identity)} must resolve exactly once`));
                continue;
            }
            target.splice(indexes[0] ?? 0, 1);
        }
        const after = cloneJsonValue(readPatchPath(root, operation.path, operationPath, diagnostics));
        changes.push({
            plane: operation.plane,
            path: patchPath(operation.path),
            pathSegments: operation.path,
            before,
            after,
            effective: canonicalJson(before) !== canonicalJson(after),
        });
        if (operation.plane === 'semantic')
            semantic = root;
        else
            presentation = root;
    }
    if (diagnostics.length > 0)
        return undefined;
    return {
        semantic: immutable(semantic),
        presentation: Object.keys(isRecord(presentation) ? presentation : {}).length === 0
            ? null
            : immutable(presentation),
        changes: immutable(changes),
    };
}
function readPatchPath(root, path, diagnosticPath, diagnostics) {
    let current = root;
    for (const segment of path) {
        if (segment.kind === 'field') {
            if (!isRecord(current) || !(segment.name in current)) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_PATH_MISSING', diagnosticPath, `field ${segment.name} is missing at ${patchPath(path)}`));
                return undefined;
            }
            current = current[segment.name];
        }
        else {
            if (!Array.isArray(current)) {
                diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_LIST_REQUIRED', diagnosticPath, `member selector ${patchSegment(segment)} requires a list`));
                return undefined;
            }
            const matches = current.filter((entry) => memberMatches(entry, segment));
            if (matches.length !== 1) {
                diagnostics.push(diagnostic('materialization', matches.length === 0 ? 'RULESET_PATCH_MEMBER_MISSING' : 'RULESET_PATCH_MEMBER_AMBIGUOUS', diagnosticPath, `member ${patchSegment(segment)} must resolve exactly once`));
                return undefined;
            }
            current = matches[0];
        }
    }
    return current;
}
function writePatchPath(root, path, value, diagnosticPath, diagnostics) {
    if (path.length === 0) {
        diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_ROOT_WRITE_FORBIDDEN', diagnosticPath, 'patch operations must name a field or stable member'));
        return false;
    }
    const parentPath = path.slice(0, -1);
    const parent = readPatchPath(root, parentPath, diagnosticPath, diagnostics);
    const leaf = path[path.length - 1];
    if (leaf?.kind !== 'field' || !isRecord(parent) || !(leaf.name in parent)) {
        diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_PATH_MISSING', diagnosticPath, `writable field is missing at ${patchPath(path)}`));
        return false;
    }
    parent[leaf.name] = value;
    return true;
}
function resolvePatchScalar(value, parameters, path, diagnostics) {
    if (!isParameterReference(value))
        return value;
    const resolved = parameters[value.parameter];
    if (resolved !== undefined)
        return resolved;
    diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_PARAMETER_UNRESOLVED', path, `parameter ${value.parameter} is not supplied`));
    return undefined;
}
function resolvePatchNumber(value, parameters, path, diagnostics) {
    if (typeof value === 'number')
        return value;
    const resolved = parameters[value.parameter];
    if (typeof resolved === 'number')
        return resolved;
    diagnostics.push(diagnostic('materialization', 'RULESET_PATCH_NUMBER_PARAMETER_UNRESOLVED', path, `numeric parameter ${value.parameter} is not supplied`));
    return undefined;
}
function isParameterReference(value) {
    return isRecord(value) && typeof value['parameter'] === 'string';
}
function patchMatchesPlane(patch, plane) {
    return patch.operations.every((operation) => plane === 'both' || operation.plane === plane);
}
function patchPath(path) {
    return path.map(patchSegment).join('.');
}
function patchSegment(segment) {
    return segment.kind === 'field'
        ? segment.name
        : `[${segment.key}=${segment.value}]`;
}
function memberMatches(value, selector) {
    return isRecord(value) && value[selector.key] === selector.value;
}
function definitionMaterializationFingerprint(record) {
    return stableFingerprint(definitionMaterializationStage(record));
}
function definitionMaterializationStage(record) {
    if (record.definition.kind !== 'action' && record.definition.kind !== 'support') {
        throw new Error(`definition ${record.definition.id} is not concrete`);
    }
    return {
        id: record.definition.id,
        kind: record.definition.kind,
        extensionPolicy: record.definition.extensionPolicy,
        value: normalizedDefinitionValue(record),
        references: materializationReferenceIds(record),
    };
}
export function rulesetDefinitionMaterializationFingerprint(definition) {
    return stableFingerprint({
        id: definition.id,
        kind: definition.kind,
        extensionPolicy: definition.extensionPolicy,
        value: definition.kind === 'action'
            ? { semantic: normalizeAction(definition.action), presentation: definition.presentation ?? null }
            : { semantic: definition.semantic, presentation: definition.presentation ?? null },
        references: authoredDefinitionReferenceIds(definition),
    });
}
function normalizedDefinitionValue(record) {
    if (record.definition.kind === 'action') {
        return {
            semantic: normalizeAction(record.definition.action),
            presentation: record.definition.presentation ?? null,
        };
    }
    if (record.definition.kind === 'support') {
        return {
            semantic: record.definition.semantic,
            presentation: record.definition.presentation ?? null,
        };
    }
    throw new Error(`definition ${record.definition.id} is not concrete`);
}
function materializationReferenceIds(record) {
    return [
        ...new Set([
            ...authoredDefinitionReferenceIds(record.definition),
            ...(record.inheritedReferenceIds ?? []).map(localDefinitionId),
        ]),
    ].sort();
}
const CATALOG_REFERENCE_FIELDS = {
    statId: 'stat',
    defenseId: 'defense',
    resourceId: 'resource',
    modifierId: 'modifier',
    damageType: 'damageType',
};
function authoredDefinitionReferenceIds(definition) {
    return [
        ...new Set([
            ...(definition.lowLevelReferences ?? []).map((reference) => reference.definitionId),
            ...authoredCatalogReferences(definition).map((reference) => reference.definitionId),
        ]),
    ].sort();
}
function authoredCatalogReferences(definition) {
    if (definition.kind !== 'action')
        return [];
    const byIdentity = new Map();
    collectCatalogReferences(definition.action, '$.action', byIdentity);
    return [...byIdentity.values()].sort((left, right) => `${left.category}#${left.definitionId}`.localeCompare(`${right.category}#${right.definitionId}`));
}
function collectCatalogReferences(value, path, references) {
    if (Array.isArray(value)) {
        value.forEach((entry, index) => collectCatalogReferences(entry, `${path}[${index}]`, references));
        return;
    }
    if (!isRecord(value))
        return;
    for (const [key, child] of Object.entries(value)) {
        const childPath = `${path}.${key}`;
        const category = CATALOG_REFERENCE_FIELDS[key];
        if (category !== undefined && typeof child === 'string') {
            references.set(`${category}#${child}`, {
                definitionId: child,
                category,
                path: childPath,
            });
            continue;
        }
        collectCatalogReferences(child, childPath, references);
    }
}
function definitionReferences(record, definitionsByPackage, diagnostics) {
    const references = [...(record.definition.lowLevelReferences ?? [])];
    const inheritedLocalIds = new Set((record.inheritedReferenceIds ?? []).map(localDefinitionId));
    for (const catalogReference of authoredCatalogReferences(record.definition)) {
        const definitionId = catalogReference.definitionId;
        if (inheritedLocalIds.has(definitionId))
            continue;
        if (definitionsByPackage.get(record.package.key)?.has(definitionId) === true) {
            references.push({ definitionId });
            continue;
        }
        const matches = [...record.package.aliases.entries()].filter(([, packageKey]) => definitionsByPackage.get(packageKey)?.has(definitionId) === true);
        if (matches.length === 1) {
            for (const [importAs] of matches) {
                references.push({ importAs, definitionId });
            }
            continue;
        }
        if (matches.length > 1) {
            diagnostics.push(diagnostic('graph', 'RULESET_CATALOG_REFERENCE_AMBIGUOUS', catalogReference.path, `catalog definition ${definitionId} is provided by more than one dependency`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
            }));
            continue;
        }
        const aliases = [...record.package.aliases.keys()];
        if (aliases.length === 1) {
            for (const importAs of aliases) {
                references.push({ importAs, definitionId });
            }
        }
        else {
            references.push({ definitionId });
        }
    }
    return uniqueReferences(references);
}
function cloneJsonValue(value) {
    if (Array.isArray(value))
        return value.map(cloneJsonValue);
    if (isRecord(value)) {
        return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, cloneJsonValue(child)]));
    }
    return value;
}
function isRecord(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}
function closeDefinitionGraph(context, rootKeys, sourceRecords) {
    const definitionsByPackage = new Map();
    for (const entry of context.selected.values()) {
        const definitions = new Map();
        const exports = new Set(entry.source.manifest.exports);
        for (const definition of entry.source.manifest.definitions) {
            definitions.set(definition.id, {
                package: entry,
                definition,
                exported: exports.has(definition.id),
            });
        }
        for (const record of sourceRecords.filter((candidate) => candidate.package.key === entry.key)) {
            definitions.set(record.definition.id, record);
        }
        for (const [index, definitionId] of entry.source.manifest.exports.entries()) {
            if (!entry.source.manifest.definitions.some((definition) => definition.id === definitionId)) {
                context.diagnostics.push(diagnostic('graph', 'RULESET_EXPORT_MISSING', `$.packages[${entry.key}].exports[${index}]`, `export ${definitionId} has no declaration`, { packageId: entry.source.manifest.identity.id, definitionId, source: entry.source.manifest.entry }));
            }
        }
        definitionsByPackage.set(entry.key, definitions);
    }
    const roots = [...rootKeys]
        .flatMap((key) => [...(definitionsByPackage.get(key)?.values() ?? [])]
        .filter((record) => record.exported &&
        sourceRecords.some((candidate) => globalDefinitionId(candidate) === globalDefinitionId(record)))
        .map((record) => globalDefinitionId(record)))
        .sort();
    const reachable = new Set();
    const visiting = [];
    const resolvedReferences = new Map();
    const byGlobalId = new Map();
    for (const definitions of definitionsByPackage.values()) {
        for (const record of definitions.values())
            byGlobalId.set(globalDefinitionId(record), record);
    }
    const visit = (record) => {
        const globalId = globalDefinitionId(record);
        const cycleStart = visiting.indexOf(globalId);
        if (cycleStart >= 0) {
            const graphPath = [...visiting.slice(cycleStart), globalId];
            context.diagnostics.push(diagnostic('graph', 'RULESET_DEFINITION_CYCLE', '$.definitionGraph', `definition cycle: ${graphPath.join(' -> ')}`, { definitionId: record.definition.id, source: record.definition.source, graphPath }));
            return;
        }
        if (reachable.has(globalId))
            return;
        visiting.push(globalId);
        const references = new Set();
        for (const [index, reference] of definitionReferences(record, definitionsByPackage, context.diagnostics).entries()) {
            const target = resolveDefinitionReference(record, reference, index, definitionsByPackage, context.diagnostics);
            if (target !== undefined) {
                references.add(globalDefinitionId(target));
                visit(target);
            }
        }
        for (const inheritedReferenceId of record.inheritedReferenceIds ?? []) {
            const target = byGlobalId.get(inheritedReferenceId);
            if (target === undefined) {
                context.diagnostics.push(diagnostic('graph', 'RULESET_INHERITED_REFERENCE_MISSING', `$.packages[${record.package.key}].definitions.${record.definition.id}.references`, `inherited definition reference ${inheritedReferenceId} is missing`, {
                    packageId: record.package.source.manifest.identity.id,
                    definitionId: record.definition.id,
                    source: record.definition.source,
                }));
                continue;
            }
            references.add(inheritedReferenceId);
            visit(target);
        }
        visiting.pop();
        reachable.add(globalId);
        resolvedReferences.set(globalId, Object.freeze([...references].sort()));
    };
    for (const root of roots) {
        const record = byGlobalId.get(root);
        if (record !== undefined)
            visit(record);
    }
    for (const record of sourceRecords) {
        const globalId = globalDefinitionId(record);
        if (rootKeys.has(record.package.key) &&
            !reachable.has(globalId) &&
            record.definition.visibility === 'public') {
            context.diagnostics.push(diagnostic('graph', 'RULESET_PUBLIC_DEFINITION_UNREACHABLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `public definition ${record.definition.id} is unreachable from an exported root`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
            }));
        }
    }
    const materialized = [...reachable]
        .map((id) => byGlobalId.get(id))
        .filter((record) => record !== undefined)
        .sort((left, right) => globalDefinitionId(left).localeCompare(globalDefinitionId(right)));
    const materializedIds = new Set();
    for (const record of materialized) {
        if (record.definition.kind === 'template') {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_TEMPLATE_MATERIALIZATION_DEFERRED', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `template ${record.definition.id} requires #5957 derivation materialization`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
            }));
        }
        if (materializedIds.has(record.definition.id)) {
            context.diagnostics.push(diagnostic('graph', 'RULESET_DUPLICATE_DEFINITION_ID', '$.materializedDefinitions', `definition identity ${record.definition.id} is contributed by more than one package`, { definitionId: record.definition.id, source: record.definition.source }));
        }
        else {
            materializedIds.add(record.definition.id);
        }
    }
    if (context.diagnostics.length > 0)
        return undefined;
    return {
        materialized,
        exportedRoots: roots.map((root) => byGlobalId.get(root)?.definition.id ?? root).sort(),
        resolvedReferences,
        byGlobalId: new Map(materialized.map((record) => [record.definition.id, record])),
    };
}
function resolveMaterializationReferenceIds(record, definitionsByPackage, diagnostics) {
    const resolved = new Set(record.inheritedReferenceIds ?? []);
    for (const [index, reference] of definitionReferences(record, definitionsByPackage, diagnostics).entries()) {
        const target = resolveDefinitionReference(record, reference, index, definitionsByPackage, diagnostics);
        if (target !== undefined)
            resolved.add(globalDefinitionId(target));
    }
    return [...resolved].sort();
}
function resolveDefinitionReference(source, reference, index, definitionsByPackage, diagnostics) {
    const targetPackageKey = reference.importAs === undefined
        ? source.package.key
        : source.package.aliases.get(reference.importAs);
    const path = `$.packages[${source.package.key}].definitions.${source.definition.id}.references[${index}]`;
    if (targetPackageKey === undefined) {
        diagnostics.push(diagnostic('graph', 'RULESET_IMPORT_ALIAS_UNRESOLVED', path, `import alias ${reference.importAs ?? ''} is not declared`, {
            packageId: source.package.source.manifest.identity.id,
            definitionId: source.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    const target = definitionsByPackage.get(targetPackageKey)?.get(reference.definitionId);
    if (target === undefined) {
        diagnostics.push(diagnostic('graph', 'RULESET_DEFINITION_REFERENCE_MISSING', path, `definition ${reference.definitionId} was not found in ${targetPackageKey}`, {
            packageId: source.package.source.manifest.identity.id,
            definitionId: source.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    if (targetPackageKey !== source.package.key &&
        (!target.exported || target.definition.visibility === 'private') &&
        !source.inheritedReferenceIds?.includes(globalDefinitionId(target))) {
        diagnostics.push(diagnostic('graph', 'RULESET_PRIVATE_CROSS_PACKAGE_REFERENCE', path, `definition ${target.definition.id} is not exported for cross-package use`, {
            packageId: target.package.source.manifest.identity.id,
            definitionId: target.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    const catalogReference = authoredCatalogReferences(source.definition).find((candidate) => candidate.definitionId === reference.definitionId);
    if (catalogReference !== undefined &&
        (target.definition.kind !== 'support' ||
            target.definition.semantic.catalog !== catalogReference.category)) {
        diagnostics.push(diagnostic('graph', 'RULESET_CATALOG_REFERENCE_KIND_MISMATCH', catalogReference.path, `catalog reference ${reference.definitionId} requires ${catalogReference.category} support`, {
            packageId: source.package.source.manifest.identity.id,
            definitionId: source.definition.id,
            source: source.definition.source,
            expected: catalogReference.category,
            actual: target.definition.kind === 'support'
                ? target.definition.semantic.catalog
                : target.definition.kind,
        }));
        return undefined;
    }
    return target;
}
function normalizeMaterializedActions(composition, graph, diagnostics) {
    const actions = graph.materialized
        .filter((record) => record.definition.kind === 'action')
        .map((record) => {
        if (record.definition.kind !== 'action')
            throw new Error('unreachable narrowing failure');
        if (record.definition.action.id !== record.definition.id) {
            diagnostics.push(diagnostic('materialization', 'RULESET_ACTION_ID_MISMATCH', `$.definitions.${record.definition.id}.action.id`, 'definition identity must match the normalized action identity', { definitionId: record.definition.id, source: record.definition.source }));
        }
        return record.definition.action;
    });
    if (diagnostics.length > 0)
        return undefined;
    const result = normalizePackage(definePackage({
        id: composition.identity.id,
        version: composition.identity.version,
        sources: [defineActions('compiled-ruleset-actions', actions)],
    }));
    if (!result.ok) {
        diagnostics.push(...result.diagnostics.map((entry) => diagnostic('normalization', entry.code, entry.path, entry.message, entry.sourcePath === undefined
            ? {}
            : { source: { module: entry.sourcePath, declaration: entry.path } })));
        return undefined;
    }
    return result.artifact;
}
function collectRequirements(context, normalizedRequirements, target) {
    const operations = new Map();
    const capabilities = new Map();
    for (const requirement of normalizedRequirements) {
        if (requirement.kind === 'operation') {
            operations.set(requirement.id, requirement.version);
        }
        else {
            capabilities.set(requirement.id, requirement.version);
        }
    }
    for (const entry of context.selected.values()) {
        for (const requirement of entry.source.manifest.requirements.operations) {
            operations.set(requirement.id, requirement.version);
        }
        for (const requirement of entry.source.manifest.requirements.capabilities) {
            capabilities.set(requirement.id, requirement.version);
        }
    }
    if (context.diagnostics.length > 0)
        return undefined;
    return {
        operations: [...operations]
            .map(([id, version]) => ({ id, version }))
            .sort(compareRequirement),
        capabilities: [...capabilities]
            .map(([id, version]) => ({ id, version }))
            .sort(compareRequirement),
    };
}
function materializeDefinitions(records, references, exportedRoots, actions) {
    const normalizedActions = new Map(actions.map((action) => [action.id, action]));
    const rootSet = new Set(exportedRoots);
    return records
        .filter((record) => record.definition.kind === 'action' || record.definition.kind === 'support')
        .map((record) => {
        const definition = record.definition;
        const semantic = definition.kind === 'action'
            ? normalizedActions.get(definition.id)
            : definition.semantic;
        if (semantic === undefined)
            throw new Error(`materialization missing ${definition.id}`);
        const materialized = {
            id: definition.id,
            kind: definition.kind,
            visibility: rootSet.has(definition.id) ? 'exported' : 'support',
            extensionPolicy: definition.extensionPolicy,
            semantic,
            presentation: definition.presentation ?? null,
            references: (references.get(globalDefinitionId(record)) ?? []).map(localDefinitionId),
            provenance: provenance(record),
        };
        return {
            ...materialized,
            fingerprint: stableFingerprint(materialized),
        };
    })
        .sort((left, right) => left.id.localeCompare(right.id));
}
function provenance(record) {
    return {
        definitionId: record.definition.id,
        packageId: record.package.source.manifest.identity.id,
        packageVersion: record.package.source.manifest.identity.version,
        source: record.definition.source,
    };
}
function globalDefinitionId(record) {
    return `${record.package.key}#${record.definition.id}`;
}
function localDefinitionId(globalId) {
    const separator = globalId.lastIndexOf('#');
    return separator < 0 ? globalId : globalId.slice(separator + 1);
}
function requireIdentifier(value, path, diagnostics) {
    if (!/^[a-z][a-z0-9]*(?:[._-][a-z0-9]+)*$/.test(value)) {
        diagnostics.push(diagnostic('source', 'RULESET_IDENTIFIER_INVALID', path, `invalid identifier ${value}`));
    }
}
function requireExactVersion(value, path, diagnostics) {
    if (parseVersion(value) === undefined) {
        diagnostics.push(diagnostic('source', 'RULESET_VERSION_INVALID', path, `version ${value} is not exact semver`));
    }
}
function requireText(value, path, label, diagnostics) {
    if (value.trim().length === 0) {
        diagnostics.push(diagnostic('source', 'RULESET_TEXT_REQUIRED', path, `${label} is required`));
    }
}
function rejectExecutableValues(value, path, diagnostics, seen) {
    if (typeof value === 'function') {
        diagnostics.push(diagnostic('source', 'RULESET_EXECUTABLE_VALUE_FORBIDDEN', path, 'ruleset manifests may contain immutable declarations only'));
        return;
    }
    if (value === null || typeof value !== 'object' || seen.has(value))
        return;
    seen.add(value);
    if (Array.isArray(value)) {
        for (const [index, child] of value.entries()) {
            rejectExecutableValues(child, `${path}[${index}]`, diagnostics, seen);
        }
        return;
    }
    for (const [key, child] of Object.entries(value)) {
        rejectExecutableValues(child, `${path}.${key}`, diagnostics, seen);
    }
}
function rejectDuplicateValues(values, code, path, label, diagnostics) {
    const seen = new Set();
    for (const [index, value] of values.entries()) {
        if (seen.has(value)) {
            diagnostics.push(diagnostic('source', code, `${path}[${index}]`, `duplicate ${label} ${value}`));
        }
        else {
            seen.add(value);
        }
    }
}
function failed(diagnostics) {
    return immutable({ ok: false, diagnostics: [...diagnostics].sort(compareDiagnostic) });
}
function diagnostic(stage, code, path, message, context = {}) {
    const compactContext = Object.fromEntries(Object.entries(context).filter(([, value]) => value !== undefined));
    return immutable({ stage, severity: 'error', code, path, message, ...compactContext });
}
function supportedRange(range) {
    const candidate = range.startsWith('^') || range.startsWith('~') ? range.slice(1) : range;
    return parseVersion(candidate) !== undefined;
}
function satisfiesVersion(version, range) {
    const actual = parseVersion(version);
    const prefix = range[0];
    const expected = parseVersion(prefix === '^' || prefix === '~' ? range.slice(1) : range);
    if (actual === undefined || expected === undefined)
        return false;
    const comparison = compareSegments(actual, expected);
    if (prefix === '^') {
        if (comparison < 0)
            return false;
        if (expected[0] > 0)
            return actual[0] === expected[0];
        if (expected[1] > 0)
            return actual[0] === 0 && actual[1] === expected[1];
        return compareSegments(actual, expected) === 0;
    }
    if (prefix === '~') {
        return comparison >= 0 && actual[0] === expected[0] && actual[1] === expected[1];
    }
    return comparison === 0;
}
function parseVersion(value) {
    const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(value);
    if (match === null)
        return undefined;
    const major = Number(match[1]);
    const minor = Number(match[2]);
    const patch = Number(match[3]);
    return Number.isSafeInteger(major) && Number.isSafeInteger(minor) && Number.isSafeInteger(patch)
        ? [major, minor, patch]
        : undefined;
}
function compareVersion(left, right) {
    const leftVersion = parseVersion(left) ?? [0, 0, 0];
    const rightVersion = parseVersion(right) ?? [0, 0, 0];
    return compareSegments(leftVersion, rightVersion);
}
function compareSegments(left, right) {
    return left[0] - right[0] || left[1] - right[1] || left[2] - right[2];
}
function identityKey(id, version) {
    return `${id}@${version}`;
}
function compareText(left, right) {
    return left < right ? -1 : left > right ? 1 : 0;
}
function compareIdentity(left, right) {
    return left.id.localeCompare(right.id) || compareVersion(left.version, right.version);
}
function compareLock(left, right) {
    return (left.requester.localeCompare(right.requester) ||
        left.packageId.localeCompare(right.packageId) ||
        left.importAs.localeCompare(right.importAs));
}
function compareRequirement(left, right) {
    return left.id.localeCompare(right.id) || left.version - right.version;
}
function compareRelationship(left, right) {
    return (left.kind.localeCompare(right.kind) ||
        left.source.localeCompare(right.source) ||
        left.target.localeCompare(right.target) ||
        left.order - right.order);
}
function compareDiagnostic(left, right) {
    return left.path.localeCompare(right.path) || left.code.localeCompare(right.code);
}
//# sourceMappingURL=ruleset-compiler.js.map