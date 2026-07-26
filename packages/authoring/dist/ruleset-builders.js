import { immutable } from './canonical.js';
const rulesetValueReferenceBrand = Symbol('asha-rpg.ruleset-value-reference');
const authoredRulesetValueOwnership = Symbol('asha-rpg.authored-ruleset-value-ownership');
const rulesetCalculationSelectorReferenceBrand = Symbol('asha-rpg.calculation-selector-reference');
const rulesetContributionStackingGroupReferenceBrand = Symbol('asha-rpg.contribution-stacking-group-reference');
const rulesetScalarTestProfileReferenceBrand = Symbol('asha-rpg.scalar-test-profile-reference');
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
            calculationSelectors: [...(input.provides.calculationSelectors ?? [])].sort((left, right) => left.id.localeCompare(right.id)),
            contributionStackingGroups: [
                ...(input.provides.contributionStackingGroups ?? []),
            ].sort((left, right) => left.id.localeCompare(right.id)),
            scalarTestProfiles: [...(input.provides.scalarTestProfiles ?? [])].sort((left, right) => left.id.localeCompare(right.id)),
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
export function rulesetCalculationSelector(ruleset, id) {
    if (!ruleset.provides.calculationSelectors.some((candidate) => candidate.id === id)) {
        throw new Error(`ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide calculation selector ${id}`);
    }
    return immutable({
        rulesetId: ruleset.identity.id,
        id,
        [rulesetCalculationSelectorReferenceBrand]: true,
    });
}
export function rulesetContributionStackingGroup(ruleset, id) {
    if (!ruleset.provides.contributionStackingGroups.some((candidate) => candidate.id === id)) {
        throw new Error(`ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide contribution stacking group ${id}`);
    }
    return immutable({
        rulesetId: ruleset.identity.id,
        id,
        [rulesetContributionStackingGroupReferenceBrand]: true,
    });
}
export function rulesetScalarTestProfile(ruleset, id) {
    if (!ruleset.provides.scalarTestProfiles.some((candidate) => candidate.id === id)) {
        throw new Error(`ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide scalar test profile ${id}`);
    }
    return immutable({
        rulesetId: ruleset.identity.id,
        id,
        [rulesetScalarTestProfileReferenceBrand]: true,
    });
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