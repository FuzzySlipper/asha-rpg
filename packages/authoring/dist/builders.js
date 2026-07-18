export function actionId(value) {
    return value;
}
export function statId(value) {
    return value;
}
export function defenseId(value) {
    return value;
}
export function resourceId(value) {
    return value;
}
export function modifierId(value) {
    return value;
}
export function damageType(value) {
    return value;
}
export function stackingGroup(value) {
    return value;
}
export function reactionId(value) {
    return value;
}
export function reactionOptionId(value) {
    return value;
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
    return frozen({ kind: 'readStat', subject, statId: id });
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
    return frozen({
        kind: 'attack',
        modifier: options.modifier,
        defenseId: options.defense,
    });
}
export function savingThrow(options) {
    return frozen({
        kind: 'savingThrow',
        difficulty: options.difficulty,
        defenseId: options.defense,
    });
}
export function spend(resource, amount) {
    return frozen({ resourceId: resource, amount });
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
    return operation(frozen({ kind: 'damage', amount: options.amount, damageType: options.type }), options.timing);
}
export function heal(options) {
    return operation(frozen({ kind: 'heal', amount: options.amount }), options.timing);
}
export function changeResource(options) {
    return operation(frozen({
        kind: 'changeResource',
        subject: options.subject,
        resourceId: options.resource,
        delta: options.delta,
    }), options.timing);
}
export function applyModifier(options) {
    return operation(frozen({
        kind: 'applyModifier',
        modifierId: options.modifier,
        stackingGroup: options.stacking.group,
        stacking: options.stacking.kind,
        value: options.value,
        durationTurns: options.duration.count,
    }), options.timing);
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
function frozenList(values) {
    return Object.freeze([...values]);
}
//# sourceMappingURL=builders.js.map