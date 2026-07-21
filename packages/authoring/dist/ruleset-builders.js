import { immutable } from './canonical.js';
const rulesetValueReferenceBrand = Symbol('asha-rpg.ruleset-value-reference');
const authoredRulesetValueOwnership = Symbol('asha-rpg.authored-ruleset-value-ownership');
export function defineRuleset(input) {
    return immutable({
        ...input,
        schema: { identity: 'asha.rpg.ruleset', major: 1 },
        provides: {
            operations: [...input.provides.operations].sort(compareVersionedProvision),
            capabilities: [...input.provides.capabilities].sort(compareVersionedProvision),
            values: input.provides.values
                .map((value) => ({
                ...value,
                source: value.source ?? { kind: 'input' },
            }))
                .sort((left, right) => left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id)),
            numericDomains: [...input.provides.numericDomains].sort((left, right) => left.id.localeCompare(right.id)),
        },
    });
}
export function rulesetValueConstant(value) {
    return immutable({ kind: 'constant', value });
}
export function readRulesetValue(reference) {
    return immutable({
        kind: 'readValue',
        rulesetId: reference.rulesetId,
        valueKind: reference.kind,
        valueId: reference.id,
    });
}
export function subtractRulesetValues(minuend, subtrahend) {
    return immutable({ kind: 'subtract', minuend, subtrahend });
}
export function floorDivideRulesetValues(dividend, divisor) {
    return immutable({ kind: 'floorDivide', dividend, divisor });
}
export function derivedRulesetValue(expression) {
    return immutable({
        kind: 'derived',
        formula: {
            schema: {
                identity: 'asha.rpg.ruleset-value-formula',
                version: 1,
            },
            expression,
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
/** @internal Retains Ruleset owner identity on an AST node without serializing it. */
export function retainRulesetValueOwnership(value, fields) {
    const ownership = fields.flatMap(({ field, reference }) => isRulesetValueReference(reference)
        ? [
            immutable({
                field,
                kind: reference.kind,
                id: reference.id,
                rulesetId: reference.rulesetId,
            }),
        ]
        : []);
    if (ownership.length > 0) {
        Object.defineProperty(value, authoredRulesetValueOwnership, {
            value: immutable(ownership),
            enumerable: false,
            configurable: false,
            writable: false,
        });
    }
    return value;
}
/** @internal Reads Ruleset owner identity retained by typed authoring builders. */
export function rulesetValueOwnershipOf(value) {
    if (!(authoredRulesetValueOwnership in value))
        return [];
    const ownership = value[authoredRulesetValueOwnership];
    return Array.isArray(ownership) ? ownership : [];
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
function isRulesetValueReference(value) {
    return (value !== null &&
        typeof value === 'object' &&
        rulesetValueReferenceBrand in value);
}
//# sourceMappingURL=ruleset-builders.js.map