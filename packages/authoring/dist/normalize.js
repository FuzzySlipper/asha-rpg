import { RPG_CAPABILITY_VERSIONS, RPG_IR_IDENTITY, RPG_IR_MAJOR, RPG_OPERATION_VERSIONS, } from '@asha-rpg/ir';
const OPERATION_IDS = {
    damage: 'operation.damage',
    heal: 'operation.heal',
    changeResource: 'operation.changeResource',
    applyModifier: 'operation.applyModifier',
    applyEffect: 'operation.applyEffect',
    removeEffect: 'operation.removeEffect',
    move: 'operation.move',
    moveToCell: 'operation.moveToCell',
    openReaction: 'operation.openReaction',
};
const NO_DIAGNOSTICS = Object.freeze([]);
export function normalizePackage(source) {
    const diagnostics = [];
    rejectExecutableValues(source, '$', diagnostics, new WeakSet());
    requireText(source.id, '$.package.id', 'package id', diagnostics);
    requireText(source.version, '$.package.version', 'package version', diagnostics);
    const actions = source.sources.flatMap((entry) => entry.actions);
    const actionIds = new Set();
    for (const [index, action] of actions.entries()) {
        const path = `$.actions[${index}]`;
        validateAction(action, path, diagnostics);
        if (actionIds.has(action.id)) {
            diagnostics.push(diagnostic('normalization.duplicateActionId', `${path}.id`, `duplicate action id ${action.id}`, action.sourcePath));
        }
        actionIds.add(action.id);
    }
    if (diagnostics.length > 0) {
        return Object.freeze({ ok: false, diagnostics: Object.freeze(diagnostics) });
    }
    const collection = emptyCollection();
    for (const action of actions)
        collectAction(action, collection);
    const normalizedActions = actions
        .map(normalizeAction)
        .sort((left, right) => compareText(left.id, right.id));
    const artifact = deepFreeze({
        schema: { identity: RPG_IR_IDENTITY, major: RPG_IR_MAJOR },
        package: { id: source.id, version: source.version },
        catalogs: {
            stats: sorted(collection.stats),
            defenses: sorted(collection.defenses),
            resources: sorted(collection.resources),
            modifiers: sorted(collection.modifiers),
            capabilities: sorted(collection.capabilities),
        },
        requirements: [
            ...sorted(collection.operations).map((id) => ({
                kind: 'operation',
                id,
                version: RPG_OPERATION_VERSIONS[id],
            })),
            ...sorted(collection.capabilities).map((id) => ({
                kind: 'capability',
                id,
                version: RPG_CAPABILITY_VERSIONS[id],
            })),
        ],
        actions: normalizedActions,
    });
    return Object.freeze({ ok: true, artifact, diagnostics: NO_DIAGNOSTICS });
}
export function canonicalRpgJson(artifact) {
    return JSON.stringify(canonicalValue(artifact));
}
export function normalizeAction(action) {
    return {
        id: action.id,
        name: action.name,
        sourcePath: action.sourcePath,
        tags: [...action.tags],
        targets: {
            ...action.targets,
            lineOfEffect: action.targets.lineOfEffect ?? 'ignored',
        },
        check: action.check,
        rollScope: normalizedRollScope(action),
        costs: [...action.costs],
        ...(action.activation === undefined
            ? {}
            : { activation: action.activation }),
        program: { kind: 'atomic', body: normalizeProgram(action.program) },
    };
}
function normalizeProgram(program) {
    switch (program.kind) {
        case 'operation':
            return { kind: 'operation', operation: program.operation };
        case 'sequence':
            return { kind: 'sequence', steps: program.steps.map(normalizeProgram) };
        case 'when':
            return program.otherwise === undefined
                ? {
                    kind: 'when',
                    predicate: program.predicate,
                    then: normalizeProgram(program.then),
                }
                : {
                    kind: 'when',
                    predicate: program.predicate,
                    then: normalizeProgram(program.then),
                    otherwise: normalizeProgram(program.otherwise),
                };
        case 'repeat':
            return { kind: 'repeat', count: program.count, body: normalizeProgram(program.body) };
        case 'forEachTarget':
            return {
                kind: 'forEachTarget',
                maximum: program.maximum,
                body: normalizeProgram(program.body),
            };
        case 'onCheck': {
            return copyCheckBranches(program);
        }
        case 'onOutcome':
            return {
                kind: 'onOutcome',
                branches: Object.fromEntries(Object.entries(program.branches)
                    .sort(([left], [right]) => left.localeCompare(right))
                    .map(([id, branch]) => [id, normalizeProgram(branch)])),
                default: normalizeProgram(program.default),
            };
    }
}
function copyCheckBranches(source) {
    return {
        kind: 'onCheck',
        ...(source.hit === undefined ? {} : { hit: normalizeProgram(source.hit) }),
        ...(source.miss === undefined ? {} : { miss: normalizeProgram(source.miss) }),
        ...(source.saved === undefined ? {} : { saved: normalizeProgram(source.saved) }),
        ...(source.failed === undefined ? {} : { failed: normalizeProgram(source.failed) }),
        ...(source.noRoll === undefined ? {} : { noRoll: normalizeProgram(source.noRoll) }),
    };
}
function validateAction(action, path, diagnostics) {
    requireText(action.id, `${path}.id`, 'action id', diagnostics, action.sourcePath);
    requireText(action.name, `${path}.name`, 'action name', diagnostics, action.sourcePath);
    requireText(action.sourcePath, `${path}.sourcePath`, 'source path', diagnostics);
    let previousTag;
    for (const [index, tag] of action.tags.entries()) {
        requireText(tag, `${path}.tags[${index}]`, 'action tag', diagnostics, action.sourcePath);
        if (previousTag !== undefined && previousTag >= tag) {
            diagnostics.push(diagnostic('normalization.actionTagsNotCanonical', `${path}.tags[${index}]`, 'action tags must be unique and sorted', action.sourcePath));
        }
        previousTag = tag;
    }
    if (action.check.kind === 'noRoll' && action.rollScope !== 'none') {
        diagnostics.push(diagnostic('normalization.rollScopeInvalid', `${path}.rollScope`, 'no-roll checks require roll scope none', action.sourcePath));
    }
    if (action.check.kind !== 'noRoll' &&
        action.rollScope !== 'shared' &&
        action.rollScope !== 'perTarget') {
        diagnostics.push(diagnostic('normalization.rollScopeInvalid', `${path}.rollScope`, 'rolled checks require shared or per-target scope', action.sourcePath));
    }
    if (action.check.kind === 'scalarTest') {
        validateScalarExpression(action.check.base, `${path}.check.base`, diagnostics, action.sourcePath);
        if (action.check.difficulty.kind === 'explicit') {
            validateScalarExpression(action.check.difficulty.value, `${path}.check.difficulty.value`, diagnostics, action.sourcePath);
        }
    }
    if (action.activation !== undefined) {
        validateActivation(action.activation, 'action', `${path}.activation`, diagnostics, action.sourcePath);
    }
    if (!integerInRange(action.targets.maximumTargets, 1, 32)) {
        diagnostics.push(diagnostic('normalization.targetBoundInvalid', `${path}.targets.maximumTargets`, 'target maximum must be an integer between 1 and 32', action.sourcePath));
    }
    if (action.targets.lineOfEffect !== undefined &&
        action.targets.lineOfEffect !== 'ignored' &&
        action.targets.lineOfEffect !== 'required') {
        diagnostics.push(diagnostic('normalization.lineOfEffectInvalid', `${path}.targets.lineOfEffect`, 'line of effect must be explicitly ignored or required', action.sourcePath));
    }
    if (action.targets.kind === 'cell' &&
        (action.targets.team !== 'any' ||
            action.targets.maximumTargets !== 1 ||
            action.targets.area !== undefined)) {
        diagnostics.push(diagnostic('normalization.cellTargetInvalid', `${path}.targets`, 'cell targets require team any and exactly one destination', action.sourcePath));
    }
    if (action.targets.kind === 'area') {
        const area = action.targets.area;
        const shapeBound = area?.shape.kind === 'diamond'
            ? 1 + 2 * area.shape.radius * (area.shape.radius + 1)
            : area?.shape.kind === 'orthogonalLine'
                ? area.shape.length
                : 0;
        const shapeValid = area !== undefined &&
            area.schema.identity === 'asha.rpg.area-selector' &&
            area.schema.version === 1 &&
            integerInRange(area.minimumTargets, 0, action.targets.maximumTargets) &&
            integerInRange(action.targets.maximumRange, 0, 2046) &&
            shapeBound >= 1 &&
            shapeBound <= 256 &&
            ((area.origin === 'anchor' &&
                area.shape.kind === 'diamond' &&
                integerInRange(area.shape.radius, 0, 1024)) ||
                (area.origin === 'actor' &&
                    area.shape.kind === 'orthogonalLine' &&
                    integerInRange(area.shape.length, 1, 256)));
        if (!shapeValid) {
            diagnostics.push(diagnostic('normalization.areaTargetInvalid', `${path}.targets.area`, 'area targets require the versioned bounded diamond/anchor or orthogonal-line/actor contract', action.sourcePath));
        }
        if (!areaProgramCoversTargetMaximum(action.program, action.targets.maximumTargets)) {
            diagnostics.push(diagnostic('normalization.areaProgramBoundInvalid', `${path}.program`, 'every area-target for-each bound must equal the selector maximum', action.sourcePath));
        }
    }
    else if (action.targets.area !== undefined) {
        diagnostics.push(diagnostic('normalization.areaTargetUnexpected', `${path}.targets.area`, 'only area-target actions may declare an area selector', action.sourcePath));
    }
    if (action.targets.kind === 'cell' && action.check.kind !== 'noRoll') {
        diagnostics.push(diagnostic('normalization.cellCheckInvalid', `${path}.check`, 'cell-target actions require a no-roll check', action.sourcePath));
    }
    if (action.targets.kind === 'cell' &&
        !isSelectedDestinationMovementProgram(action.program)) {
        diagnostics.push(diagnostic('normalization.cellProgramInvalid', `${path}.program`, 'a cell-target action requires an unconditional no-roll branch containing only one moveToCell operation', action.sourcePath));
    }
    if (action.targets.kind !== 'cell' &&
        countOperations(action.program, 'moveToCell') > 0) {
        diagnostics.push(diagnostic('normalization.moveToCellTargetInvalid', `${path}.program`, 'moveToCell requires a cell-target action', action.sourcePath));
    }
    for (const [index, cost] of action.costs.entries()) {
        if (!integerInRange(cost.amount, 1, Number.MAX_SAFE_INTEGER)) {
            diagnostics.push(diagnostic('normalization.costInvalid', `${path}.costs[${index}].amount`, 'resource cost must be a positive safe integer', action.sourcePath));
        }
    }
    validateProgram(action.program, `${path}.program`, 1, action.check.kind, diagnostics, action.sourcePath);
}
function validateActivation(activation, expectedTiming, path, diagnostics, sourcePath) {
    if (activation.timing !== expectedTiming) {
        diagnostics.push(diagnostic('normalization.activationTimingInvalid', `${path}.timing`, `activation timing must be ${expectedTiming}`, sourcePath));
    }
    let previousBudget;
    for (const [index, cost] of activation.costs.entries()) {
        const key = `${cost.budget.rulesetId}:${cost.budget.id}`;
        if (cost.budget.rulesetId.trim().length === 0 ||
            cost.budget.id.trim().length === 0 ||
            previousBudget !== undefined && previousBudget >= key ||
            !Number.isSafeInteger(cost.amount) ||
            cost.amount < 0 ||
            cost.amount > 2_147_483_647) {
            diagnostics.push(diagnostic('normalization.activationCostInvalid', `${path}.costs[${index}]`, 'activation costs must be unique sorted owned budget references with bounded non-negative amounts', sourcePath));
        }
        previousBudget = key;
    }
}
function validateScalarExpression(formula, path, diagnostics, sourcePath) {
    switch (formula.kind) {
        case 'constant':
        case 'readStat':
            return;
        case 'add':
            for (const [index, term] of formula.terms.entries()) {
                validateScalarExpression(term, `${path}.terms[${index}]`, diagnostics, sourcePath);
            }
            return;
        case 'half':
            validateScalarExpression(formula.value, `${path}.value`, diagnostics, sourcePath);
            return;
        case 'dice':
            diagnostics.push(diagnostic('normalization.scalarTestRandomFormulaInvalid', path, 'scalar-test base and explicit difficulty expressions cannot contain dice', sourcePath));
    }
}
function isSelectedDestinationMovementProgram(program) {
    if (program.kind !== 'onCheck')
        return false;
    if (program.hit !== undefined ||
        program.miss !== undefined ||
        program.saved !== undefined ||
        program.failed !== undefined ||
        program.noRoll === undefined) {
        return false;
    }
    return (program.noRoll.kind === 'operation' &&
        program.noRoll.operation.kind === 'moveToCell');
}
function areaProgramCoversTargetMaximum(program, maximum) {
    const maxima = [];
    const collect = (node) => {
        switch (node.kind) {
            case 'operation':
                return;
            case 'sequence':
                node.steps.forEach(collect);
                return;
            case 'when':
                collect(node.then);
                if (node.otherwise !== undefined)
                    collect(node.otherwise);
                return;
            case 'repeat':
                collect(node.body);
                return;
            case 'forEachTarget':
                maxima.push(node.maximum);
                collect(node.body);
                return;
            case 'onCheck':
                [
                    node.hit,
                    node.miss,
                    node.saved,
                    node.failed,
                    node.noRoll,
                ].forEach((branch) => {
                    if (branch !== undefined)
                        collect(branch);
                });
                return;
            case 'onOutcome':
                Object.values(node.branches).forEach(collect);
                collect(node.default);
        }
    };
    collect(program);
    return maxima.length > 0 && maxima.every((bound) => bound === maximum);
}
function countOperations(program, kind) {
    switch (program.kind) {
        case 'operation':
            return program.operation.kind === kind ? 1 : 0;
        case 'sequence':
            return program.steps.reduce((count, step) => count + countOperations(step, kind), 0);
        case 'when':
            return (countOperations(program.then, kind) +
                (program.otherwise === undefined
                    ? 0
                    : countOperations(program.otherwise, kind)));
        case 'repeat':
        case 'forEachTarget':
            return countOperations(program.body, kind);
        case 'onCheck':
            return [
                program.hit,
                program.miss,
                program.saved,
                program.failed,
                program.noRoll,
            ].reduce((count, branch) => count + (branch === undefined ? 0 : countOperations(branch, kind)), 0);
        case 'onOutcome':
            return (Object.values(program.branches).reduce((count, branch) => count + countOperations(branch, kind), 0) + countOperations(program.default, kind));
    }
}
function validateProgram(program, path, depth, checkKind, diagnostics, sourcePath) {
    if (depth > 16) {
        diagnostics.push(diagnostic('normalization.programDepthExceeded', path, 'program depth exceeds 16', sourcePath));
        return;
    }
    switch (program.kind) {
        case 'operation':
            if (program.timing.kind !== 'immediate') {
                diagnostics.push(diagnostic('normalization.timingUnsupported', `${path}.timing`, 'the active vocabulary supports immediate timing only', sourcePath));
            }
            validateOperation(program.operation, path, diagnostics, sourcePath);
            return;
        case 'sequence':
            if (program.steps.length === 0) {
                diagnostics.push(diagnostic('normalization.emptySequence', path, 'sequence is empty', sourcePath));
            }
            for (const [index, step] of program.steps.entries()) {
                validateProgram(step, `${path}.steps[${index}]`, depth + 1, checkKind, diagnostics, sourcePath);
            }
            return;
        case 'when':
            validateProgram(program.then, `${path}.then`, depth + 1, checkKind, diagnostics, sourcePath);
            if (program.otherwise !== undefined) {
                validateProgram(program.otherwise, `${path}.otherwise`, depth + 1, checkKind, diagnostics, sourcePath);
            }
            return;
        case 'repeat':
            if (!integerInRange(program.count, 1, 16)) {
                diagnostics.push(diagnostic('normalization.repeatBoundInvalid', `${path}.count`, 'repeat count must be an integer between 1 and 16', sourcePath));
            }
            validateProgram(program.body, `${path}.body`, depth + 1, checkKind, diagnostics, sourcePath);
            return;
        case 'forEachTarget':
            if (!integerInRange(program.maximum, 1, 32)) {
                diagnostics.push(diagnostic('normalization.targetBoundInvalid', `${path}.maximum`, 'per-target maximum must be an integer between 1 and 32', sourcePath));
            }
            validateProgram(program.body, `${path}.body`, depth + 1, checkKind, diagnostics, sourcePath);
            return;
        case 'onCheck': {
            const hasIncompatibleBranch = (checkKind === 'noRoll' &&
                (program.hit !== undefined ||
                    program.miss !== undefined ||
                    program.saved !== undefined ||
                    program.failed !== undefined)) ||
                (checkKind === 'attack' &&
                    (program.saved !== undefined ||
                        program.failed !== undefined ||
                        program.noRoll !== undefined)) ||
                (checkKind === 'savingThrow' &&
                    (program.hit !== undefined ||
                        program.miss !== undefined ||
                        program.noRoll !== undefined)) ||
                checkKind === 'scalarTest' ||
                checkKind === 'heterogeneousPool';
            if (hasIncompatibleBranch) {
                diagnostics.push(diagnostic('normalization.checkBranchIncompatible', path, 'check branch contains an outcome unavailable to the selected check', sourcePath));
            }
            const branches = [program.hit, program.miss, program.saved, program.failed, program.noRoll];
            if (branches.every((branch) => branch === undefined)) {
                diagnostics.push(diagnostic('normalization.emptyCheckBranch', path, 'check branch has no outcomes', sourcePath));
            }
            for (const [index, branch] of branches.entries()) {
                if (branch !== undefined) {
                    validateProgram(branch, `${path}.branches[${index}]`, depth + 1, checkKind, diagnostics, sourcePath);
                }
            }
            return;
        }
        case 'onOutcome': {
            if (checkKind !== 'scalarTest' &&
                checkKind !== 'heterogeneousPool') {
                diagnostics.push(diagnostic('normalization.outcomeBranchIncompatible', path, 'onOutcome is available only to scalar tests and heterogeneous pools', sourcePath));
            }
            for (const [id, branch] of Object.entries(program.branches)) {
                requireText(id, `${path}.branches.${id}`, 'outcome band id', diagnostics, sourcePath);
                validateProgram(branch, `${path}.branches.${id}`, depth + 1, checkKind, diagnostics, sourcePath);
            }
            validateProgram(program.default, `${path}.default`, depth + 1, checkKind, diagnostics, sourcePath);
            return;
        }
    }
}
function validateOperation(operation, path, diagnostics, sourcePath) {
    if (operation.kind === 'damage') {
        if (operation.parts.length < 1 || operation.parts.length > 16) {
            diagnostics.push(diagnostic('normalization.damagePartsInvalid', `${path}.operation.parts`, 'damage packets require between 1 and 16 parts', sourcePath));
        }
        let previousId;
        for (const [index, part] of operation.parts.entries()) {
            const partPath = `${path}.operation.parts[${index}]`;
            if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(part.id) ||
                (previousId !== undefined && previousId >= part.id)) {
                diagnostics.push(diagnostic('normalization.damagePartsNotCanonical', `${partPath}.id`, 'damage part identities must be unique sorted portable identifiers', sourcePath));
            }
            previousId = part.id;
            let previousTag;
            for (const [tagIndex, tag] of part.tags.entries()) {
                if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(tag) ||
                    (previousTag !== undefined && previousTag >= tag)) {
                    diagnostics.push(diagnostic('normalization.damageTagsNotCanonical', `${partPath}.tags[${tagIndex}]`, 'damage tags must be unique sorted portable identifiers', sourcePath));
                }
                previousTag = tag;
            }
            if (part.tags.length > 16) {
                diagnostics.push(diagnostic('normalization.damageTagsInvalid', `${partPath}.tags`, 'one damage part may declare at most 16 tags', sourcePath));
            }
        }
    }
    if (operation.kind === 'applyModifier' && !integerInRange(operation.durationTurns, 1, 1_000)) {
        diagnostics.push(diagnostic('normalization.durationInvalid', `${path}.operation.durationTurns`, 'turn duration must be a positive bounded integer', sourcePath));
    }
    if ((operation.kind === 'move' || operation.kind === 'moveToCell') &&
        !integerInRange(operation.maximumDistance, 1, 64)) {
        diagnostics.push(diagnostic('normalization.movementBoundInvalid', `${path}.operation.maximumDistance`, 'movement maximum must be an integer between 1 and 64', sourcePath));
    }
    if (operation.kind === 'openReaction') {
        if (operation.options.length < 1 || operation.options.length > 16) {
            diagnostics.push(diagnostic('normalization.reactionOptionsInvalid', `${path}.operation.options`, 'a reaction must declare between 1 and 16 options', sourcePath));
        }
        const optionIds = new Set();
        for (const [index, option] of operation.options.entries()) {
            if (optionIds.has(option.id)) {
                diagnostics.push(diagnostic('normalization.reactionOptionDuplicate', `${path}.operation.options[${index}].id`, `duplicate reaction option ${option.id}`, sourcePath));
            }
            optionIds.add(option.id);
            if (!integerInRange(option.damageReduction, 0, 10_000)) {
                diagnostics.push(diagnostic('normalization.reactionReductionInvalid', `${path}.operation.options[${index}].damageReduction`, 'reaction damage reduction must be a bounded non-negative integer', sourcePath));
            }
            if (option.activation !== undefined) {
                validateActivation(option.activation, 'reaction', `${path}.operation.options[${index}].activation`, diagnostics, sourcePath);
            }
        }
    }
}
function collectAction(action, collection) {
    for (const cost of action.costs) {
        collection.resources.add(cost.resourceId);
        collection.capabilities.add('capability.resources');
    }
    if (action.activation !== undefined) {
        collection.capabilities.add('capability.activation-budgets');
    }
    switch (action.check.kind) {
        case 'noRoll':
            break;
        case 'attack':
            collection.defenses.add(action.check.defenseId);
            collection.capabilities.add('capability.defenses');
            collection.capabilities.add('capability.random');
            collectFormula(action.check.modifier, collection);
            break;
        case 'savingThrow':
            collection.defenses.add(action.check.defenseId);
            collection.capabilities.add('capability.defenses');
            collection.capabilities.add('capability.random');
            collectFormula(action.check.difficulty, collection);
            break;
    }
    collectProgram(action.program, collection);
}
function normalizedRollScope(action) {
    if (action.check.kind === 'noRoll')
        return 'none';
    return action.rollScope === 'shared' ? 'shared' : 'perTarget';
}
function collectProgram(program, collection) {
    switch (program.kind) {
        case 'operation':
            collectOperation(program.operation, collection);
            return;
        case 'sequence':
            for (const step of program.steps)
                collectProgram(step, collection);
            return;
        case 'when':
            collectPredicate(program.predicate, collection);
            collectProgram(program.then, collection);
            if (program.otherwise !== undefined)
                collectProgram(program.otherwise, collection);
            return;
        case 'repeat':
        case 'forEachTarget':
            collectProgram(program.body, collection);
            return;
        case 'onCheck':
            for (const branch of [
                program.hit,
                program.miss,
                program.saved,
                program.failed,
                program.noRoll,
            ]) {
                if (branch !== undefined)
                    collectProgram(branch, collection);
            }
    }
}
function collectOperation(operation, collection) {
    const operationId = OPERATION_IDS[operation.kind];
    collection.operations.add(operationId);
    switch (operation.kind) {
        case 'damage':
            collection.capabilities.add('capability.vitality');
            for (const part of operation.parts)
                collectFormula(part.amount, collection);
            return;
        case 'heal':
            collection.capabilities.add('capability.vitality');
            collectFormula(operation.amount, collection);
            return;
        case 'changeResource':
            collection.resources.add(operation.resourceId);
            collection.capabilities.add('capability.resources');
            collectFormula(operation.delta, collection);
            return;
        case 'applyModifier':
            collection.modifiers.add(operation.modifierId);
            collection.capabilities.add('capability.modifiers');
            collectFormula(operation.value, collection);
            return;
        case 'applyEffect':
            collection.capabilities.add('capability.effects');
            collectFormula(operation.rank, collection);
            return;
        case 'removeEffect':
            collection.capabilities.add('capability.effects');
            return;
        case 'move':
            collection.capabilities.add('capability.position');
            collectFormula(operation.deltaX, collection);
            collectFormula(operation.deltaY, collection);
            return;
        case 'moveToCell':
            collection.capabilities.add('capability.position');
            return;
        case 'openReaction':
            collection.capabilities.add('capability.reactions');
            if (operation.options.some((option) => option.activation !== undefined)) {
                collection.capabilities.add('capability.activation-budgets');
            }
    }
}
function collectFormula(formula, collection) {
    switch (formula.kind) {
        case 'constant':
            return;
        case 'readStat':
            collection.stats.add(formula.statId);
            collection.capabilities.add('capability.stats');
            return;
        case 'add':
            for (const term of formula.terms)
                collectFormula(term, collection);
            return;
        case 'dice':
            collection.capabilities.add('capability.random');
            return;
        case 'half':
            collectFormula(formula.value, collection);
    }
}
function collectPredicate(predicate, collection) {
    switch (predicate.kind) {
        case 'always':
            return;
        case 'compare':
            collectFormula(predicate.left, collection);
            collectFormula(predicate.right, collection);
            return;
        case 'not':
            collectPredicate(predicate.predicate, collection);
            return;
        case 'all':
        case 'any':
            for (const entry of predicate.predicates)
                collectPredicate(entry, collection);
    }
}
function emptyCollection() {
    return {
        operations: new Set(),
        capabilities: new Set(),
        stats: new Set(),
        defenses: new Set(),
        resources: new Set(),
        modifiers: new Set(),
    };
}
function sorted(values) {
    return [...values].sort(compareText);
}
function canonicalValue(value) {
    if (Array.isArray(value))
        return value.map(canonicalValue);
    if (value !== null && typeof value === 'object') {
        return Object.fromEntries(Object.entries(value)
            .sort(([left], [right]) => compareText(left, right))
            .map(([key, entry]) => [key, canonicalValue(entry)]));
    }
    return value;
}
function compareText(left, right) {
    return left < right ? -1 : left > right ? 1 : 0;
}
function requireText(value, path, field, diagnostics, sourcePath) {
    if (value.trim() === '') {
        diagnostics.push(diagnostic('normalization.valueEmpty', path, `${field} must not be empty`, sourcePath));
    }
}
function integerInRange(value, minimum, maximum) {
    return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}
function diagnostic(code, path, message, sourcePath) {
    return sourcePath === undefined
        ? { stage: 'normalization', severity: 'error', code, path, message }
        : { stage: 'normalization', severity: 'error', code, path, message, sourcePath };
}
function deepFreeze(value) {
    if (value !== null && typeof value === 'object' && !Object.isFrozen(value)) {
        for (const entry of Object.values(value))
            deepFreeze(entry);
        Object.freeze(value);
    }
    return value;
}
function rejectExecutableValues(value, path, diagnostics, visited) {
    if (typeof value === 'function') {
        diagnostics.push(diagnostic('normalization.executableValueForbidden', path, 'authored packages must contain data only'));
        return;
    }
    if (value === null || typeof value !== 'object' || visited.has(value))
        return;
    visited.add(value);
    if (Array.isArray(value)) {
        for (const [index, entry] of value.entries()) {
            rejectExecutableValues(entry, `${path}[${index}]`, diagnostics, visited);
        }
        return;
    }
    for (const [key, entry] of Object.entries(value)) {
        rejectExecutableValues(entry, `${path}.${key}`, diagnostics, visited);
    }
}
//# sourceMappingURL=normalize.js.map