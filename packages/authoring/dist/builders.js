import { catalogDefinitionId, retainCatalogOwnership, } from './catalogs.js';
import { retainRulesetValueOwnership, rulesetValueId, } from './ruleset-builders.js';
const authoredDefinitionOwnership = Symbol('asha-rpg.authored-definition-ownership');
export function actionId(value) {
    return checkedIdentifier(value, 'action id');
}
export function stackingGroup(value) {
    return checkedIdentifier(value, 'stacking group');
}
export function reactionId(value) {
    return checkedIdentifier(value, 'reaction id');
}
export function reactionOptionId(value) {
    return checkedIdentifier(value, 'reaction option id');
}
export function targets(options) {
    return frozen({
        kind: 'participant',
        team: options.team,
        maximumRange: options.maximumRange,
        maximumTargets: options.maximumTargets ?? 1,
    });
}
export function cells(options) {
    return frozen({
        kind: 'cell',
        team: 'any',
        maximumRange: options.range,
        maximumTargets: 1,
    });
}
export function hostile(options) {
    return options.maximum === undefined
        ? targets({ team: 'hostile', maximumRange: options.range })
        : targets({
            team: 'hostile',
            maximumRange: options.range,
            maximumTargets: options.maximum,
        });
}
export function ally(options) {
    return options.maximum === undefined
        ? targets({ team: 'ally', maximumRange: options.range })
        : targets({
            team: 'ally',
            maximumRange: options.range,
            maximumTargets: options.maximum,
        });
}
export function constant(value) {
    return frozen({ kind: 'constant', value });
}
export function readStat(subject, id) {
    return frozenWithCatalogOwnership({ kind: 'readStat', subject, statId: authoredValueId(id) }, 'statId', id);
}
export function add(...terms) {
    return frozen({ kind: 'add', terms: frozenList(terms) });
}
export function dice(options) {
    return frozen({
        kind: 'dice',
        count: options.count,
        sides: options.sides,
        bonus: options.bonus ?? 0,
    });
}
export function half(value) {
    return frozen({ kind: 'half', value });
}
export function always() {
    return frozen({ kind: 'always' });
}
export function compare(left, comparison, right) {
    return frozen({ kind: 'compare', left, comparison, right });
}
export function not(predicate) {
    return frozen({ kind: 'not', predicate });
}
export function all(...predicates) {
    return frozen({ kind: 'all', predicates: frozenList(predicates) });
}
export function any(...predicates) {
    return frozen({ kind: 'any', predicates: frozenList(predicates) });
}
export function noRoll() {
    return frozen({ kind: 'noRoll' });
}
export function attack(options) {
    return frozenWithCatalogOwnership({
        kind: 'attack',
        modifier: options.modifier,
        defenseId: authoredValueId(options.defense),
        ...(options.contributionSelector === undefined
            ? {}
            : { contributionSelector: options.contributionSelector }),
    }, 'defenseId', options.defense);
}
export function savingThrow(options) {
    return frozenWithCatalogOwnership({
        kind: 'savingThrow',
        difficulty: options.difficulty,
        defenseId: authoredValueId(options.defense),
    }, 'defenseId', options.defense);
}
export function scalarTest(options) {
    if (options.difficulty.kind === 'explicit') {
        return frozen({
            kind: 'scalarTest',
            profile: options.profile,
            base: options.base,
            difficulty: {
                kind: 'explicit',
                value: options.difficulty.value,
            },
        });
    }
    return frozenWithCatalogOwnership({
        kind: 'scalarTest',
        profile: options.profile,
        base: options.base,
        difficulty: {
            kind: 'targetDefense',
            defenseId: authoredValueId(options.difficulty.defense),
        },
    }, 'difficulty.defenseId', options.difficulty.defense);
}
export function heterogeneousPool(options) {
    return frozen({
        kind: 'heterogeneousPool',
        profile: options.profile,
        baseDice: [...options.baseDice].sort((left, right) => left.dieTypeId.localeCompare(right.dieTypeId)),
        automaticAxes: [...(options.automaticAxes ?? [])].sort((left, right) => left.axisId.localeCompare(right.axisId)),
    });
}
export function spend(resource, amount) {
    return frozenWithCatalogOwnership({ resourceId: catalogDefinitionId(resource), amount }, 'resourceId', resource);
}
export function activation(options) {
    return frozen({
        timing: options.timing,
        costs: frozenList((options.costs ?? [])
            .map((cost) => frozen({
            budget: cost.budget,
            amount: cost.amount,
        }))
            .sort((left, right) => left.budget.rulesetId.localeCompare(right.budget.rulesetId) ||
            left.budget.id.localeCompare(right.budget.id))),
    });
}
export function immediate() {
    return frozen({ kind: 'immediate' });
}
export function turns(count) {
    return frozen({ kind: 'turns', count });
}
export function replace(group) {
    return frozen({ kind: 'replace', group });
}
export function refresh(group) {
    return frozen({ kind: 'refresh', group });
}
export function damage(options) {
    return operation(frozenWithCatalogOwnership({
        kind: 'damage',
        amount: options.amount,
        damageType: catalogDefinitionId(options.type),
    }, 'damageType', options.type), options.timing);
}
export function heal(options) {
    return operation(frozen({ kind: 'heal', amount: options.amount }), options.timing);
}
export function changeResource(options) {
    return operation(frozenWithCatalogOwnership({
        kind: 'changeResource',
        subject: options.subject,
        resourceId: catalogDefinitionId(options.resource),
        delta: options.delta,
    }, 'resourceId', options.resource), options.timing);
}
export function applyModifier(options) {
    return operation(frozenWithCatalogOwnership({
        kind: 'applyModifier',
        modifierId: catalogDefinitionId(options.modifier),
        stackingGroup: options.stacking.group,
        stacking: options.stacking.kind,
        value: options.value,
        durationTurns: options.duration.count,
    }, 'modifierId', options.modifier), options.timing);
}
export function applyEffect(options) {
    const declaration = {
        kind: 'applyEffect',
        effectDefinitionId: options.effect.definitionId,
        rank: options.rank,
    };
    retainDefinitionOwnership(declaration, 'effectDefinitionId', options.effect);
    return operation(frozen(declaration), options.timing);
}
export function removeEffect(options) {
    const declaration = {
        kind: 'removeEffect',
        effectDefinitionId: options.effect.definitionId,
    };
    retainDefinitionOwnership(declaration, 'effectDefinitionId', options.effect);
    return operation(frozen(declaration), options.timing);
}
function retainDefinitionOwnership(value, field, reference) {
    Object.defineProperty(value, authoredDefinitionOwnership, {
        value: frozenList([frozen({ field, reference })]),
        enumerable: false,
        configurable: false,
        writable: false,
    });
}
/** @internal Reads typed definition edges retained by operation builders. */
export function definitionOwnershipOf(value) {
    if (!(authoredDefinitionOwnership in value))
        return [];
    const ownership = Reflect.get(value, authoredDefinitionOwnership);
    return Array.isArray(ownership) ? ownership : [];
}
export function moveEntity(options) {
    return operation(frozen({
        kind: 'move',
        subject: options.subject,
        deltaX: options.deltaX,
        deltaY: options.deltaY,
        maximumDistance: options.maximumDistance,
        provokes: options.provokes,
    }), options.timing);
}
export function moveToCell(options) {
    return operation(frozen({
        kind: 'moveToCell',
        maximumDistance: options.maximumDistance,
        provokes: options.provokes,
    }), options.timing);
}
export function openReaction(options) {
    return operation(frozen({
        kind: 'openReaction',
        reactionId: options.id,
        options: frozenList(options.options.map((option) => frozen({ ...option }))),
    }), options.timing);
}
export function sequence(...steps) {
    return frozen({ kind: 'sequence', steps: frozenList(steps) });
}
export function when(predicate, then, otherwise) {
    return otherwise === undefined
        ? frozen({ kind: 'when', predicate, then })
        : frozen({ kind: 'when', predicate, then, otherwise });
}
export function repeat(count, body) {
    return frozen({ kind: 'repeat', count, body });
}
export function forEachTarget(maximum, body) {
    return frozen({ kind: 'forEachTarget', maximum, body });
}
export function onCheck(branches) {
    return frozen({ kind: 'onCheck', ...branches });
}
export function onOutcome(branches) {
    return frozen({
        kind: 'onOutcome',
        branches: frozen(Object.fromEntries(Object.entries(branches.branches)
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([id, branch]) => [id, branch]))),
        default: branches.default,
    });
}
export function action(input) {
    const rollScope = input.check.kind === 'noRoll' ? (input.rollScope ?? 'none') : input.rollScope;
    return frozen({
        id: input.id,
        name: input.name,
        sourcePath: input.sourcePath,
        tags: frozenList([...(input.tags ?? [])].sort()),
        targets: input.targets,
        check: input.check,
        rollScope,
        costs: frozenList(input.costs ?? []),
        ...(input.activation === undefined ? {} : { activation: input.activation }),
        program: input.program,
    });
}
export function defineActions(id, actions) {
    return source('actions', id, actions);
}
export function defineArchetype(id, actions) {
    return source('archetype', id, actions);
}
export function defineItem(id, actions) {
    return source('item', id, actions);
}
export function definePackage(options) {
    return frozen({
        id: options.id,
        version: options.version,
        sources: frozenList(options.sources),
    });
}
function operation(declaration, timing = immediate()) {
    return frozen({ kind: 'operation', operation: declaration, timing });
}
function source(kind, id, actions) {
    return frozen({ kind, id, actions: frozenList(actions) });
}
function frozen(value) {
    return Object.freeze(value);
}
function frozenWithCatalogOwnership(value, field, reference) {
    retainCatalogOwnership(value, [{ field, reference }]);
    retainRulesetValueOwnership(value, [{ field, reference }]);
    return frozen(value);
}
function authoredValueId(reference) {
    return 'definitionId' in reference
        ? catalogDefinitionId(reference)
        : rulesetValueId(reference);
}
function frozenList(values) {
    return Object.freeze([...values]);
}
function checkedIdentifier(value, label) {
    if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)) {
        throw new Error(`${label} must be a non-empty portable identifier`);
    }
    return value;
}
//# sourceMappingURL=builders.js.map