import type {
  RpgActionId,
  RpgIrActivation,
  RpgIrActivationTiming,
  RpgIrComparison,
  RpgIrFormula,
  RpgIrPoolAxisValue,
  RpgIrPoolDieTerm,
  RpgIrPredicate,
  RpgIrResourceCost,
  RpgIrScalarExpression,
  RpgIrSubject,
  RpgIrTargetSelector,
  RpgReactionId,
  RpgReactionOptionId,
  RpgStackingGroup,
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
  OutcomeBranchInput,
} from './types.js';
import {
  catalogDefinitionId,
  retainCatalogOwnership,
} from './catalogs.js';
import type { ContentCatalogReference } from './catalogs.js';
import {
  retainRulesetValueOwnership,
  rulesetValueId,
} from './ruleset-builders.js';
import type { ContentDefinitionReference } from './play-bundle-types.js';

const authoredDefinitionOwnership: unique symbol = Symbol(
  'asha-rpg.authored-definition-ownership',
);

export interface AuthoredDefinitionOwnership {
  readonly field: string;
  readonly reference: ContentDefinitionReference;
}
import type {
  RulesetCalculationSelectorReference,
  RulesetActivationBudgetReference,
  RulesetHeterogeneousPoolProfileReference,
  RulesetScalarTestProfileReference,
  RulesetValueReference,
} from './ruleset-builders.js';

type AuthoredStatReference =
  | ContentCatalogReference<'stat', string>
  | RulesetValueReference<'stat', string, string>;
type AuthoredDefenseReference =
  | ContentCatalogReference<'defense', string>
  | RulesetValueReference<'defense', string, string>;

export function actionId(value: string): RpgActionId {
  return checkedIdentifier(value, 'action id') as RpgActionId;
}

export function stackingGroup(value: string): RpgStackingGroup {
  return checkedIdentifier(value, 'stacking group') as RpgStackingGroup;
}

export function reactionId(value: string): RpgReactionId {
  return checkedIdentifier(value, 'reaction id') as RpgReactionId;
}

export function reactionOptionId(value: string): RpgReactionOptionId {
  return checkedIdentifier(value, 'reaction option id') as RpgReactionOptionId;
}

export function targets(options: {
  readonly team: 'hostile' | 'ally' | 'any';
  readonly maximumRange: number;
  readonly maximumTargets?: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  return frozen({
    kind: 'participant',
    team: options.team,
    maximumRange: options.maximumRange,
    maximumTargets: options.maximumTargets ?? 1,
    lineOfEffect: options.lineOfEffect ?? 'ignored',
  });
}

export function cells(options: {
  readonly range: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  return frozen({
    kind: 'cell',
    team: 'any',
    maximumRange: options.range,
    maximumTargets: 1,
    lineOfEffect: options.lineOfEffect ?? 'ignored',
  });
}

export function diamondArea(options: {
  readonly range: number;
  readonly radius: number;
  readonly team: 'hostile' | 'ally' | 'any';
  readonly livingRequired?: boolean;
  readonly minimumTargets?: number;
  readonly maximumTargets?: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  return frozen({
    kind: 'area',
    team: options.team,
    maximumRange: options.range,
    maximumTargets: options.maximumTargets ?? 32,
    lineOfEffect: options.lineOfEffect ?? 'ignored',
    area: frozen({
      schema: frozen({ identity: 'asha.rpg.area-selector', version: 1 }),
      origin: 'anchor',
      shape: frozen({ kind: 'diamond', radius: options.radius }),
      livingRequired: options.livingRequired ?? true,
      minimumTargets: options.minimumTargets ?? 1,
    }),
  });
}

export function orthogonalLineArea(options: {
  readonly range: number;
  readonly length: number;
  readonly team: 'hostile' | 'ally' | 'any';
  readonly livingRequired?: boolean;
  readonly minimumTargets?: number;
  readonly maximumTargets?: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  return frozen({
    kind: 'area',
    team: options.team,
    maximumRange: options.range,
    maximumTargets: options.maximumTargets ?? 32,
    lineOfEffect: options.lineOfEffect ?? 'ignored',
    area: frozen({
      schema: frozen({ identity: 'asha.rpg.area-selector', version: 1 }),
      origin: 'actor',
      shape: frozen({ kind: 'orthogonalLine', length: options.length }),
      livingRequired: options.livingRequired ?? true,
      minimumTargets: options.minimumTargets ?? 1,
    }),
  });
}

export function hostile(options: {
  readonly range: number;
  readonly maximum?: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  const lineOfEffect = options.lineOfEffect ?? 'ignored';
  return options.maximum === undefined
    ? targets({
        team: 'hostile',
        maximumRange: options.range,
        lineOfEffect,
      })
    : targets({
        team: 'hostile',
        maximumRange: options.range,
        maximumTargets: options.maximum,
        lineOfEffect,
      });
}

export function ally(options: {
  readonly range: number;
  readonly maximum?: number;
  readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector {
  const lineOfEffect = options.lineOfEffect ?? 'ignored';
  return options.maximum === undefined
    ? targets({
        team: 'ally',
        maximumRange: options.range,
        lineOfEffect,
      })
    : targets({
        team: 'ally',
        maximumRange: options.range,
        maximumTargets: options.maximum,
        lineOfEffect,
      });
}

export function constant(value: number): RpgIrScalarExpression {
  return frozen({ kind: 'constant', value });
}

export function readStat(
  subject: RpgIrSubject,
  id: AuthoredStatReference,
): RpgIrScalarExpression {
  return frozenWithCatalogOwnership(
    { kind: 'readStat' as const, subject, statId: authoredValueId(id) },
    'statId',
    id,
  );
}

export function add(
  ...terms: readonly RpgIrScalarExpression[]
): RpgIrScalarExpression;
export function add(...terms: readonly RpgIrFormula[]): RpgIrFormula;
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

export function half(value: RpgIrScalarExpression): RpgIrScalarExpression;
export function half(value: RpgIrFormula): RpgIrFormula;
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
  readonly defense: AuthoredDefenseReference;
  readonly contributionSelector?: RulesetCalculationSelectorReference<string, string>;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'attack' }> {
  return frozenWithCatalogOwnership(
    {
      kind: 'attack' as const,
      modifier: options.modifier,
      defenseId: authoredValueId(options.defense),
      ...(options.contributionSelector === undefined
        ? {}
        : { contributionSelector: options.contributionSelector }),
    },
    'defenseId',
    options.defense,
  );
}

export function savingThrow(options: {
  readonly difficulty: RpgIrFormula;
  readonly defense: AuthoredDefenseReference;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'savingThrow' }> {
  return frozenWithCatalogOwnership(
    {
      kind: 'savingThrow' as const,
      difficulty: options.difficulty,
      defenseId: authoredValueId(options.defense),
    },
    'defenseId',
    options.defense,
  );
}

export function scalarTest(options: {
  readonly profile: RulesetScalarTestProfileReference<string, string>;
  readonly base: RpgIrScalarExpression;
  readonly difficulty:
    | {
        readonly kind: 'explicit';
        readonly value: RpgIrScalarExpression;
      }
    | {
        readonly kind: 'targetDefense';
        readonly defense: AuthoredDefenseReference;
      };
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'scalarTest' }> {
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
  return frozenWithCatalogOwnership(
    {
      kind: 'scalarTest' as const,
      profile: options.profile,
      base: options.base,
      difficulty: {
        kind: 'targetDefense' as const,
        defenseId: authoredValueId(options.difficulty.defense),
      },
    },
    'difficulty.defenseId',
    options.difficulty.defense,
  );
}

export function heterogeneousPool(options: {
  readonly profile: RulesetHeterogeneousPoolProfileReference<string, string>;
  readonly baseDice: readonly RpgIrPoolDieTerm[];
  readonly automaticAxes?: readonly RpgIrPoolAxisValue[];
}): Extract<import('@asha-rpg/ir').RpgIrCheck, { kind: 'heterogeneousPool' }> {
  return frozen({
    kind: 'heterogeneousPool',
    profile: options.profile,
    baseDice: [...options.baseDice].sort((left, right) =>
      left.dieTypeId.localeCompare(right.dieTypeId),
    ),
    automaticAxes: [...(options.automaticAxes ?? [])].sort((left, right) =>
      left.axisId.localeCompare(right.axisId),
    ),
  });
}

export function spend(
  resource: ContentCatalogReference<'resource', string>,
  amount: number,
): RpgIrResourceCost {
  return frozenWithCatalogOwnership(
    { resourceId: catalogDefinitionId(resource), amount },
    'resourceId',
    resource,
  );
}

export function activation(options: {
  readonly timing: RpgIrActivationTiming;
  readonly costs?: readonly {
    readonly budget: RulesetActivationBudgetReference<string, string>;
    readonly amount: number;
  }[];
}): RpgIrActivation {
  return frozen({
    timing: options.timing,
    costs: frozenList(
      (options.costs ?? [])
        .map((cost) =>
          frozen({
            budget: cost.budget,
            amount: cost.amount,
          }),
        )
        .sort(
          (left, right) =>
            left.budget.rulesetId.localeCompare(right.budget.rulesetId) ||
            left.budget.id.localeCompare(right.budget.id),
        ),
    ),
  });
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

export interface DamagePartInput {
  readonly id: string;
  readonly amount: RpgIrFormula;
  readonly type: ContentCatalogReference<'damageType', string>;
  readonly tags?: readonly string[];
}

export function damage(options:
  | {
      readonly parts: readonly DamagePartInput[];
      readonly timing?: AuthoringTiming;
    }
  | {
      readonly amount: RpgIrFormula;
      readonly type: ContentCatalogReference<'damageType', string>;
      readonly tags?: readonly string[];
      readonly timing?: AuthoringTiming;
    }): AuthoringProgram {
  const inputs: readonly DamagePartInput[] =
    'parts' in options
      ? options.parts
      : [{
          id: 'damage',
          amount: options.amount,
          type: options.type,
          tags: options.tags ?? [],
        }];
  const parts = inputs
    .map((part) =>
      frozenWithCatalogOwnership(
        {
          id: checkedIdentifier(part.id, 'damage part id'),
          amount: part.amount,
          damageType: catalogDefinitionId(part.type),
          tags: frozenList([...(part.tags ?? [])].sort()),
        },
        'damageType',
        part.type,
      ),
    )
    .sort((left, right) => left.id.localeCompare(right.id));
  return operation(
    frozen({
      kind: 'damage' as const,
      parts: frozenList(parts),
    }),
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
  readonly resource: ContentCatalogReference<'resource', string>;
  readonly delta: RpgIrFormula;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozenWithCatalogOwnership(
      {
        kind: 'changeResource' as const,
        subject: options.subject,
        resourceId: catalogDefinitionId(options.resource),
        delta: options.delta,
      },
      'resourceId',
      options.resource,
    ),
    options.timing,
  );
}

export function applyModifier(options: {
  readonly modifier: ContentCatalogReference<'modifier', string>;
  readonly value: RpgIrFormula;
  readonly duration: AuthoringDuration;
  readonly stacking: AuthoringStacking;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozenWithCatalogOwnership(
      {
        kind: 'applyModifier' as const,
        modifierId: catalogDefinitionId(options.modifier),
        stackingGroup: options.stacking.group,
        stacking: options.stacking.kind,
        value: options.value,
        durationTurns: options.duration.count,
      },
      'modifierId',
      options.modifier,
    ),
    options.timing,
  );
}

export function applyEffect(options: {
  readonly effect: ContentDefinitionReference;
  readonly rank: RpgIrFormula;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  const declaration = {
    kind: 'applyEffect',
    effectDefinitionId: options.effect.definitionId,
    rank: options.rank,
  } as const;
  retainDefinitionOwnership(declaration, 'effectDefinitionId', options.effect);
  return operation(frozen(declaration), options.timing);
}

export function removeEffect(options: {
  readonly effect: ContentDefinitionReference;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  const declaration = {
    kind: 'removeEffect',
    effectDefinitionId: options.effect.definitionId,
  } as const;
  retainDefinitionOwnership(declaration, 'effectDefinitionId', options.effect);
  return operation(frozen(declaration), options.timing);
}

function retainDefinitionOwnership(
  value: object,
  field: string,
  reference: ContentDefinitionReference,
): void {
  Object.defineProperty(value, authoredDefinitionOwnership, {
    value: frozenList([frozen({ field, reference })]),
    enumerable: false,
    configurable: false,
    writable: false,
  });
}

/** @internal Reads typed definition edges retained by operation builders. */
export function definitionOwnershipOf(
  value: object,
): readonly AuthoredDefinitionOwnership[] {
  if (!(authoredDefinitionOwnership in value)) return [];
  const ownership = Reflect.get(value, authoredDefinitionOwnership);
  return Array.isArray(ownership) ? ownership : [];
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

export function moveToCell(options: {
  readonly maximumDistance: number;
  readonly provokes: boolean;
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({
      kind: 'moveToCell',
      maximumDistance: options.maximumDistance,
      provokes: options.provokes,
    }),
    options.timing,
  );
}

export function openReaction(options: {
  readonly id: RpgReactionId;
  readonly options: readonly {
    readonly id: RpgReactionOptionId;
    readonly label: string;
    readonly damageReduction: number;
    readonly activation?: RpgIrActivation;
  }[];
  readonly timing?: AuthoringTiming;
}): AuthoringProgram {
  return operation(
    frozen({
      kind: 'openReaction',
      reactionId: options.id,
      options: frozenList(options.options.map((option) => frozen({ ...option }))),
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

export function onOutcome(branches: OutcomeBranchInput): AuthoringProgram {
  return frozen({
    kind: 'onOutcome',
    branches: frozen(
      Object.fromEntries(
        Object.entries(branches.branches)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([id, branch]) => [id, branch]),
      ),
    ),
    default: branches.default,
  });
}

export function action(input: ActionInput): AuthoredAction {
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

function frozenWithCatalogOwnership<Value extends object>(
  value: Value,
  field: string,
  reference: unknown,
): Readonly<Value> {
  retainCatalogOwnership(value, [{ field, reference }]);
  retainRulesetValueOwnership(value, [{ field, reference }]);
  return frozen(value);
}

function authoredValueId(
  reference: AuthoredStatReference,
): import('@asha-rpg/ir').RpgStatId;
function authoredValueId(
  reference: AuthoredDefenseReference,
): import('@asha-rpg/ir').RpgDefenseId;
function authoredValueId(
  reference: AuthoredStatReference | AuthoredDefenseReference,
): import('@asha-rpg/ir').RpgStatId | import('@asha-rpg/ir').RpgDefenseId {
  return 'definitionId' in reference
    ? catalogDefinitionId(reference)
    : rulesetValueId(reference);
}

function frozenList<Value>(values: readonly Value[]): readonly Value[] {
  return Object.freeze([...values]);
}

function checkedIdentifier(value: string, label: string): string {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value)) {
    throw new Error(`${label} must be a non-empty portable identifier`);
  }
  return value;
}
