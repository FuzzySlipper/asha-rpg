import type {
  RpgActionId,
  RpgDamageType,
  RpgDefenseId,
  RpgIrComparison,
  RpgIrFormula,
  RpgIrPredicate,
  RpgIrResourceCost,
  RpgIrSubject,
  RpgIrTargetSelector,
  RpgModifierId,
  RpgResourceId,
  RpgStackingGroup,
  RpgStatId,
} from '@asha-rpg/ir';

import type {
  ActionInput,
  AuthoredAction,
  AuthoredActionSource,
  AuthoredPackage,
  AuthoringDuration,
  AuthoringProgram,
  AuthoringStacking,
  AuthoringTiming,
  CheckBranchInput,
} from './types.js';

export function actionId(value: string): RpgActionId {
  return value as RpgActionId;
}

export function statId(value: string): RpgStatId {
  return value as RpgStatId;
}

export function defenseId(value: string): RpgDefenseId {
  return value as RpgDefenseId;
}

export function resourceId(value: string): RpgResourceId {
  return value as RpgResourceId;
}

export function modifierId(value: string): RpgModifierId {
  return value as RpgModifierId;
}

export function damageType(value: string): RpgDamageType {
  return value as RpgDamageType;
}

export function stackingGroup(value: string): RpgStackingGroup {
  return value as RpgStackingGroup;
}

export function targets(options: {
  readonly team: 'hostile' | 'ally' | 'any';
  readonly maximumRange: number;
  readonly maximumTargets?: number;
}): RpgIrTargetSelector {
  return frozen({
    team: options.team,
    maximumRange: options.maximumRange,
    maximumTargets: options.maximumTargets ?? 1,
  });
}

export function hostile(options: {
  readonly range: number;
  readonly maximum?: number;
}): RpgIrTargetSelector {
  return options.maximum === undefined
    ? targets({ team: 'hostile', maximumRange: options.range })
    : targets({
        team: 'hostile',
        maximumRange: options.range,
        maximumTargets: options.maximum,
      });
}

export function ally(options: {
  readonly range: number;
  readonly maximum?: number;
}): RpgIrTargetSelector {
  return options.maximum === undefined
    ? targets({ team: 'ally', maximumRange: options.range })
    : targets({
        team: 'ally',
        maximumRange: options.range,
        maximumTargets: options.maximum,
      });
}

export function constant(value: number): RpgIrFormula {
  return frozen({ kind: 'constant', value });
}

export function readStat(subject: RpgIrSubject, id: RpgStatId): RpgIrFormula {
  return frozen({ kind: 'readStat', subject, statId: id });
}

export function add(...terms: readonly RpgIrFormula[]): RpgIrFormula {
  return frozen({ kind: 'add', terms: frozenList(terms) });
}

export function dice(options: {
  readonly count: number;
  readonly sides: number;
  readonly bonus?: number;
}): RpgIrFormula {
  return frozen({
    kind: 'dice',
    count: options.count,
    sides: options.sides,
    bonus: options.bonus ?? 0,
  });
}

export function half(value: RpgIrFormula): RpgIrFormula {
  return frozen({ kind: 'half', value });
}

export function always(): RpgIrPredicate {
  return frozen({ kind: 'always' });
}

export function compare(
  left: RpgIrFormula,
  comparison: RpgIrComparison,
  right: RpgIrFormula,
): RpgIrPredicate {
  return frozen({ kind: 'compare', left, comparison, right });
}

export function not(predicate: RpgIrPredicate): RpgIrPredicate {
  return frozen({ kind: 'not', predicate });
}

export function all(...predicates: readonly RpgIrPredicate[]): RpgIrPredicate {
  return frozen({ kind: 'all', predicates: frozenList(predicates) });
}

export function any(...predicates: readonly RpgIrPredicate[]): RpgIrPredicate {
  return frozen({ kind: 'any', predicates: frozenList(predicates) });
}

export function noRoll(): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'noRoll' }> {
  return frozen({ kind: 'noRoll' });
}

export function attack(options: {
  readonly modifier: RpgIrFormula;
  readonly defense: RpgDefenseId;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'attack' }> {
  return frozen({
    kind: 'attack',
    modifier: options.modifier,
    defenseId: options.defense,
  });
}

export function savingThrow(options: {
  readonly difficulty: RpgIrFormula;
  readonly defense: RpgDefenseId;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'savingThrow' }> {
  return frozen({
    kind: 'savingThrow',
    difficulty: options.difficulty,
    defenseId: options.defense,
  });
}

export function spend(resource: RpgResourceId, amount: number): RpgIrResourceCost {
  return frozen({ resourceId: resource, amount });
}

export function immediate(): AuthoringTiming {
  return frozen({ kind: 'immediate' });
}

export function turns(count: number): AuthoringDuration {
  return frozen({ kind: 'turns', count });
}

export function replace(group: RpgStackingGroup): AuthoringStacking {
  return frozen({ kind: 'replace', group });
}

export function refresh(group: RpgStackingGroup): AuthoringStacking {
  return frozen({ kind: 'refresh', group });
}

export function damage(options: {
  readonly amount: RpgIrFormula;
  readonly type: RpgDamageType;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({ kind: 'damage', amount: options.amount, damageType: options.type }),
    options.timing,
  );
}

export function heal(options: {
  readonly amount: RpgIrFormula;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(frozen({ kind: 'heal', amount: options.amount }), options.timing);
}

export function changeResource(options: {
  readonly subject: RpgIrSubject;
  readonly resource: RpgResourceId;
  readonly delta: RpgIrFormula;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({
      kind: 'changeResource',
      subject: options.subject,
      resourceId: options.resource,
      delta: options.delta,
    }),
    options.timing,
  );
}

export function applyModifier(options: {
  readonly modifier: RpgModifierId;
  readonly value: RpgIrFormula;
  readonly duration: AuthoringDuration;
  readonly stacking: AuthoringStacking;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({
      kind: 'applyModifier',
      modifierId: options.modifier,
      stackingGroup: options.stacking.group,
      stacking: options.stacking.kind,
      value: options.value,
      durationTurns: options.duration.count,
    }),
    options.timing,
  );
}

export function moveEntity(options: {
  readonly subject: RpgIrSubject;
  readonly deltaX: RpgIrFormula;
  readonly deltaY: RpgIrFormula;
  readonly maximumDistance: number;
  readonly provokes: boolean;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({
      kind: 'move',
      subject: options.subject,
      deltaX: options.deltaX,
      deltaY: options.deltaY,
      maximumDistance: options.maximumDistance,
      provokes: options.provokes,
    }),
    options.timing,
  );
}

export function sequence(...steps: readonly AuthoringProgram[]): AuthoringProgram {
  return frozen({ kind: 'sequence', steps: frozenList(steps) });
}

export function when(
  predicate: RpgIrPredicate,
  then: AuthoringProgram,
  otherwise?: AuthoringProgram,
): AuthoringProgram {
  return otherwise === undefined
    ? frozen({ kind: 'when', predicate, then })
    : frozen({ kind: 'when', predicate, then, otherwise });
}

export function repeat(count: number, body: AuthoringProgram): AuthoringProgram {
  return frozen({ kind: 'repeat', count, body });
}

export function forEachTarget(maximum: number, body: AuthoringProgram): AuthoringProgram {
  return frozen({ kind: 'forEachTarget', maximum, body });
}

export function onCheck(branches: CheckBranchInput): AuthoringProgram {
  return frozen({ kind: 'onCheck', ...branches });
}

export function action(input: ActionInput): AuthoredAction {
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

export function defineActions(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource {
  return source('actions', id, actions);
}

export function defineArchetype(
  id: string,
  actions: readonly AuthoredAction[],
): AuthoredActionSource {
  return source('archetype', id, actions);
}

export function defineItem(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource {
  return source('item', id, actions);
}

export function defineScenario(
  id: string,
  actions: readonly AuthoredAction[],
): AuthoredActionSource {
  return source('scenario', id, actions);
}

export function definePackage(options: {
  readonly id: string;
  readonly version: string;
  readonly sources: readonly AuthoredActionSource[];
}): AuthoredPackage {
  return frozen({
    id: options.id,
    version: options.version,
    sources: frozenList(options.sources),
  });
}

function operation(
  declaration: import('@asha-rpg/ir').RpgIrOperation,
  timing: AuthoringTiming = immediate(),
): AuthoringProgram {
  return frozen({ kind: 'operation', operation: declaration, timing });
}

function source(
  kind: AuthoredActionSource['kind'],
  id: string,
  actions: readonly AuthoredAction[],
): AuthoredActionSource {
  return frozen({ kind, id, actions: frozenList(actions) });
}

function frozen<Value extends object>(value: Value): Readonly<Value> {
  return Object.freeze(value);
}

function frozenList<Value>(values: readonly Value[]): readonly Value[] {
  return Object.freeze([...values]);
}
