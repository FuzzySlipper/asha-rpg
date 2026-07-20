import { catalogDefinitionId, retainCatalogOwnership, } from './catalogs.js';
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
        team: options.team,
        maximumRange: options.maximumRange,
        maximumTargets: options.maximumTargets ?? 1,
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
    return frozenWithCatalogOwnership({ kind: 'readStat', subject, statId: catalogDefinitionId(id) }, 'statId', id);
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
        defenseId: catalogDefinitionId(options.defense),
    }, 'defenseId', options.defense);
}
export function savingThrow(options) {
    return frozenWithCatalogOwnership({
        kind: 'savingThrow',
        difficulty: options.difficulty,
        defenseId: catalogDefinitionId(options.defense),
    }, 'defenseId', options.defense);
}
export function spend(resource, amount) {
    return frozenWithCatalogOwnership({ resourceId: catalogDefinitionId(resource), amount }, 'resourceId', resource);
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
export function action(input) {
    const rollScope = input.check.kind === 'noRoll' ? (input.rollScope ?? 'none') : input.rollScope;
    return frozen({
        id: input.id,
        name: input.name,
        sourcePath: input.sourcePath,
        targets: input.targets,
        check: input.check,
        rollScope,
        costs: frozenList(input.costs ?? []),
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
export function defineScenario(id, actions) {
    return source('scenario', id, actions);
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
    return frozen(value);
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