import {
  RPG_CAPABILITY_VERSIONS,
  RPG_IR_IDENTITY,
  RPG_IR_MAJOR,
  RPG_OPERATION_VERSIONS,
} from '@asha-rpg/ir';
import type {
  NormalizedRpgIr,
  RpgCapabilityId,
  RpgIrAction,
  RpgIrFormula,
  RpgIrOperation,
  RpgIrPredicate,
  RpgIrProgram,
  RpgOperationId,
} from '@asha-rpg/ir';

import type {
  AuthoredAction,
  AuthoredPackage,
  AuthoringDiagnostic,
  AuthoringProgram,
  NormalizationResult,
} from './types.js';

interface Collection {
  readonly operations: Set<RpgOperationId>;
  readonly capabilities: Set<RpgCapabilityId>;
  readonly stats: Set<import('@asha-rpg/ir').RpgStatId>;
  readonly defenses: Set<import('@asha-rpg/ir').RpgDefenseId>;
  readonly resources: Set<import('@asha-rpg/ir').RpgResourceId>;
  readonly modifiers: Set<import('@asha-rpg/ir').RpgModifierId>;
}

const OPERATION_IDS: Record<RpgIrOperation['kind'], RpgOperationId> = {
  damage: 'operation.damage',
  heal: 'operation.heal',
  changeResource: 'operation.changeResource',
  applyModifier: 'operation.applyModifier',
  move: 'operation.move',
  openReaction: 'operation.openReaction',
};

const NO_DIAGNOSTICS: readonly [] = Object.freeze([]);

export function normalizePackage(source: AuthoredPackage): NormalizationResult {
  const diagnostics: AuthoringDiagnostic[] = [];
  rejectExecutableValues(source, '$', diagnostics, new WeakSet());
  requireText(source.id, '$.package.id', 'package id', diagnostics);
  requireText(source.version, '$.package.version', 'package version', diagnostics);

  const actions = source.sources.flatMap((entry) => entry.actions);
  const actionIds = new Set<string>();
  for (const [index, action] of actions.entries()) {
    const path = `$.actions[${index}]`;
    validateAction(action, path, diagnostics);
    if (actionIds.has(action.id)) {
      diagnostics.push(
        diagnostic(
          'normalization.duplicateActionId',
          `${path}.id`,
          `duplicate action id ${action.id}`,
          action.sourcePath,
        ),
      );
    }
    actionIds.add(action.id);
  }
  if (diagnostics.length > 0) {
    return Object.freeze({ ok: false, diagnostics: Object.freeze(diagnostics) });
  }

  const collection = emptyCollection();
  for (const action of actions) collectAction(action, collection);
  const normalizedActions = actions
    .map(normalizeAction)
    .sort((left, right) => compareText(left.id, right.id));
  const artifact: NormalizedRpgIr = deepFreeze({
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
        kind: 'operation' as const,
        id,
        version: RPG_OPERATION_VERSIONS[id],
      })),
      ...sorted(collection.capabilities).map((id) => ({
        kind: 'capability' as const,
        id,
        version: RPG_CAPABILITY_VERSIONS[id],
      })),
    ],
    actions: normalizedActions,
  });
  return Object.freeze({ ok: true, artifact, diagnostics: NO_DIAGNOSTICS });
}

export function canonicalRpgJson(artifact: NormalizedRpgIr): string {
  return JSON.stringify(canonicalValue(artifact));
}

export function normalizeAction(action: AuthoredAction): RpgIrAction {
  return {
    id: action.id,
    name: action.name,
    sourcePath: action.sourcePath,
    targets: action.targets,
    check: action.check,
    rollScope: normalizedRollScope(action),
    costs: [...action.costs],
    program: { kind: 'atomic', body: normalizeProgram(action.program) },
  };
}

function normalizeProgram(program: AuthoringProgram): RpgIrProgram {
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
  }
}

function copyCheckBranches(
  source: Extract<AuthoringProgram, { kind: 'onCheck' }>,
): Extract<RpgIrProgram, { kind: 'onCheck' }> {
  return {
    kind: 'onCheck',
    ...(source.hit === undefined ? {} : { hit: normalizeProgram(source.hit) }),
    ...(source.miss === undefined ? {} : { miss: normalizeProgram(source.miss) }),
    ...(source.saved === undefined ? {} : { saved: normalizeProgram(source.saved) }),
    ...(source.failed === undefined ? {} : { failed: normalizeProgram(source.failed) }),
    ...(source.noRoll === undefined ? {} : { noRoll: normalizeProgram(source.noRoll) }),
  };
}

function validateAction(
  action: AuthoredAction,
  path: string,
  diagnostics: AuthoringDiagnostic[],
): void {
  requireText(action.id, `${path}.id`, 'action id', diagnostics, action.sourcePath);
  requireText(action.name, `${path}.name`, 'action name', diagnostics, action.sourcePath);
  requireText(action.sourcePath, `${path}.sourcePath`, 'source path', diagnostics);
  if (action.check.kind === 'noRoll' && action.rollScope !== 'none') {
    diagnostics.push(
      diagnostic(
        'normalization.rollScopeInvalid',
        `${path}.rollScope`,
        'no-roll checks require roll scope none',
        action.sourcePath,
      ),
    );
  }
  if (
    action.check.kind !== 'noRoll' &&
    action.rollScope !== 'shared' &&
    action.rollScope !== 'perTarget'
  ) {
    diagnostics.push(
      diagnostic(
        'normalization.rollScopeInvalid',
        `${path}.rollScope`,
        'rolled checks require shared or per-target scope',
        action.sourcePath,
      ),
    );
  }
  if (!integerInRange(action.targets.maximumTargets, 1, 32)) {
    diagnostics.push(
      diagnostic(
        'normalization.targetBoundInvalid',
        `${path}.targets.maximumTargets`,
        'target maximum must be an integer between 1 and 32',
        action.sourcePath,
      ),
    );
  }
  for (const [index, cost] of action.costs.entries()) {
    if (!integerInRange(cost.amount, 1, Number.MAX_SAFE_INTEGER)) {
      diagnostics.push(
        diagnostic(
          'normalization.costInvalid',
          `${path}.costs[${index}].amount`,
          'resource cost must be a positive safe integer',
          action.sourcePath,
        ),
      );
    }
  }
  validateProgram(
    action.program,
    `${path}.program`,
    1,
    action.check.kind,
    diagnostics,
    action.sourcePath,
  );
}

function validateProgram(
  program: AuthoringProgram,
  path: string,
  depth: number,
  checkKind: import('@asha-rpg/ir').RpgIrCheck['kind'],
  diagnostics: AuthoringDiagnostic[],
  sourcePath: string,
): void {
  if (depth > 16) {
    diagnostics.push(
      diagnostic(
        'normalization.programDepthExceeded',
        path,
        'program depth exceeds 16',
        sourcePath,
      ),
    );
    return;
  }
  switch (program.kind) {
    case 'operation':
      if (program.timing.kind !== 'immediate') {
        diagnostics.push(
          diagnostic(
            'normalization.timingUnsupported',
            `${path}.timing`,
            'the active vocabulary supports immediate timing only',
            sourcePath,
          ),
        );
      }
      validateOperation(program.operation, path, diagnostics, sourcePath);
      return;
    case 'sequence':
      if (program.steps.length === 0) {
        diagnostics.push(
          diagnostic('normalization.emptySequence', path, 'sequence is empty', sourcePath),
        );
      }
      for (const [index, step] of program.steps.entries()) {
        validateProgram(
          step,
          `${path}.steps[${index}]`,
          depth + 1,
          checkKind,
          diagnostics,
          sourcePath,
        );
      }
      return;
    case 'when':
      validateProgram(
        program.then,
        `${path}.then`,
        depth + 1,
        checkKind,
        diagnostics,
        sourcePath,
      );
      if (program.otherwise !== undefined) {
        validateProgram(
          program.otherwise,
          `${path}.otherwise`,
          depth + 1,
          checkKind,
          diagnostics,
          sourcePath,
        );
      }
      return;
    case 'repeat':
      if (!integerInRange(program.count, 1, 16)) {
        diagnostics.push(
          diagnostic(
            'normalization.repeatBoundInvalid',
            `${path}.count`,
            'repeat count must be an integer between 1 and 16',
            sourcePath,
          ),
        );
      }
      validateProgram(
        program.body,
        `${path}.body`,
        depth + 1,
        checkKind,
        diagnostics,
        sourcePath,
      );
      return;
    case 'forEachTarget':
      if (!integerInRange(program.maximum, 1, 32)) {
        diagnostics.push(
          diagnostic(
            'normalization.targetBoundInvalid',
            `${path}.maximum`,
            'per-target maximum must be an integer between 1 and 32',
            sourcePath,
          ),
        );
      }
      validateProgram(
        program.body,
        `${path}.body`,
        depth + 1,
        checkKind,
        diagnostics,
        sourcePath,
      );
      return;
    case 'onCheck': {
      const hasIncompatibleBranch =
        (checkKind === 'noRoll' &&
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
            program.noRoll !== undefined));
      if (hasIncompatibleBranch) {
        diagnostics.push(
          diagnostic(
            'normalization.checkBranchIncompatible',
            path,
            'check branch contains an outcome unavailable to the selected check',
            sourcePath,
          ),
        );
      }
      const branches = [program.hit, program.miss, program.saved, program.failed, program.noRoll];
      if (branches.every((branch) => branch === undefined)) {
        diagnostics.push(
          diagnostic(
            'normalization.emptyCheckBranch',
            path,
            'check branch has no outcomes',
            sourcePath,
          ),
        );
      }
      for (const [index, branch] of branches.entries()) {
        if (branch !== undefined) {
          validateProgram(
            branch,
            `${path}.branches[${index}]`,
            depth + 1,
            checkKind,
            diagnostics,
            sourcePath,
          );
        }
      }
    }
  }
}

function validateOperation(
  operation: RpgIrOperation,
  path: string,
  diagnostics: AuthoringDiagnostic[],
  sourcePath: string,
): void {
  if (operation.kind === 'applyModifier' && !integerInRange(operation.durationTurns, 1, 1_000)) {
    diagnostics.push(
      diagnostic(
        'normalization.durationInvalid',
        `${path}.operation.durationTurns`,
        'turn duration must be a positive bounded integer',
        sourcePath,
      ),
    );
  }
  if (
    operation.kind === 'move' &&
    !integerInRange(operation.maximumDistance, 1, 64)
  ) {
    diagnostics.push(
      diagnostic(
        'normalization.movementBoundInvalid',
        `${path}.operation.maximumDistance`,
        'movement maximum must be an integer between 1 and 64',
        sourcePath,
      ),
    );
  }
  if (operation.kind === 'openReaction') {
    if (operation.options.length < 1 || operation.options.length > 16) {
      diagnostics.push(
        diagnostic(
          'normalization.reactionOptionsInvalid',
          `${path}.operation.options`,
          'a reaction must declare between 1 and 16 options',
          sourcePath,
        ),
      );
    }
    const optionIds = new Set<string>();
    for (const [index, option] of operation.options.entries()) {
      if (optionIds.has(option.id)) {
        diagnostics.push(
          diagnostic(
            'normalization.reactionOptionDuplicate',
            `${path}.operation.options[${index}].id`,
            `duplicate reaction option ${option.id}`,
            sourcePath,
          ),
        );
      }
      optionIds.add(option.id);
      if (!integerInRange(option.damageReduction, 0, 10_000)) {
        diagnostics.push(
          diagnostic(
            'normalization.reactionReductionInvalid',
            `${path}.operation.options[${index}].damageReduction`,
            'reaction damage reduction must be a bounded non-negative integer',
            sourcePath,
          ),
        );
      }
    }
  }
}

function collectAction(action: AuthoredAction, collection: Collection): void {
  for (const cost of action.costs) {
    collection.resources.add(cost.resourceId);
    collection.capabilities.add('capability.resources');
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

function normalizedRollScope(action: AuthoredAction): 'shared' | 'perTarget' | 'none' {
  if (action.check.kind === 'noRoll') return 'none';
  return action.rollScope === 'shared' ? 'shared' : 'perTarget';
}

function collectProgram(program: AuthoringProgram, collection: Collection): void {
  switch (program.kind) {
    case 'operation':
      collectOperation(program.operation, collection);
      return;
    case 'sequence':
      for (const step of program.steps) collectProgram(step, collection);
      return;
    case 'when':
      collectPredicate(program.predicate, collection);
      collectProgram(program.then, collection);
      if (program.otherwise !== undefined) collectProgram(program.otherwise, collection);
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
        if (branch !== undefined) collectProgram(branch, collection);
      }
  }
}

function collectOperation(operation: RpgIrOperation, collection: Collection): void {
  const operationId = OPERATION_IDS[operation.kind];
  collection.operations.add(operationId);
  switch (operation.kind) {
    case 'damage':
      collection.capabilities.add('capability.vitality');
      collectFormula(operation.amount, collection);
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
    case 'move':
      collection.capabilities.add('capability.position');
      collectFormula(operation.deltaX, collection);
      collectFormula(operation.deltaY, collection);
      return;
    case 'openReaction':
      collection.capabilities.add('capability.reactions');
  }
}

function collectFormula(formula: RpgIrFormula, collection: Collection): void {
  switch (formula.kind) {
    case 'constant':
      return;
    case 'readStat':
      collection.stats.add(formula.statId);
      collection.capabilities.add('capability.stats');
      return;
    case 'add':
      for (const term of formula.terms) collectFormula(term, collection);
      return;
    case 'dice':
      collection.capabilities.add('capability.random');
      return;
    case 'half':
      collectFormula(formula.value, collection);
  }
}

function collectPredicate(predicate: RpgIrPredicate, collection: Collection): void {
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
      for (const entry of predicate.predicates) collectPredicate(entry, collection);
  }
}

function emptyCollection(): Collection {
  return {
    operations: new Set(),
    capabilities: new Set(),
    stats: new Set(),
    defenses: new Set(),
    resources: new Set(),
    modifiers: new Set(),
  };
}

function sorted<Value extends string>(values: ReadonlySet<Value>): Value[] {
  return [...values].sort(compareText);
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => compareText(left, right))
        .map(([key, entry]) => [key, canonicalValue(entry)]),
    );
  }
  return value;
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function requireText(
  value: string,
  path: string,
  field: string,
  diagnostics: AuthoringDiagnostic[],
  sourcePath?: string,
): void {
  if (value.trim() === '') {
    diagnostics.push(
      diagnostic('normalization.valueEmpty', path, `${field} must not be empty`, sourcePath),
    );
  }
}

function integerInRange(value: number, minimum: number, maximum: number): boolean {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}

function diagnostic(
  code: string,
  path: string,
  message: string,
  sourcePath?: string,
): AuthoringDiagnostic {
  return sourcePath === undefined
    ? { stage: 'normalization', severity: 'error', code, path, message }
    : { stage: 'normalization', severity: 'error', code, path, message, sourcePath };
}

function deepFreeze<Value>(value: Value): Value {
  if (value !== null && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const entry of Object.values(value)) deepFreeze(entry);
    Object.freeze(value);
  }
  return value;
}

function rejectExecutableValues(
  value: unknown,
  path: string,
  diagnostics: AuthoringDiagnostic[],
  visited: WeakSet<object>,
): void {
  if (typeof value === 'function') {
    diagnostics.push(
      diagnostic(
        'normalization.executableValueForbidden',
        path,
        'authored packages must contain data only',
      ),
    );
    return;
  }
  if (value === null || typeof value !== 'object' || visited.has(value)) return;
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
