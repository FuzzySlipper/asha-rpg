import { RPG_CAPABILITY_VERSIONS, RPG_OPERATION_VERSIONS, } from '@asha-rpg/ir';
import { canonicalJson, immutable, stableFingerprint } from './canonical.js';
import { defineActions, definePackage } from './builders.js';
import { catalogOwnershipOf } from './catalogs.js';
import { rulesetValueOwnershipOf } from './ruleset-builders.js';
import { normalizeAction, normalizePackage } from './normalize.js';
const NO_DIAGNOSTICS = Object.freeze([]);
export const ASHA_RPG_PLAY_BUNDLE_TARGET = immutable({
    language: { id: 'asha-rpg', version: '1.0.0' },
    operations: { ...RPG_OPERATION_VERSIONS },
    capabilities: { ...RPG_CAPABILITY_VERSIONS },
    models: {
        checks: { 'check.d20-roll-over': 1 },
        turns: { 'turn.ordered-one-action': 1 },
        initiative: { 'initiative.scenario-ordered': 1 },
        reactions: { 'reaction.before-damage-choice': 1 },
        actionEconomy: { 'action-economy.one-action-plus-reaction': 1 },
    },
});
export function preparePlayBundle(options) {
    const target = options.target ?? ASHA_RPG_PLAY_BUNDLE_TARGET;
    const diagnostics = [];
    rejectExecutableValues(options, '$', diagnostics, new WeakSet());
    validatePlayBundle(options.bundle, target, diagnostics);
    validateUniquePackageSources(options.contentPacks, diagnostics);
    const context = {
        diagnostics,
        ruleset: options.bundle.ruleset,
        availableById: indexAvailablePackages(options.contentPacks, diagnostics),
        selected: new Map(),
        lock: [],
        relationships: [],
    };
    const bundleKey = identityKey(options.bundle.identity.id, options.bundle.identity.version);
    const resolved = resolvePackageGraph(context, options.bundle, bundleKey);
    if (resolved === undefined)
        return failed(diagnostics);
    const { base, additions } = resolved;
    validateSelectedPackages(context, options.bundle.ruleset);
    const rootKeys = new Set([base, ...additions]
        .filter((entry) => entry !== undefined)
        .map((entry) => entry.key));
    const materialization = materializeSelectedDefinitions(context, options.bundle, resolved.overlays);
    if (materialization === undefined)
        return failed(diagnostics);
    const graph = closeDefinitionGraph(context, rootKeys, materialization.records);
    if (diagnostics.length > 0 || graph === undefined)
        return failed(diagnostics);
    validateParticipantProfiles(graph.materialized, graph.resolvedReferences, options.bundle.ruleset, diagnostics);
    if (diagnostics.length > 0)
        return failed(diagnostics);
    const normalized = normalizeMaterializedActions(options.bundle, graph, diagnostics);
    if (normalized === undefined)
        return failed(diagnostics);
    const requirements = collectRequirements(context, normalized, graph.materialized, options.bundle.ruleset);
    validateCollectedRequirements(requirements, options.bundle.ruleset, diagnostics);
    const policyBindings = [...context.selected.values()]
        .flatMap((entry) => entry.source.manifest.policyBindings)
        .sort((left, right) => left.id.localeCompare(right.id));
    rejectDuplicateValues(policyBindings.map((binding) => binding.id), 'CONTENT_PACK_DUPLICATE_POLICY_BINDING', '$.compiledPolicyBindings', 'policy binding', diagnostics);
    if (diagnostics.length > 0 || requirements === undefined)
        return failed(diagnostics);
    const definitionProvenance = graph.materialized
        .map((record) => provenance(record))
        .sort((left, right) => left.definitionId.localeCompare(right.definitionId));
    const definitionCommitments = selectedDefinitionCommitments(materialization.definitionCommitments, materialization.derivationProvenance, materialization.overlayProvenance);
    const materializedDefinitions = materializeDefinitions(graph.materialized, graph.resolvedReferences, graph.exportedRoots, normalized.actions);
    const relationships = [
        ...context.relationships,
        ...materialization.relationships,
        ...graph.exportedRoots.map((definitionId, order) => ({
            kind: 'exports',
            source: graph.byGlobalId.get(definitionId)?.package.key ?? bundleKey,
            target: definitionId,
            order,
        })),
        ...Object.entries(options.bundle.configure)
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([optionId, value], order) => ({
            kind: 'configures',
            source: bundleKey,
            target: `${optionId}=${String(value)}`,
            order,
        })),
    ].sort(compareRelationship);
    const prepared = immutable({
        schema: { identity: 'asha.rpg.play-bundle.prepared', major: 1 },
        playBundleIdentity: options.bundle.identity,
        ruleset: options.bundle.ruleset,
        contentPacks: [...context.selected.values()]
            .map((entry) => ({
            id: entry.source.manifest.identity.id,
            version: entry.source.manifest.identity.version,
            sourceFingerprint: entry.source.sourceFingerprint,
        }))
            .sort(compareIdentity),
        dependencyLock: [...context.lock].sort(compareLock),
        contentRequirements: requirements,
        exportedRoots: graph.exportedRoots,
        materializedDefinitions,
        compiledPolicyBindings: policyBindings,
        definitionProvenance,
        definitionCommitments,
        relationships,
        derivationProvenance: materialization.derivationProvenance,
        overlayProvenance: materialization.overlayProvenance,
    });
    return immutable({ ok: true, prepared, diagnostics: NO_DIAGNOSTICS });
}
function validatePlayBundle(bundle, target, diagnostics) {
    requireIdentifier(bundle.identity.id, '$.bundle.identity.id', diagnostics);
    requireExactVersion(bundle.identity.version, '$.bundle.identity.version', diagnostics);
    validateRuleset(bundle.ruleset, target, diagnostics);
    for (const optionId of Object.keys(bundle.configure).sort()) {
        requireIdentifier(optionId, `$.bundle.configure.${optionId}`, diagnostics);
    }
}
function validateRuleset(ruleset, target, diagnostics) {
    if (ruleset.schema.identity !== 'asha.rpg.ruleset' ||
        ruleset.schema.major !== 1) {
        diagnostics.push(diagnostic('compatibility', 'RULESET_SCHEMA_UNSUPPORTED', '$.bundle.ruleset.schema', 'the ruleset schema must be asha.rpg.ruleset@1'));
    }
    for (const model of [
        'checks',
        'turns',
        'initiative',
        'reactions',
        'actionEconomy',
    ]) {
        const binding = ruleset.models[model];
        if (target.models[model][binding.id] === binding.version)
            continue;
        diagnostics.push(diagnostic('compatibility', 'RULESET_MODEL_UNSUPPORTED', `$.bundle.ruleset.models.${model}`, `ruleset model ${binding.id}@${binding.version} is not bound by Rust authority`));
    }
    requireIdentifier(ruleset.identity.id, '$.bundle.ruleset.identity.id', diagnostics);
    requireExactVersion(ruleset.identity.version, '$.bundle.ruleset.identity.version', diagnostics);
    if (ruleset.language.id !== target.language.id ||
        !satisfiesVersion(target.language.version, ruleset.language.version)) {
        diagnostics.push(diagnostic('compatibility', 'RULESET_LANGUAGE_INCOMPATIBLE', '$.bundle.ruleset.language', `the ruleset requires ${ruleset.language.id}@${ruleset.language.version}`, {
            expected: `${target.language.id}@${target.language.version}`,
            actual: `${ruleset.language.id}@${ruleset.language.version}`,
        }));
    }
    const operationIds = new Set();
    for (const [index, provision] of ruleset.provides.operations.entries()) {
        if (target.operations[provision.id] !== provision.version ||
            !operationIds.add(provision.id)) {
            diagnostics.push(diagnostic('compatibility', 'RULESET_OPERATION_PROVISION_INVALID', `$.bundle.ruleset.provides.operations[${index}]`, `ruleset operation provision ${provision.id}@${provision.version} is unsupported or duplicated`));
        }
    }
    const capabilityIds = new Set();
    for (const [index, provision] of ruleset.provides.capabilities.entries()) {
        if (target.capabilities[provision.id] !== provision.version ||
            !capabilityIds.add(provision.id)) {
            diagnostics.push(diagnostic('compatibility', 'RULESET_CAPABILITY_PROVISION_INVALID', `$.bundle.ruleset.provides.capabilities[${index}]`, `ruleset capability provision ${provision.id}@${provision.version} is unsupported or duplicated`));
        }
    }
    const valueIds = new Set();
    const declaredDomains = new Set(ruleset.provides.numericDomains.map((domain) => domain.id));
    for (const [index, provision] of ruleset.provides.values.entries()) {
        const key = `${provision.kind}:${provision.id}`;
        if (!valueIds.add(key) ||
            provision.label.trim().length === 0 ||
            !declaredDomains.has(provision.numericDomainId)) {
            diagnostics.push(diagnostic('source', 'RULESET_VALUE_PROVISION_INVALID', `$.bundle.ruleset.provides.values[${index}]`, `ruleset value provision ${key} must be unique, labelled, and use a declared numeric domain`));
        }
    }
    const domainIds = new Set();
    for (const [index, provision] of ruleset.provides.numericDomains.entries()) {
        if (!domainIds.add(provision.id) ||
            provision.minimum > provision.maximum) {
            diagnostics.push(diagnostic('source', 'RULESET_NUMERIC_DOMAIN_INVALID', `$.bundle.ruleset.provides.numericDomains[${index}]`, `ruleset numeric domain ${provision.id} must be unique and ordered`));
        }
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
        diagnostics.push(diagnostic('source', 'CONTENT_PACK_DUPLICATE_PACKAGE_IDENTITY', `$.packages[${index}]`, `duplicate package source ${key}`, { packageId: identity.id }));
    }
}
function resolvePackageGraph(context, bundle, bundleKey) {
    const rootConstraints = [
        {
            request: bundle.base,
            requester: bundleKey,
            importAs: 'base',
            relationship: 'contributes',
            path: '$.bundle.base',
        },
        ...bundle.add.map((request, index) => ({
            request,
            requester: bundleKey,
            importAs: `add:${request.id}`,
            relationship: 'contributes',
            path: `$.bundle.add[${index}]`,
        })),
        ...bundle.overlays.map((request, index) => ({
            request,
            requester: bundleKey,
            importAs: `overlay:${request.id}`,
            relationship: 'patches',
            path: `$.bundle.overlays[${index}]`,
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
            context.diagnostics.push(diagnostic('resolution', 'CONTENT_PACK_PACKAGE_UNRESOLVED', first.path, `no package ${first.request.id} satisfies all constraints: ${expected}`, { packageId: first.request.id, expected }));
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
    const base = selectedForRequest(bundle.base);
    const additions = bundle.add.map(selectedForRequest);
    const overlays = bundle.overlays.map(selectedForRequest);
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
    diagnostics.push(diagnostic('resolution', 'CONTENT_PACK_VERSION_RANGE_UNSUPPORTED', `${constraint.path}.version`, `unsupported version range ${constraint.request.version}`, { packageId: constraint.request.id }));
}
function resolveDependencies(context, roots, selectedById) {
    const visiting = [];
    const visited = new Set();
    const visit = (entry) => {
        const cycleStart = visiting.indexOf(entry.key);
        if (cycleStart >= 0) {
            const graphPath = [...visiting.slice(cycleStart), entry.key];
            context.diagnostics.push(diagnostic('resolution', 'CONTENT_PACK_DEPENDENCY_CYCLE', '$.dependencyGraph', `dependency cycle: ${graphPath.join(' -> ')}`, { graphPath }));
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
                context.diagnostics.push(diagnostic('source', 'CONTENT_PACK_DUPLICATE_IMPORT_ALIAS', `${path}.importAs`, `duplicate import alias ${dependency.importAs}`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
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
function validateSelectedPackages(context, ruleset) {
    for (const entry of context.selected.values()) {
        const manifest = entry.source.manifest;
        if (manifest.language.id !== ruleset.language.id ||
            !satisfiesVersion(ruleset.language.version, manifest.language.version)) {
            context.diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_LANGUAGE_INCOMPATIBLE', `$.packages[${entry.key}].language`, `${entry.key} requires ${manifest.language.id}@${manifest.language.version}`, {
                packageId: manifest.identity.id,
                expected: `${ruleset.language.id}@${ruleset.language.version}`,
                actual: `${manifest.language.id}@${manifest.language.version}`,
                source: manifest.entry,
            }));
        }
        validateRequirements(entry, ruleset, context.diagnostics);
        const exported = new Set(manifest.exports);
        for (const definition of manifest.definitions) {
            validateRulesetValueOwners({
                package: entry,
                definition,
                exported: exported.has(definition.id),
            }, ruleset, context.diagnostics);
        }
    }
}
function validateRequirements(entry, ruleset, diagnostics) {
    for (const [index, requirement] of entry.source.manifest.requirements.operations.entries()) {
        const supported = ruleset.provides.operations.find((candidate) => candidate.id === requirement.id)?.version;
        if (supported !== requirement.version) {
            diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_OPERATION_REQUIREMENT_MISSING', `$.packages[${entry.key}].requirements.operations[${index}]`, `content pack requires operation ${requirement.id}@${requirement.version}, which the ruleset does not provide`, {
                packageId: entry.source.manifest.identity.id,
                expected: supported === undefined ? 'unavailable' : String(supported),
                actual: String(requirement.version),
                source: entry.source.manifest.entry,
            }));
        }
    }
    for (const [index, requirement] of entry.source.manifest.requirements.capabilities.entries()) {
        const supported = ruleset.provides.capabilities.find((candidate) => candidate.id === requirement.id)?.version;
        if (supported !== requirement.version) {
            diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_CAPABILITY_REQUIREMENT_MISSING', `$.packages[${entry.key}].requirements.capabilities[${index}]`, `content pack requires capability ${requirement.id}@${requirement.version}, which the ruleset does not provide`, {
                packageId: entry.source.manifest.identity.id,
                expected: supported === undefined ? 'unavailable' : String(supported),
                actual: String(requirement.version),
                source: entry.source.manifest.entry,
            }));
        }
    }
    const values = new Set(ruleset.provides.values.map((value) => `${value.kind}:${value.id}`));
    for (const [index, requirement] of entry.source.manifest.requirements.values.entries()) {
        const key = `${requirement.kind}:${requirement.id}`;
        if (!values.has(key)) {
            diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_VALUE_REQUIREMENT_MISSING', `$.packages[${entry.key}].requirements.values[${index}]`, `content pack requires ${key}, which the ruleset does not provide`, { packageId: entry.source.manifest.identity.id, expected: key, actual: 'unavailable' }));
        }
    }
    const domains = new Set(ruleset.provides.numericDomains.map((domain) => domain.id));
    for (const [index, requirement] of entry.source.manifest.requirements.numericDomains.entries()) {
        if (!domains.has(requirement)) {
            diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_NUMERIC_DOMAIN_REQUIREMENT_MISSING', `$.packages[${entry.key}].requirements.numericDomains[${index}]`, `content pack requires numeric domain ${requirement}, which the ruleset does not provide`, { packageId: entry.source.manifest.identity.id, expected: requirement, actual: 'unavailable' }));
        }
    }
}
function materializeSelectedDefinitions(context, bundle, overlayPackages) {
    const definitionsByPackage = new Map();
    for (const entry of context.selected.values()) {
        rejectDuplicateValues(entry.source.manifest.definitions.map((definition) => definition.id), 'CONTENT_PACK_DUPLICATE_LOCAL_DEFINITION', `$.packages[${entry.key}].definitions`, 'definition', context.diagnostics);
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
            context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_EXPORT_MISSING', `$.packages[${entry.key}].exports[${index}]`, `export ${definitionId} has no declaration`, { packageId: entry.source.manifest.identity.id, definitionId, source: entry.source.manifest.entry }));
        }
        definitionsByPackage.set(entry.key, definitions);
    }
    const derivationsByDefinition = new Map();
    for (const entry of context.selected.values()) {
        for (const [index, relationship] of entry.source.manifest.relationships.entries()) {
            if (relationship.version !== 1) {
                context.diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_RELATIONSHIP_VERSION_UNSUPPORTED', `$.packages[${entry.key}].relationships[${index}].version`, `${relationship.kind} relationship version ${String(relationship.version)} is unsupported`, { packageId: entry.source.manifest.identity.id }));
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
    const definitionCommitments = new Map();
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_CYCLE', '$.derivationGraph', `derivation cycle: ${graphPath.join(' -> ')}`, { definitionId: record.definition.id, source: record.definition.source, graphPath }));
            return undefined;
        }
        if (visiting.length >= 32) {
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_DEPTH_EXCEEDED', '$.derivationGraph', `derivation depth exceeds the supported limit of 32 at ${key}`, { definitionId: record.definition.id, source: record.definition.source, graphPath: [...visiting, key] }));
            return undefined;
        }
        if (record.definition.kind === 'action' || record.definition.kind === 'support') {
            if ((derivationsByDefinition.get(key)?.length ?? 0) > 0) {
                context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_DECLARATION_INCOMPATIBLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, 'a derivesFrom relationship must name a derived definition declaration', { definitionId: record.definition.id, source: record.definition.source }));
                return undefined;
            }
            records.set(key, record);
            definitionCommitments.set(key, concreteDefinitionCommitment(record, bundle.ruleset));
            return record;
        }
        if (record.definition.kind === 'mixin' || record.definition.kind === 'template') {
            return undefined;
        }
        const derivations = derivationsByDefinition.get(key) ?? [];
        if (derivations.length !== 1) {
            context.diagnostics.push(diagnostic('materialization', derivations.length === 0
                ? 'CONTENT_PACK_DERIVATION_BASE_MISSING'
                : 'CONTENT_PACK_DERIVATION_BASE_AMBIGUOUS', `$.packages[${record.package.key}].definitions.${record.definition.id}`, derivations.length === 0
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].relationships.${record.definition.id}.target`, `derived ${record.definition.materializesAs} cannot use ${baseSource.definition.kind} base ${baseSource.definition.id}`, {
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_BASE_FORBIDDEN', `$.packages[${record.package.key}].relationships.${record.definition.id}.target`, `definition ${base.definition.id} is ${base.definition.extensionPolicy}, not derivable`, { definitionId: base.definition.id, source: record.definition.source }));
            visiting.pop();
            return undefined;
        }
        if (record.definition.materializesAs !== base.definition.kind) {
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVATION_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].definitions.${record.definition.id}.materializesAs`, `derived ${record.definition.materializesAs} cannot use ${base.definition.kind} base ${base.definition.id}`, { definitionId: record.definition.id, source: record.definition.source }));
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
                    context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_MIXIN_KIND_INCOMPATIBLE', `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}]`, `definition ${mixinRecord.definition.id} is not a mixin`, { definitionId: mixinRecord.definition.id, source: record.definition.source }));
                }
                continue;
            }
            const parameters = resolveMixinParameters(mixinRecord.definition, application.parameters, `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}].parameters`, context.diagnostics);
            if (parameters === undefined)
                continue;
            definitionCommitments.set(globalDefinitionId(mixinRecord), mixinDefinitionCommitment(mixinRecord));
            for (const referenceId of resolveMaterializationReferenceIds(mixinRecord, definitionsByPackage, context.diagnostics)) {
                inheritedReferenceIds.add(referenceId);
            }
            const applied = applyContentPatch(current, mixinRecord.definition.patch, parameters, `$.packages[${record.package.key}].relationships.${record.definition.id}.mixins[${order}].patch`, context.diagnostics);
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
        const local = applyContentPatch(current, derivation.localPatch, {}, `$.packages[${record.package.key}].relationships.${record.definition.id}.localPatch`, context.diagnostics);
        if (local !== undefined) {
            current = local;
            changes.push(...local.changes);
        }
        const concrete = concreteDerivedRecord(record, base, current, [...inheritedReferenceIds].sort(), context.diagnostics);
        visiting.pop();
        if (concrete === undefined)
            return undefined;
        records.set(key, concrete);
        definitionCommitments.set(key, concreteDefinitionCommitment(concrete, bundle.ruleset));
        const baseIdentity = base.package.source.manifest.identity;
        const identity = record.package.source.manifest.identity;
        derivationProvenance.push({
            definitionId: record.definition.id,
            packageId: identity.id,
            packageVersion: identity.version,
            baseDefinitionId: base.definition.id,
            basePackageId: baseIdentity.id,
            basePackageVersion: baseIdentity.version,
            baseFingerprint: definitionMaterializationFingerprint(base, bundle.ruleset),
            base: definitionMaterializationStage(base, bundle.ruleset),
            mixins: mixinProvenance,
            localPatchFingerprint: stableFingerprint(derivation.localPatch),
            localPatch: derivation.localPatch,
            materializedFingerprint: definitionMaterializationFingerprint(concrete, bundle.ruleset),
            materialized: definitionMaterializationStage(concrete, bundle.ruleset),
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
                context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_PUBLIC_DEFINITION_UNREACHABLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `public template ${record.definition.id} has no materialized definition`, {
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_PACKAGE_NOT_SELECTED', `$.packages[${entry.key}].relationships`, `package ${entry.key} declares patches but is not selected in bundle overlay order`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
        }
    }
    for (const [overlayOrder, entry] of overlayPackages.entries()) {
        const relationships = entry.source.manifest.relationships.filter((relationship) => relationship.kind === 'patches');
        if (relationships.length === 0) {
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_EMPTY', `$.bundle.overlays[${overlayOrder}]`, `selected overlay ${entry.key} declares no patch relationships`, { packageId: entry.source.manifest.identity.id, source: entry.source.manifest.entry }));
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
                context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_TARGET_PACKAGE_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].targetPackage`, `overlay pins ${relationship.targetPackage.id}@${relationship.targetPackage.version}, resolved ${targetIdentity.id}@${targetIdentity.version}`, { definitionId: target.definition.id, expected: `${relationship.targetPackage.id}@${relationship.targetPackage.version}`, actual: `${targetIdentity.id}@${targetIdentity.version}` }));
                continue;
            }
            if (target.definition.extensionPolicy !== 'patchable') {
                context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_TARGET_FORBIDDEN', `$.packages[${entry.key}].relationships[${relationshipOrder}].target`, `definition ${target.definition.id} is ${target.definition.extensionPolicy}, not patchable`, { definitionId: target.definition.id, source: entry.source.manifest.entry }));
                continue;
            }
            const before = definitionMaterializationStage(target, bundle.ruleset);
            const beforeFingerprint = stableFingerprint(before);
            if (beforeFingerprint !== relationship.expectedFingerprint) {
                context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_EXPECTED_FINGERPRINT_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].expectedFingerprint`, `overlay expected ${relationship.expectedFingerprint}, materialized target is ${beforeFingerprint}`, { definitionId: target.definition.id, expected: relationship.expectedFingerprint, actual: beforeFingerprint }));
                continue;
            }
            if (!patchMatchesPlane(relationship.patch, relationship.plane)) {
                context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_IMPACT_PLANE_MISMATCH', `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, `overlay patch operations exceed declared ${relationship.plane} impact plane`, { definitionId: target.definition.id }));
                continue;
            }
            let conflicted = false;
            for (const operation of relationship.patch.operations) {
                const write = `${target.definition.id}:${operation.plane}:${patchPath(operation.path)}`;
                if (writes.has(write) && relationship.conflictPolicy === 'reject') {
                    conflicted = true;
                    context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_OVERLAY_WRITE_CONFLICT', `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, `overlay write conflicts at ${write}`, { definitionId: target.definition.id }));
                }
                writes.add(write);
            }
            if (conflicted)
                continue;
            const applied = applyContentPatch(definitionValue(target), relationship.patch, {}, `$.packages[${entry.key}].relationships[${relationshipOrder}].patch`, context.diagnostics);
            if (applied === undefined)
                continue;
            const patched = replaceConcreteRecordValue(target, applied, context.diagnostics);
            if (patched === undefined)
                continue;
            records.set(globalDefinitionId(target), patched);
            const afterFingerprint = definitionMaterializationFingerprint(patched, bundle.ruleset);
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
    for (const [optionOrder, [optionId, selectedValue]] of Object.entries(bundle.configure)
        .sort(([left], [right]) => left.localeCompare(right))
        .entries()) {
        const matches = [...context.selected.values()].flatMap((entry) => entry.source.manifest.relationships
            .filter((relationship) => relationship.kind === 'configures' &&
            relationship.optionId === optionId &&
            relationship.value === selectedValue)
            .map((relationship) => ({ entry, relationship })));
        if (matches.length !== 1) {
            context.diagnostics.push(diagnostic('materialization', matches.length === 0
                ? 'CONTENT_PACK_CONFIGURATION_OPTION_UNAVAILABLE'
                : 'CONTENT_PACK_CONFIGURATION_OPTION_AMBIGUOUS', `$.bundle.configure.${optionId}`, matches.length === 0
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_CONFIGURATION_TARGET_FORBIDDEN', `$.packages[${match.entry.key}].relationships.${optionId}.target`, `definition ${target.definition.id} is ${target.definition.extensionPolicy}, not configurable`, { definitionId: target.definition.id }));
            continue;
        }
        const applied = applyContentPatch(definitionValue(target), match.relationship.patch, {}, `$.packages[${match.entry.key}].relationships.${optionId}.patch`, context.diagnostics);
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
        definitionCommitments: [...definitionCommitments.values()].sort(compareDefinitionCommitment),
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
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_IMPORT_ALIAS_UNRESOLVED', path, `import alias ${reference.importAs ?? ''} is not declared`, { packageId: sourcePackage.source.manifest.identity.id, source: sourcePackage.source.manifest.entry }));
        return undefined;
    }
    const target = definitionsByPackage.get(targetPackageKey)?.get(reference.definitionId);
    if (target === undefined) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DEFINITION_REFERENCE_MISSING', path, `definition ${reference.definitionId} was not found in ${targetPackageKey}`, { packageId: sourcePackage.source.manifest.identity.id, definitionId: reference.definitionId, source: sourcePackage.source.manifest.entry }));
        return undefined;
    }
    if (targetPackageKey !== sourcePackage.key &&
        (!target.exported || target.definition.visibility === 'private')) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PRIVATE_CROSS_PACKAGE_REFERENCE', path, `definition ${target.definition.id} is not exported for cross-package use`, { packageId: target.package.source.manifest.identity.id, definitionId: target.definition.id, source: sourcePackage.source.manifest.entry }));
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
            diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVED_ACTION_INVALID', '$.semantic', 'derived action semantic value must be an object', { definitionId: derived.definition.id }));
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
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_DERIVED_SUPPORT_INVALID', '$.semantic', 'derived support semantic value must be an object', { definitionId: derived.definition.id }));
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
    diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_TARGET_INCOMPATIBLE', '$.patch', `definition ${record.definition.id} is not patchable materialized content`, { definitionId: record.definition.id }));
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
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_MIXIN_PARAMETER_UNKNOWN', `${path}.${parameterId}`, `mixin ${mixin.id} does not declare parameter ${parameterId}`, { definitionId: mixin.id }));
    }
    for (const parameter of mixin.parameters) {
        const value = supplied[parameter.id] ?? parameter.default;
        if (value === undefined) {
            diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_MIXIN_PARAMETER_MISSING', `${path}.${parameter.id}`, `mixin parameter ${parameter.id} is required`, { definitionId: mixin.id }));
            continue;
        }
        if (typeof value !== parameter.type) {
            diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_MIXIN_PARAMETER_TYPE_MISMATCH', `${path}.${parameter.id}`, `mixin parameter ${parameter.id} must be ${parameter.type}`, { definitionId: mixin.id, expected: parameter.type, actual: typeof value }));
            continue;
        }
        resolved[parameter.id] = value;
    }
    return diagnostics.length > 0 ? undefined : immutable(resolved);
}
function applyContentPatch(value, patch, parameters, path, diagnostics) {
    if (patch.version !== 1) {
        diagnostics.push(diagnostic('compatibility', 'CONTENT_PACK_PATCH_VERSION_UNSUPPORTED', `${path}.version`, `patch version ${String(patch.version)} is unsupported`));
        return undefined;
    }
    let semantic = cloneJsonValue(value.semantic);
    let presentation = cloneJsonValue(value.presentation ?? {});
    const changes = [];
    for (const [index, operation] of patch.operations.entries()) {
        const operationPath = `${path}.operations[${index}]`;
        const root = operation.plane === 'semantic' ? semantic : presentation;
        if (operation.kind === 'upsertScalar' &&
            !isSupportedPatchUpsert(operation.plane, operation.path)) {
            diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_UPSERT_UNSUPPORTED', operationPath, `upsertScalar is not supported at ${operation.plane}.${patchPath(operation.path)}`));
            continue;
        }
        const before = cloneJsonValue(operation.kind === 'upsertScalar'
            ? readUpsertPatchPath(root, operation.path, operationPath, diagnostics)
            : readPatchPath(root, operation.path, operationPath, diagnostics));
        if (operation.kind === 'setScalar' || operation.kind === 'upsertScalar') {
            const replacement = resolvePatchScalar(operation.value, parameters, operationPath, diagnostics);
            if (replacement === undefined && operation.value !== null)
                continue;
            const written = operation.kind === 'upsertScalar'
                ? writeUpsertPatchPath(root, operation.path, replacement ?? null, operationPath, diagnostics)
                : writePatchPath(root, operation.path, replacement ?? null, operationPath, diagnostics);
            if (!written)
                continue;
        }
        else if (operation.kind === 'adjustNumber') {
            const current = readPatchPath(root, operation.path, operationPath, diagnostics);
            const multiply = resolvePatchNumber(operation.multiply, parameters, operationPath, diagnostics);
            const add = resolvePatchNumber(operation.add, parameters, operationPath, diagnostics);
            if (typeof current !== 'number' || multiply === undefined || add === undefined) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_NUMBER_REQUIRED', operationPath, `adjustNumber requires a numeric target at ${patchPath(operation.path)}`));
                continue;
            }
            if (!writePatchPath(root, operation.path, current * multiply + add, operationPath, diagnostics))
                continue;
        }
        else if (operation.kind === 'appendMember') {
            const target = readPatchPath(root, operation.path, operationPath, diagnostics);
            if (!Array.isArray(target)) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_LIST_REQUIRED', operationPath, `appendMember requires a list at ${patchPath(operation.path)}`));
                continue;
            }
            if (target.some((entry) => isRecord(entry) && entry[operation.identity.key] === operation.identity.value)) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_MEMBER_DUPLICATE', operationPath, `member ${operation.identity.key}=${operation.identity.value} already exists`));
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
                    diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_ANCHOR_MISSING', operationPath, `anchor ${patchSegment(position.anchor)} is missing`));
                    continue;
                }
                target.splice(position.kind === 'before' ? anchorIndex : anchorIndex + 1, 0, member);
            }
        }
        else {
            const target = readPatchPath(root, operation.path, operationPath, diagnostics);
            if (!Array.isArray(target)) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_LIST_REQUIRED', operationPath, `removeMember requires a list at ${patchPath(operation.path)}`));
                continue;
            }
            const indexes = target
                .map((entry, memberIndex) => memberMatches(entry, operation.identity) ? memberIndex : -1)
                .filter((memberIndex) => memberIndex >= 0);
            if (indexes.length !== 1) {
                diagnostics.push(diagnostic('materialization', indexes.length === 0 ? 'CONTENT_PACK_PATCH_MEMBER_MISSING' : 'CONTENT_PACK_PATCH_MEMBER_AMBIGUOUS', operationPath, `member ${patchSegment(operation.identity)} must resolve exactly once`));
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
function readUpsertPatchPath(root, path, diagnosticPath, diagnostics) {
    if (path.length === 0) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_ROOT_WRITE_FORBIDDEN', diagnosticPath, 'patch operations must name a field or stable member'));
        return undefined;
    }
    const parent = readPatchPath(root, path.slice(0, -1), diagnosticPath, diagnostics);
    const leaf = path[path.length - 1];
    if (leaf?.kind !== 'field' || !isRecord(parent)) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_PATH_MISSING', diagnosticPath, `upsert field is invalid at ${patchPath(path)}`));
        return undefined;
    }
    return leaf.name in parent ? parent[leaf.name] : null;
}
function readPatchPath(root, path, diagnosticPath, diagnostics) {
    let current = root;
    for (const segment of path) {
        if (segment.kind === 'field') {
            if (!isRecord(current) || !(segment.name in current)) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_PATH_MISSING', diagnosticPath, `field ${segment.name} is missing at ${patchPath(path)}`));
                return undefined;
            }
            current = current[segment.name];
        }
        else {
            if (!Array.isArray(current)) {
                diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_LIST_REQUIRED', diagnosticPath, `member selector ${patchSegment(segment)} requires a list`));
                return undefined;
            }
            const matches = current.filter((entry) => memberMatches(entry, segment));
            if (matches.length !== 1) {
                diagnostics.push(diagnostic('materialization', matches.length === 0 ? 'CONTENT_PACK_PATCH_MEMBER_MISSING' : 'CONTENT_PACK_PATCH_MEMBER_AMBIGUOUS', diagnosticPath, `member ${patchSegment(segment)} must resolve exactly once`));
                return undefined;
            }
            current = matches[0];
        }
    }
    return current;
}
function writePatchPath(root, path, value, diagnosticPath, diagnostics) {
    if (path.length === 0) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_ROOT_WRITE_FORBIDDEN', diagnosticPath, 'patch operations must name a field or stable member'));
        return false;
    }
    const parentPath = path.slice(0, -1);
    const parent = readPatchPath(root, parentPath, diagnosticPath, diagnostics);
    const leaf = path[path.length - 1];
    if (leaf?.kind !== 'field' || !isRecord(parent) || !(leaf.name in parent)) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_PATH_MISSING', diagnosticPath, `writable field is missing at ${patchPath(path)}`));
        return false;
    }
    parent[leaf.name] = value;
    return true;
}
function writeUpsertPatchPath(root, path, value, diagnosticPath, diagnostics) {
    if (path.length === 0) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_ROOT_WRITE_FORBIDDEN', diagnosticPath, 'patch operations must name a field or stable member'));
        return false;
    }
    const parent = readPatchPath(root, path.slice(0, -1), diagnosticPath, diagnostics);
    const leaf = path[path.length - 1];
    if (leaf?.kind !== 'field' || !isRecord(parent)) {
        diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_PATH_MISSING', diagnosticPath, `upsert field is invalid at ${patchPath(path)}`));
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
    diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_PARAMETER_UNRESOLVED', path, `parameter ${value.parameter} is not supplied`));
    return undefined;
}
function resolvePatchNumber(value, parameters, path, diagnostics) {
    if (typeof value === 'number')
        return value;
    const resolved = parameters[value.parameter];
    if (typeof resolved === 'number')
        return resolved;
    diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_PATCH_NUMBER_PARAMETER_UNRESOLVED', path, `numeric parameter ${value.parameter} is not supplied`));
    return undefined;
}
function isParameterReference(value) {
    return isRecord(value) && typeof value['parameter'] === 'string';
}
function patchMatchesPlane(patch, plane) {
    return patch.operations.every((operation) => plane === 'both' || operation.plane === plane);
}
function isSupportedPatchUpsert(plane, path) {
    return (plane === 'presentation' &&
        path.length === 1 &&
        path[0]?.kind === 'field' &&
        path[0].name === 'description');
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
function definitionMaterializationFingerprint(record, ruleset) {
    return stableFingerprint(definitionMaterializationStage(record, ruleset));
}
function definitionMaterializationStage(record, ruleset) {
    if (record.definition.kind !== 'action' && record.definition.kind !== 'support') {
        throw new Error(`definition ${record.definition.id} is not concrete`);
    }
    return {
        id: record.definition.id,
        kind: record.definition.kind,
        extensionPolicy: record.definition.extensionPolicy,
        value: normalizedDefinitionValue(record),
        references: materializationReferenceIds(record, ruleset),
    };
}
function concreteDefinitionCommitment(record, ruleset) {
    const stage = definitionMaterializationStage(record, ruleset);
    const identity = record.package.source.manifest.identity;
    return immutable({
        kind: 'concrete',
        packageId: identity.id,
        packageVersion: identity.version,
        packageSourceFingerprint: record.package.source.sourceFingerprint,
        definitionId: record.definition.id,
        fingerprint: stableFingerprint(stage),
        stage,
    });
}
function mixinDefinitionCommitment(record) {
    if (record.definition.kind !== 'mixin') {
        throw new Error(`definition ${record.definition.id} is not a mixin`);
    }
    const identity = record.package.source.manifest.identity;
    const value = immutable({
        parameters: [...record.definition.parameters].sort((left, right) => left.id.localeCompare(right.id)),
        patch: record.definition.patch,
    });
    return immutable({
        kind: 'mixin',
        packageId: identity.id,
        packageVersion: identity.version,
        packageSourceFingerprint: record.package.source.sourceFingerprint,
        definitionId: record.definition.id,
        fingerprint: stableFingerprint(value),
        value,
    });
}
function selectedDefinitionCommitments(commitments, derivations, overlays) {
    const selected = new Set();
    for (const provenance of derivations) {
        selected.add(definitionCommitmentIdentity(provenance.packageId, provenance.packageVersion, provenance.definitionId));
        selected.add(definitionCommitmentIdentity(provenance.basePackageId, provenance.basePackageVersion, provenance.baseDefinitionId));
        for (const mixin of provenance.mixins) {
            selected.add(definitionCommitmentIdentity(mixin.packageId, mixin.packageVersion, mixin.definitionId));
        }
    }
    for (const provenance of overlays) {
        selected.add(definitionCommitmentIdentity(provenance.targetPackageId, provenance.targetPackageVersion, provenance.targetDefinitionId));
    }
    return immutable(commitments
        .filter((commitment) => selected.has(definitionCommitmentIdentity(commitment.packageId, commitment.packageVersion, commitment.definitionId)))
        .sort(compareDefinitionCommitment));
}
function compareDefinitionCommitment(left, right) {
    return definitionCommitmentIdentity(left.packageId, left.packageVersion, left.definitionId).localeCompare(definitionCommitmentIdentity(right.packageId, right.packageVersion, right.definitionId));
}
function definitionCommitmentIdentity(packageId, packageVersion, definitionId) {
    return `${packageId}@${packageVersion}#${definitionId}`;
}
export function contentDefinitionMaterializationFingerprint(definition) {
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
function materializationReferenceIds(record, ruleset) {
    const authoredReferences = authoredCatalogReferences(record.definition)
        .filter((reference) => !(reference.packageId === undefined &&
        (reference.category === 'stat' || reference.category === 'defense') &&
        ruleset?.provides.values.some((value) => value.kind === reference.category &&
            value.id === reference.definitionId)))
        .map((reference) => reference.definitionId);
    return [
        ...new Set([
            ...(record.definition.lowLevelReferences ?? []).map((reference) => reference.definitionId),
            ...authoredReferences,
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
    const byIdentity = new Map();
    if (definition.kind === 'action') {
        collectCatalogReferences(definition.action, '$.action', byIdentity);
    }
    else if (definition.kind === 'mixin') {
        collectCatalogReferences(definition.patch, '$.patch', byIdentity);
    }
    else if (definition.kind === 'support' &&
        definition.semantic.catalog === 'participantProfile') {
        collectCatalogReferences(definition.semantic.data, '$.semantic.data', byIdentity);
    }
    else {
        return [];
    }
    return [...byIdentity.values()].sort((left, right) => `${left.category}#${left.packageId ?? ''}#${left.definitionId}`.localeCompare(`${right.category}#${right.packageId ?? ''}#${right.definitionId}`));
}
function collectCatalogReferences(value, path, references) {
    if (Array.isArray(value)) {
        value.forEach((entry, index) => collectCatalogReferences(entry, `${path}[${index}]`, references));
        return;
    }
    if (!isRecord(value))
        return;
    const ownedFields = new Set();
    for (const ownership of catalogOwnershipOf(value)) {
        ownedFields.add(ownership.field);
        references.set(`${ownership.category}#${ownership.packageId}#${ownership.definitionId}`, {
            definitionId: ownership.definitionId,
            category: ownership.category,
            packageId: ownership.packageId,
            path: `${path}.${ownership.field}`,
        });
    }
    for (const ownership of rulesetValueOwnershipOf(value)) {
        ownedFields.add(ownership.field);
    }
    for (const [key, child] of Object.entries(value)) {
        const childPath = `${path}.${key}`;
        const category = CATALOG_REFERENCE_FIELDS[key];
        if (category !== undefined &&
            typeof child === 'string' &&
            !ownedFields.has(key)) {
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
function definitionReferences(record, diagnostics, ruleset) {
    const references = [...(record.definition.lowLevelReferences ?? [])];
    const inheritedLocalIds = new Set((record.inheritedReferenceIds ?? []).map(localDefinitionId));
    for (const catalogReference of authoredCatalogReferences(record.definition)) {
        const definitionId = catalogReference.definitionId;
        if (inheritedLocalIds.has(definitionId))
            continue;
        if (catalogReference.packageId === undefined &&
            (catalogReference.category === 'stat' || catalogReference.category === 'defense') &&
            ruleset?.provides.values.some((value) => value.kind === catalogReference.category &&
                value.id === definitionId)) {
            continue;
        }
        if (catalogReference.packageId !== undefined) {
            const ownerPackageId = catalogReference.packageId;
            if (record.package.source.manifest.identity.id === ownerPackageId) {
                references.push({ definitionId });
                continue;
            }
            const ownerAliases = [...record.package.aliases.entries()]
                .filter(([, packageKey]) => packageIdFromIdentityKey(packageKey) === ownerPackageId)
                .map(([importAs]) => importAs)
                .sort();
            const importAs = ownerAliases[0];
            if (importAs !== undefined) {
                references.push({ importAs, definitionId });
                continue;
            }
            diagnostics.push(diagnostic('graph', 'CONTENT_PACK_CATALOG_REFERENCE_OWNER_UNAVAILABLE', catalogReference.path, `catalog owner ${ownerPackageId} is not a direct package dependency`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
                expected: ownerPackageId,
            }));
            continue;
        }
        const explicitMatches = references.filter((reference) => reference.definitionId === definitionId);
        if (explicitMatches.length === 1) {
            continue;
        }
        if (explicitMatches.length > 1) {
            diagnostics.push(diagnostic('graph', 'CONTENT_PACK_CATALOG_REFERENCE_AMBIGUOUS', catalogReference.path, `bare catalog definition ${definitionId} has more than one explicit low-level owner`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
            }));
            continue;
        }
        diagnostics.push(diagnostic('graph', 'CONTENT_PACK_CATALOG_REFERENCE_OWNER_REQUIRED', catalogReference.path, `bare catalog definition ${definitionId} has no owner; use defineContentCatalog or an explicit low-level definition reference`, {
            packageId: record.package.source.manifest.identity.id,
            definitionId: record.definition.id,
            source: record.definition.source,
        }));
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
                context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_EXPORT_MISSING', `$.packages[${entry.key}].exports[${index}]`, `export ${definitionId} has no declaration`, { packageId: entry.source.manifest.identity.id, definitionId, source: entry.source.manifest.entry }));
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
            context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_DEFINITION_CYCLE', '$.definitionGraph', `definition cycle: ${graphPath.join(' -> ')}`, { definitionId: record.definition.id, source: record.definition.source, graphPath }));
            return;
        }
        if (reachable.has(globalId))
            return;
        visiting.push(globalId);
        const references = new Set();
        for (const [index, reference] of definitionReferences(record, context.diagnostics, context.ruleset).entries()) {
            const target = resolveDefinitionReference(record, reference, index, definitionsByPackage, context.diagnostics);
            if (target !== undefined) {
                references.add(globalDefinitionId(target));
                visit(target);
            }
        }
        for (const inheritedReferenceId of record.inheritedReferenceIds ?? []) {
            const target = byGlobalId.get(inheritedReferenceId);
            if (target === undefined) {
                context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_INHERITED_REFERENCE_MISSING', `$.packages[${record.package.key}].definitions.${record.definition.id}.references`, `inherited definition reference ${inheritedReferenceId} is missing`, {
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
            context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_PUBLIC_DEFINITION_UNREACHABLE', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `public definition ${record.definition.id} is unreachable from an exported root`, {
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
            context.diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_TEMPLATE_MATERIALIZATION_DEFERRED', `$.packages[${record.package.key}].definitions.${record.definition.id}`, `template ${record.definition.id} requires #5957 derivation materialization`, {
                packageId: record.package.source.manifest.identity.id,
                definitionId: record.definition.id,
                source: record.definition.source,
            }));
        }
        if (materializedIds.has(record.definition.id)) {
            context.diagnostics.push(diagnostic('graph', 'CONTENT_PACK_DUPLICATE_DEFINITION_ID', '$.materializedDefinitions', `definition identity ${record.definition.id} is contributed by more than one package`, { definitionId: record.definition.id, source: record.definition.source }));
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
    for (const [index, reference] of definitionReferences(record, diagnostics).entries()) {
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
        diagnostics.push(diagnostic('graph', 'CONTENT_PACK_IMPORT_ALIAS_UNRESOLVED', path, `import alias ${reference.importAs ?? ''} is not declared`, {
            packageId: source.package.source.manifest.identity.id,
            definitionId: source.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    const target = definitionsByPackage.get(targetPackageKey)?.get(reference.definitionId);
    if (target === undefined) {
        diagnostics.push(diagnostic('graph', 'CONTENT_PACK_DEFINITION_REFERENCE_MISSING', path, `definition ${reference.definitionId} was not found in ${targetPackageKey}`, {
            packageId: source.package.source.manifest.identity.id,
            definitionId: source.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    if (targetPackageKey !== source.package.key &&
        (!target.exported || target.definition.visibility === 'private') &&
        !source.inheritedReferenceIds?.includes(globalDefinitionId(target))) {
        diagnostics.push(diagnostic('graph', 'CONTENT_PACK_PRIVATE_CROSS_PACKAGE_REFERENCE', path, `definition ${target.definition.id} is not exported for cross-package use`, {
            packageId: target.package.source.manifest.identity.id,
            definitionId: target.definition.id,
            source: source.definition.source,
        }));
        return undefined;
    }
    const catalogReference = authoredCatalogReferences(source.definition).find((candidate) => candidate.definitionId === reference.definitionId &&
        (candidate.packageId === undefined ||
            candidate.packageId === target.package.source.manifest.identity.id));
    if (catalogReference !== undefined &&
        (target.definition.kind !== 'support' ||
            target.definition.semantic.catalog !== catalogReference.category)) {
        diagnostics.push(diagnostic('graph', 'CONTENT_PACK_CATALOG_REFERENCE_KIND_MISMATCH', catalogReference.path, `catalog reference ${reference.definitionId} requires ${catalogReference.category} support`, {
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
function normalizeMaterializedActions(bundle, graph, diagnostics) {
    const actions = graph.materialized
        .filter((record) => record.definition.kind === 'action')
        .map((record) => {
        if (record.definition.kind !== 'action')
            throw new Error('unreachable narrowing failure');
        if (record.definition.action.id !== record.definition.id) {
            diagnostics.push(diagnostic('materialization', 'CONTENT_PACK_ACTION_ID_MISMATCH', `$.definitions.${record.definition.id}.action.id`, 'definition identity must match the normalized action identity', { definitionId: record.definition.id, source: record.definition.source }));
        }
        return record.definition.action;
    });
    if (diagnostics.length > 0)
        return undefined;
    const result = normalizePackage(definePackage({
        id: bundle.identity.id,
        version: bundle.identity.version,
        sources: [defineActions('play-bundle-actions', actions)],
    }));
    if (!result.ok) {
        diagnostics.push(...result.diagnostics.map((entry) => diagnostic('normalization', entry.code, entry.path, entry.message, entry.sourcePath === undefined
            ? {}
            : { source: { module: entry.sourcePath, declaration: entry.path } })));
        return undefined;
    }
    return result.artifact;
}
function validateRulesetValueOwners(record, ruleset, diagnostics) {
    const authoredValue = record.definition.kind === 'action'
        ? record.definition.action
        : record.definition.kind === 'support' &&
            record.definition.semantic.catalog === 'participantProfile'
            ? record.definition.semantic.data
            : undefined;
    if (authoredValue === undefined)
        return;
    const rootPath = record.definition.kind === 'action' ? '$.action' : '$.semantic.data';
    visitAuthoredValue(authoredValue, rootPath, (ownership, path) => {
        if (ownership.rulesetId === ruleset.identity.id)
            return;
        diagnostics.push(diagnostic('compatibility', 'RULESET_VALUE_REFERENCE_OWNER_MISMATCH', path, `value ${ownership.id} belongs to Ruleset ${ownership.rulesetId}, not ${ruleset.identity.id}`, {
            packageId: record.package.source.manifest.identity.id,
            definitionId: record.definition.id,
            source: record.definition.source,
            expected: ruleset.identity.id,
            actual: ownership.rulesetId,
        }));
    });
}
function visitAuthoredValue(value, path, visit) {
    if (Array.isArray(value)) {
        value.forEach((entry, index) => visitAuthoredValue(entry, `${path}[${index}]`, visit));
        return;
    }
    if (!isRecord(value))
        return;
    for (const ownership of rulesetValueOwnershipOf(value)) {
        visit(ownership, `${path}.${ownership.field}`);
    }
    for (const [field, child] of Object.entries(value)) {
        visitAuthoredValue(child, `${path}.${field}`, visit);
    }
}
function validateParticipantProfiles(materialized, resolvedReferences, ruleset, diagnostics) {
    const recordsByGlobalId = new Map(materialized.map((record) => [globalDefinitionId(record), record]));
    const rulesetValues = new Map(ruleset.provides.values.map((value) => [`${value.kind}:${value.id}`, value]));
    const numericDomains = new Map(ruleset.provides.numericDomains.map((domain) => [domain.id, domain]));
    for (const record of materialized) {
        if (!isParticipantProfileDefinition(record.definition))
            continue;
        const path = `$.packages[${record.package.key}].definitions.${record.definition.id}.semantic.data`;
        const profile = authoredParticipantProfile(record.definition);
        if (profile === undefined) {
            diagnostics.push(diagnostic('source', 'PARTICIPANT_PROFILE_SCHEMA_INVALID', path, 'participant profiles require asha.rpg.participant-profile@1, a role, definition references, and typed base capabilities', profileDiagnosticContext(record)));
            continue;
        }
        const definitionIdentities = new Set();
        let hasAction = false;
        const resolved = resolvedReferences.get(globalDefinitionId(record)) ?? [];
        for (const [index, reference] of profile.definitionReferences.entries()) {
            const identity = `${reference.importAs ?? ''}#${reference.definitionId}`;
            if (definitionIdentities.has(identity)) {
                diagnostics.push(diagnostic('source', 'PARTICIPANT_PROFILE_DEFINITION_DUPLICATE', `${path}.definitionReferences[${index}]`, `participant profile repeats definition ${reference.definitionId}`, profileDiagnosticContext(record)));
            }
            definitionIdentities.add(identity);
            const target = resolved
                .map((globalId) => recordsByGlobalId.get(globalId))
                .find((candidate) => candidate?.definition.id === reference.definitionId);
            if (target === undefined)
                continue;
            if (!target.exported || target.definition.visibility !== 'public') {
                diagnostics.push(diagnostic('graph', 'PARTICIPANT_PROFILE_DEFINITION_NOT_EXPORTED', `${path}.definitionReferences[${index}]`, `profile definition ${reference.definitionId} is not exported for participant setup`, profileDiagnosticContext(record)));
            }
            if (target.definition.kind === 'action')
                hasAction = true;
        }
        if (!hasAction) {
            diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_ACTION_REQUIRED', `${path}.definitionReferences`, 'an exported participant profile must reference at least one action', profileDiagnosticContext(record)));
        }
        let vitalityCount = 0;
        const capabilityIdentities = new Set();
        for (const [index, capability] of profile.capabilities.entries()) {
            const capabilityPath = `${path}.capabilities[${index}]`;
            const identity = profileCapabilityIdentity(capability);
            if (capabilityIdentities.has(identity)) {
                diagnostics.push(diagnostic('source', 'PARTICIPANT_PROFILE_CAPABILITY_DUPLICATE', capabilityPath, `participant profile repeats capability fact ${identity}`, profileDiagnosticContext(record)));
            }
            capabilityIdentities.add(identity);
            if (capability.owner === 'vitality') {
                vitalityCount += 1;
                validateProfileBoundedValue(capability.value, capabilityPath, record, diagnostics);
                continue;
            }
            if (capability.owner === 'stat' || capability.owner === 'defense') {
                const ownership = rulesetValueOwnershipOf(capability).find((candidate) => candidate.field === 'id');
                if (ownership === undefined) {
                    diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_RULESET_VALUE_OWNER_MISSING', `${capabilityPath}.id`, `profile ${capability.owner} ${capability.id} must be authored from a nominal Ruleset reference`, profileDiagnosticContext(record)));
                }
                const contract = rulesetValues.get(`${capability.owner}:${capability.id}`);
                if (contract === undefined)
                    continue;
                if (contract.source.kind === 'derived') {
                    diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_DERIVED_VALUE_FORBIDDEN', capabilityPath, `profile must not supply derived Ruleset value ${capability.id}`, profileDiagnosticContext(record)));
                }
                const domain = numericDomains.get(contract.numericDomainId);
                if (domain !== undefined &&
                    (capability.value < domain.minimum || capability.value > domain.maximum)) {
                    diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_VALUE_OUT_OF_DOMAIN', `${capabilityPath}.value`, `profile value must be within ${domain.minimum}..=${domain.maximum}`, profileDiagnosticContext(record)));
                }
                continue;
            }
            const ownership = catalogOwnershipOf(capability).find((candidate) => candidate.field === 'id');
            if (ownership === undefined || ownership.category !== capability.owner) {
                diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_CONTENT_VALUE_OWNER_MISSING', `${capabilityPath}.id`, `profile ${capability.owner} ${capability.id} must be authored from a nominal Content Pack catalog reference`, profileDiagnosticContext(record)));
            }
            if (capability.owner === 'resource') {
                validateProfileBoundedValue(capability.value, capabilityPath, record, diagnostics);
            }
            else if (capability.stackingGroup.trim().length === 0 ||
                capability.id.trim().length === 0 ||
                !Number.isInteger(capability.remainingTurns) ||
                capability.remainingTurns < 1 ||
                capability.remainingTurns > 1_000) {
                diagnostics.push(diagnostic('source', 'PARTICIPANT_PROFILE_MODIFIER_INVALID', capabilityPath, 'profile modifiers require identities and remainingTurns within 1..=1000', profileDiagnosticContext(record)));
            }
        }
        if (vitalityCount !== 1) {
            diagnostics.push(diagnostic('compatibility', 'PARTICIPANT_PROFILE_VITALITY_REQUIRED', `${path}.capabilities`, 'participant profiles require exactly one vitality base fact', profileDiagnosticContext(record)));
        }
    }
}
function isParticipantProfileDefinition(definition) {
    return definition.kind === 'support' && definition.semantic.catalog === 'participantProfile';
}
function authoredParticipantProfile(definition) {
    if (!isParticipantProfileDefinition(definition))
        return undefined;
    const data = definition.semantic.data;
    if (!isRecord(data) || !isRecord(data['schema']))
        return undefined;
    const schema = data['schema'];
    if (schema['identity'] !== 'asha.rpg.participant-profile' ||
        schema['version'] !== 1 ||
        (data['role'] !== 'player' && data['role'] !== 'creature') ||
        !Array.isArray(data['definitionReferences']) ||
        !data['definitionReferences'].every(isProfileDefinitionReference) ||
        !Array.isArray(data['capabilities']) ||
        !data['capabilities'].every(isProfileCapability)) {
        return undefined;
    }
    return data;
}
function isProfileDefinitionReference(value) {
    if (!isRecord(value) || typeof value['definitionId'] !== 'string')
        return false;
    return value['importAs'] === undefined || typeof value['importAs'] === 'string';
}
function isProfileCapability(value) {
    if (!isRecord(value) || typeof value['owner'] !== 'string')
        return false;
    if (value['owner'] === 'vitality')
        return isProfileBoundedValue(value['value']);
    if (value['owner'] === 'stat' || value['owner'] === 'defense') {
        return typeof value['id'] === 'string' && typeof value['value'] === 'number';
    }
    if (value['owner'] === 'resource') {
        return typeof value['id'] === 'string' && isProfileBoundedValue(value['value']);
    }
    if (value['owner'] === 'modifier') {
        return (typeof value['stackingGroup'] === 'string' &&
            typeof value['id'] === 'string' &&
            typeof value['value'] === 'number' &&
            typeof value['remainingTurns'] === 'number');
    }
    return false;
}
function isProfileBoundedValue(value) {
    return (isRecord(value) &&
        typeof value['current'] === 'number' &&
        typeof value['max'] === 'number');
}
function profileCapabilityIdentity(capability) {
    if (capability.owner === 'vitality')
        return 'vitality';
    if (capability.owner === 'modifier') {
        return `modifier:${capability.stackingGroup}`;
    }
    return `${capability.owner}:${capability.id}`;
}
function profileCapabilityRequirement(owner) {
    switch (owner) {
        case 'vitality': return 'capability.vitality';
        case 'stat': return 'capability.stats';
        case 'defense': return 'capability.defenses';
        case 'resource': return 'capability.resources';
        case 'modifier': return 'capability.modifiers';
    }
}
function validateProfileBoundedValue(value, path, record, diagnostics) {
    if (!Number.isInteger(value.current) ||
        !Number.isInteger(value.max) ||
        value.current < 0 ||
        value.max < 0 ||
        value.current > value.max) {
        diagnostics.push(diagnostic('source', 'PARTICIPANT_PROFILE_BOUNDED_VALUE_INVALID', path, 'profile bounded values require integer 0 <= current <= max', profileDiagnosticContext(record)));
    }
}
function profileDiagnosticContext(record) {
    return {
        packageId: record.package.source.manifest.identity.id,
        definitionId: record.definition.id,
        source: record.definition.source,
    };
}
function collectRequirements(context, normalized, materialized, ruleset) {
    const operations = new Map();
    const capabilities = new Map();
    const values = new Set();
    const numericDomains = new Set();
    for (const requirement of normalized.requirements) {
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
        for (const requirement of entry.source.manifest.requirements.values) {
            values.add(`${requirement.kind}:${requirement.id}`);
        }
        for (const requirement of entry.source.manifest.requirements.numericDomains) {
            numericDomains.add(requirement);
        }
    }
    for (const statId of normalized.catalogs.stats)
        values.add(`stat:${statId}`);
    for (const defenseId of normalized.catalogs.defenses)
        values.add(`defense:${defenseId}`);
    const rulesetValues = new Map(ruleset.provides.values.map((value) => [`${value.kind}:${value.id}`, value]));
    for (const record of materialized) {
        const profile = authoredParticipantProfile(record.definition);
        if (profile === undefined)
            continue;
        for (const capability of profile.capabilities) {
            const capabilityId = profileCapabilityRequirement(capability.owner);
            if (capabilityId !== undefined) {
                const version = RPG_CAPABILITY_VERSIONS[capabilityId];
                if (version !== undefined)
                    capabilities.set(capabilityId, version);
            }
            if (capability.owner !== 'stat' && capability.owner !== 'defense')
                continue;
            const key = `${capability.owner}:${capability.id}`;
            values.add(key);
            const contract = rulesetValues.get(key);
            if (contract !== undefined)
                numericDomains.add(contract.numericDomainId);
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
        values: [...values]
            .map((value) => {
            const separator = value.indexOf(':');
            const kind = value.slice(0, separator) === 'stat' ? 'stat' : 'defense';
            return { kind, id: value.slice(separator + 1) };
        })
            .sort((left, right) => left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id)),
        numericDomains: [...numericDomains].sort(),
    };
}
function validateCollectedRequirements(requirements, ruleset, diagnostics) {
    if (requirements === undefined)
        return;
    const operations = new Map(ruleset.provides.operations.map((provision) => [provision.id, provision.version]));
    const capabilities = new Map(ruleset.provides.capabilities.map((provision) => [provision.id, provision.version]));
    const values = new Set(ruleset.provides.values.map((provision) => `${provision.kind}:${provision.id}`));
    const numericDomains = new Set(ruleset.provides.numericDomains.map((provision) => provision.id));
    for (const [index, requirement] of requirements.operations.entries()) {
        if (operations.get(requirement.id) === requirement.version)
            continue;
        diagnostics.push(diagnostic('compatibility', 'PLAY_BUNDLE_OPERATION_REQUIREMENT_MISSING', `$.contentRequirements.operations[${index}]`, `materialized content requires operation ${requirement.id}@${requirement.version}, which the ruleset does not provide`));
    }
    for (const [index, requirement] of requirements.capabilities.entries()) {
        if (capabilities.get(requirement.id) === requirement.version)
            continue;
        diagnostics.push(diagnostic('compatibility', 'PLAY_BUNDLE_CAPABILITY_REQUIREMENT_MISSING', `$.contentRequirements.capabilities[${index}]`, `materialized content requires capability ${requirement.id}@${requirement.version}, which the ruleset does not provide`));
    }
    for (const [index, requirement] of requirements.values.entries()) {
        const key = `${requirement.kind}:${requirement.id}`;
        if (values.has(key))
            continue;
        diagnostics.push(diagnostic('compatibility', 'PLAY_BUNDLE_VALUE_REQUIREMENT_MISSING', `$.contentRequirements.values[${index}]`, `materialized content requires ${key}, which the ruleset does not provide`));
    }
    for (const [index, requirement] of requirements.numericDomains.entries()) {
        if (numericDomains.has(requirement))
            continue;
        diagnostics.push(diagnostic('compatibility', 'PLAY_BUNDLE_NUMERIC_DOMAIN_REQUIREMENT_MISSING', `$.contentRequirements.numericDomains[${index}]`, `materialized content requires numeric domain ${requirement}, which the ruleset does not provide`));
    }
}
function materializedSupportSemantic(definition) {
    const profile = authoredParticipantProfile(definition);
    if (profile === undefined)
        return definition.semantic;
    return {
        ...definition.semantic,
        data: {
            schema: profile.schema,
            role: profile.role,
            definitionIds: profile.definitionReferences
                .map((reference) => reference.definitionId)
                .sort(),
            capabilities: profile.capabilities,
        },
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
            : materializedSupportSemantic(definition);
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
        diagnostics.push(diagnostic('source', 'CONTENT_PACK_IDENTIFIER_INVALID', path, `invalid identifier ${value}`));
    }
}
function requireExactVersion(value, path, diagnostics) {
    if (parseVersion(value) === undefined) {
        diagnostics.push(diagnostic('source', 'CONTENT_PACK_VERSION_INVALID', path, `version ${value} is not exact semver`));
    }
}
function requireText(value, path, label, diagnostics) {
    if (value.trim().length === 0) {
        diagnostics.push(diagnostic('source', 'CONTENT_PACK_TEXT_REQUIRED', path, `${label} is required`));
    }
}
function rejectExecutableValues(value, path, diagnostics, seen) {
    if (typeof value === 'function') {
        diagnostics.push(diagnostic('source', 'CONTENT_PACK_EXECUTABLE_VALUE_FORBIDDEN', path, 'ruleset manifests may contain immutable declarations only'));
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
function packageIdFromIdentityKey(key) {
    const separator = key.lastIndexOf('@');
    return separator < 0 ? key : key.slice(0, separator);
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
//# sourceMappingURL=play-bundle-compiler.js.map