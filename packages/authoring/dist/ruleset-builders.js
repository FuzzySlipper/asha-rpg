import { immutable } from './canonical.js';
const rulesetValueReferenceBrand = Symbol('asha-rpg.ruleset-value-reference');
export function defineRuleset(input) {
    return immutable({
        ...input,
        schema: { identity: 'asha.rpg.ruleset', major: 1 },
        provides: {
            operations: [...input.provides.operations].sort(compareVersionedProvision),
            capabilities: [...input.provides.capabilities].sort(compareVersionedProvision),
            values: [...input.provides.values].sort((left, right) => left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id)),
            numericDomains: [...input.provides.numericDomains].sort((left, right) => left.id.localeCompare(right.id)),
        },
    });
}
function compareVersionedProvision(left, right) {
    return left.id.localeCompare(right.id) || left.version - right.version;
}
export function rulesetStat(ruleset, id) {
    return rulesetValueReference(ruleset, 'stat', id);
}
export function rulesetDefense(ruleset, id) {
    return rulesetValueReference(ruleset, 'defense', id);
}
export function rulesetValueId(reference) {
    return reference.id;
}
function rulesetValueReference(ruleset, kind, id) {
    const contract = ruleset.provides.values.find((candidate) => candidate.kind === kind && candidate.id === id);
    if (contract === undefined) {
        throw new Error(`ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide ${kind} ${id}`);
    }
    return immutable({
        kind,
        id: id,
        rulesetId: ruleset.identity.id,
        [rulesetValueReferenceBrand]: true,
    });
}
//# sourceMappingURL=ruleset-builders.js.map