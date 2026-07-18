import { RPG_CAPABILITY_VERSIONS, RPG_OPERATION_VERSIONS, } from '@asha-rpg/ir';
import { immutable } from './canonical.js';
import { defineActions, definePackage } from './builders.js';
import { normalizePackage } from './normalize.js';
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
        selectedVersionById: new Map(),
        lock: [],
        relationships: [],
    };
    const compositionKey = identityKey(options.composition.identity.id, options.composition.identity.version);
    const base = resolveRequest(context, options.composition.base, compositionKey, 'base', 'contributes', '$.composition.base');
    const additions = options.composition.add.map((request, index) => resolveRequest(context, request, compositionKey, `add:${request.id}`, 'contributes', `$.composition.add[${index}]`));
    const overlays = options.composition.overlays.map((request, index) => resolveRequest(context, request, compositionKey, `overlay:${request.id}`, 'patches', `$.composition.overlays[${index}]`));
    const roots = [base, ...additions, ...overlays].filter((entry) => entry !== undefined);
    resolveDependencies(context, roots);
    validateSelectedPackages(context, target);
    validateDeferredRelationships(context);
    const rootKeys = new Set([base, ...additions]
        .filter((entry) => entry !== undefined)
        .map((entry) => entry.key));
    const graph = closeDefinitionGraph(context, rootKeys);
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
        derivationProvenance: [],
        overlayProvenance: [],
        normalizedIr: normalized,
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
    for (const [index] of composition.overlays.entries()) {
        diagnostics.push(diagnostic('materialization', 'RULESET_OVERLAY_EXECUTION_DEFERRED', `$.composition.overlays[${index}]`, 'overlay records cannot enter an artifact until #5957 materializes them'));
    }
    for (const optionId of Object.keys(composition.configure).sort()) {
        diagnostics.push(diagnostic('materialization', 'RULESET_CONFIGURATION_EXECUTION_DEFERRED', `$.composition.configure.${optionId}`, 'configuration records require an owned materializer before entering an artifact'));
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
function resolveRequest(context, request, requester, importAs, relationship, path) {
    const available = context.availableById.get(request.id) ?? [];
    const compatible = available.filter((source) => satisfiesVersion(source.manifest.identity.version, request.version));
    if (!supportedRange(request.version)) {
        context.diagnostics.push(diagnostic('resolution', 'RULESET_VERSION_RANGE_UNSUPPORTED', `${path}.version`, `unsupported version range ${request.version}`, { packageId: request.id }));
        return undefined;
    }
    const source = compatible[0];
    if (source === undefined) {
        context.diagnostics.push(diagnostic('resolution', 'RULESET_PACKAGE_UNRESOLVED', path, `no package ${request.id} satisfies ${request.version}`, { packageId: request.id, expected: request.version }));
        return undefined;
    }
    const version = source.manifest.identity.version;
    const selectedVersion = context.selectedVersionById.get(request.id);
    if (selectedVersion !== undefined && selectedVersion !== version) {
        context.diagnostics.push(diagnostic('resolution', 'RULESET_MULTIPLE_PACKAGE_VERSIONS', path, `package ${request.id} resolved to both ${selectedVersion} and ${version}`, { packageId: request.id, expected: selectedVersion, actual: version }));
        return undefined;
    }
    context.selectedVersionById.set(request.id, version);
    const key = identityKey(request.id, version);
    let selected = context.selected.get(key);
    if (selected === undefined) {
        selected = { key, source, aliases: new Map() };
        context.selected.set(key, selected);
    }
    context.lock.push({
        requester,
        packageId: request.id,
        requestedVersion: request.version,
        resolvedVersion: version,
        sourceFingerprint: source.sourceFingerprint,
        importAs,
        relationship,
    });
    context.relationships.push({
        kind: relationship,
        source: requester,
        target: key,
        order: context.relationships.length,
    });
    return selected;
}
function resolveDependencies(context, roots) {
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
            const resolved = resolveRequest(context, dependency, entry.key, dependency.importAs, 'dependsOn', path);
            if (resolved !== undefined) {
                entry.aliases.set(dependency.importAs, resolved.key);
                visit(resolved);
            }
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
function validateDeferredRelationships(context) {
    for (const entry of context.selected.values()) {
        for (const [index, relationship] of entry.source.manifest.relationships.entries()) {
            context.diagnostics.push(diagnostic('materialization', 'RULESET_RELATIONSHIP_EXECUTION_DEFERRED', `$.packages[${entry.key}].relationships[${index}]`, `${relationship.kind} records are versioned but cannot enter an artifact until #5957 materializes them`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
        }
    }
}
function closeDefinitionGraph(context, rootKeys) {
    const definitionsByPackage = new Map();
    for (const entry of context.selected.values()) {
        const definitions = new Map();
        const exports = new Set(entry.source.manifest.exports);
        rejectDuplicateValues(entry.source.manifest.definitions.map((definition) => definition.id), 'RULESET_DUPLICATE_LOCAL_DEFINITION', `$.packages[${entry.key}].definitions`, 'definition', context.diagnostics);
        for (const definition of entry.source.manifest.definitions) {
            definitions.set(definition.id, { package: entry, definition, exported: exports.has(definition.id) });
        }
        for (const [index, definitionId] of entry.source.manifest.exports.entries()) {
            if (!definitions.has(definitionId)) {
                context.diagnostics.push(diagnostic('graph', 'RULESET_EXPORT_MISSING', `$.packages[${entry.key}].exports[${index}]`, `export ${definitionId} has no declaration`, { packageId: entry.source.manifest.identity.id, definitionId, source: entry.source.manifest.entry }));
            }
        }
        definitionsByPackage.set(entry.key, definitions);
    }
    const roots = [...rootKeys]
        .flatMap((key) => [...(definitionsByPackage.get(key)?.values() ?? [])]
        .filter((record) => record.exported)
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
        const references = [];
        for (const [index, reference] of record.definition.references.entries()) {
            const target = resolveDefinitionReference(record, reference, index, definitionsByPackage, context.diagnostics);
            if (target !== undefined) {
                references.push(globalDefinitionId(target));
                visit(target);
            }
        }
        visiting.pop();
        reachable.add(globalId);
        resolvedReferences.set(globalId, Object.freeze(references.sort()));
    };
    for (const root of roots) {
        const record = byGlobalId.get(root);
        if (record !== undefined)
            visit(record);
    }
    for (const record of byGlobalId.values()) {
        const globalId = globalDefinitionId(record);
        if (!reachable.has(globalId) && record.definition.visibility === 'public') {
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
function resolveDefinitionReference(source, reference, index, definitionsByPackage, diagnostics) {
    const targetPackageKey = reference.importAs === undefined
        ? source.package.key
        : source.package.aliases.get(reference.importAs);
    const path = `$.packages[${source.package.key}].definitions.${source.definition.id}.references[${index}]`;
    if (targetPackageKey === undefined) {
        diagnostics.push(diagnostic('graph', 'RULESET_IMPORT_ALIAS_UNRESOLVED', path, `import alias ${reference.importAs ?? ''} is not declared`, { definitionId: source.definition.id, source: source.definition.source }));
        return undefined;
    }
    const target = definitionsByPackage.get(targetPackageKey)?.get(reference.definitionId);
    if (target === undefined) {
        diagnostics.push(diagnostic('graph', 'RULESET_DEFINITION_REFERENCE_MISSING', path, `definition ${reference.definitionId} was not found in ${targetPackageKey}`, { definitionId: source.definition.id, source: source.definition.source }));
        return undefined;
    }
    if (targetPackageKey !== source.package.key &&
        (!target.exported || target.definition.visibility === 'private')) {
        diagnostics.push(diagnostic('graph', 'RULESET_PRIVATE_CROSS_PACKAGE_REFERENCE', path, `definition ${target.definition.id} is not exported for cross-package use`, {
            packageId: target.package.source.manifest.identity.id,
            definitionId: target.definition.id,
            source: source.definition.source,
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
    for (const entry of context.selected.values()) {
        for (const requirement of entry.source.manifest.requirements.operations) {
            operations.set(requirement.id, requirement.version);
        }
        for (const requirement of entry.source.manifest.requirements.capabilities) {
            capabilities.set(requirement.id, requirement.version);
        }
    }
    for (const requirement of normalizedRequirements) {
        const declared = requirement.kind === 'operation'
            ? operations.get(requirement.id)
            : capabilities.get(requirement.id);
        if (declared !== requirement.version) {
            context.diagnostics.push(diagnostic('compatibility', 'RULESET_DERIVED_REQUIREMENT_UNDECLARED', '$.requirements', `materialized rules require ${requirement.id}@${requirement.version}, but the package graph did not declare it`, { expected: `${requirement.id}@${requirement.version}` }));
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
        .filter((record) => record.definition.kind !== 'template')
        .map((record) => {
        const definition = record.definition;
        if (definition.kind === 'template') {
            throw new Error(`template ${definition.id} reached direct materialization`);
        }
        const semantic = definition.kind === 'action'
            ? normalizedActions.get(definition.id)
            : definition.semantic;
        if (semantic === undefined)
            throw new Error(`materialization missing ${definition.id}`);
        return {
            id: definition.id,
            kind: definition.kind,
            visibility: rootSet.has(definition.id) ? 'exported' : 'support',
            extensionPolicy: definition.extensionPolicy,
            semantic,
            presentation: definition.presentation ?? null,
            references: (references.get(globalDefinitionId(record)) ?? []).map(localDefinitionId),
            provenance: provenance(record),
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
        return actual[0] === 0 && actual[1] === expected[1];
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