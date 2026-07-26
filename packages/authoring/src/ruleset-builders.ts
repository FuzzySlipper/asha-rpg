import type { RpgDefenseId, RpgStatId } from '@asha-rpg/ir';

import { immutable } from './canonical.js';
import type {
  Ruleset,
  RulesetIdentity,
  RulesetValueContract,
  RulesetValueExpression,
  RulesetValueKind,
  RulesetValueSource,
} from './play-bundle-types.js';

const rulesetValueReferenceBrand: unique symbol = Symbol(
  'asha-rpg.ruleset-value-reference',
);
const authoredRulesetValueOwnership: unique symbol = Symbol(
  'asha-rpg.authored-ruleset-value-ownership',
);
const rulesetCalculationSelectorReferenceBrand: unique symbol = Symbol(
  'asha-rpg.calculation-selector-reference',
);
const rulesetContributionStackingGroupReferenceBrand: unique symbol = Symbol(
  'asha-rpg.contribution-stacking-group-reference',
);
const rulesetScalarTestProfileReferenceBrand: unique symbol = Symbol(
  'asha-rpg.scalar-test-profile-reference',
);
const rulesetActivationBudgetReferenceBrand: unique symbol = Symbol(
  'asha-rpg.activation-budget-reference',
);
const rulesetHeterogeneousPoolProfileReferenceBrand: unique symbol = Symbol(
  'asha-rpg.heterogeneous-pool-profile-reference',
);

export interface AuthoredRulesetValueOwnership {
  readonly field: string;
  readonly kind: RulesetValueKind;
  readonly id: string;
  readonly rulesetId: string;
}

type RulesetValueId<Kind extends RulesetValueKind> = Kind extends 'stat'
  ? RpgStatId
  : RpgDefenseId;

export type RulesetValueReference<
  Kind extends RulesetValueKind,
  RulesetId extends string,
  ValueId extends string,
> = Readonly<{
  readonly kind: Kind;
  readonly id: RulesetValueId<Kind> & ValueId;
  readonly rulesetId: RulesetId;
  readonly [rulesetValueReferenceBrand]: true;
}>;

type RulesetValueInput = Omit<RulesetValueContract, 'source'> & {
  readonly source?: RulesetValueSource;
};

export type RulesetCalculationSelectorReference<
  RulesetId extends string,
  SelectorId extends string,
> = Readonly<{
  readonly rulesetId: RulesetId;
  readonly id: SelectorId;
  readonly [rulesetCalculationSelectorReferenceBrand]: true;
}>;

export type RulesetContributionStackingGroupReference<
  RulesetId extends string,
  GroupId extends string,
> = Readonly<{
  readonly rulesetId: RulesetId;
  readonly id: GroupId;
  readonly [rulesetContributionStackingGroupReferenceBrand]: true;
}>;

export type RulesetScalarTestProfileReference<
  RulesetId extends string,
  ProfileId extends string,
> = Readonly<{
  readonly rulesetId: RulesetId;
  readonly id: ProfileId;
  readonly [rulesetScalarTestProfileReferenceBrand]: true;
}>;

export type RulesetActivationBudgetReference<
  RulesetId extends string,
  BudgetId extends string,
> = Readonly<{
  readonly rulesetId: RulesetId;
  readonly id: BudgetId;
  readonly [rulesetActivationBudgetReferenceBrand]: true;
}>;

export type RulesetHeterogeneousPoolProfileReference<
  RulesetId extends string,
  ProfileId extends string,
> = Readonly<{
  readonly rulesetId: RulesetId;
  readonly id: ProfileId;
  readonly [rulesetHeterogeneousPoolProfileReferenceBrand]: true;
}>;

type RulesetInput = Omit<Ruleset, 'provides'> & {
  readonly provides: Omit<
    Ruleset['provides'],
    | 'values'
    | 'calculationSelectors'
    | 'contributionStackingGroups'
    | 'scalarTestProfiles'
    | 'activationBudgets'
    | 'heterogeneousPoolProfiles'
  > & {
    readonly values: readonly RulesetValueInput[];
    readonly calculationSelectors?: Ruleset['provides']['calculationSelectors'];
    readonly contributionStackingGroups?: Ruleset['provides']['contributionStackingGroups'];
    readonly scalarTestProfiles?: Ruleset['provides']['scalarTestProfiles'];
    readonly activationBudgets?: Ruleset['provides']['activationBudgets'];
    readonly movementAllowanceBudgetId?: Ruleset['provides']['movementAllowanceBudgetId'];
    readonly heterogeneousPoolProfiles?: Ruleset['provides']['heterogeneousPoolProfiles'];
  };
};

export function defineRuleset(input: RulesetInput): Ruleset {
  return immutable({
    ...input,
    schema: { identity: 'asha.rpg.ruleset', major: 1 },
    provides: {
      operations: [...input.provides.operations].sort(compareVersionedProvision),
      capabilities: [...input.provides.capabilities].sort(compareVersionedProvision),
      values: input.provides.values
        .map((value) => ({
          ...value,
          source: value.source ?? ({ kind: 'input' } as const),
        }))
        .sort(
          (left, right) =>
            left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id),
        ),
      numericDomains: [...input.provides.numericDomains].sort((left, right) =>
        left.id.localeCompare(right.id),
      ),
      calculationSelectors: [...(input.provides.calculationSelectors ?? [])].sort(
        (left, right) => left.id.localeCompare(right.id),
      ),
      contributionStackingGroups: [
        ...(input.provides.contributionStackingGroups ?? []),
      ].sort((left, right) => left.id.localeCompare(right.id)),
      scalarTestProfiles: [...(input.provides.scalarTestProfiles ?? [])].sort(
        (left, right) => left.id.localeCompare(right.id),
      ),
      activationBudgets: [...(input.provides.activationBudgets ?? [])].sort(
        (left, right) => left.id.localeCompare(right.id),
      ),
      ...(input.provides.movementAllowanceBudgetId === undefined
        ? {}
        : {
            movementAllowanceBudgetId:
              input.provides.movementAllowanceBudgetId,
          }),
      heterogeneousPoolProfiles: [
        ...(input.provides.heterogeneousPoolProfiles ?? []),
      ].sort((left, right) => left.id.localeCompare(right.id)),
    },
  });
}

export function rulesetValueConstant(value: number): RulesetValueExpression {
  return immutable({ kind: 'constant' as const, value });
}

export function readRulesetValue(
  reference: RulesetValueReference<RulesetValueKind, string, string>,
): RulesetValueExpression {
  return immutable({
    kind: 'readValue' as const,
    rulesetId: reference.rulesetId,
    valueKind: reference.kind,
    valueId: reference.id,
  });
}

export function subtractRulesetValues(
  minuend: RulesetValueExpression,
  subtrahend: RulesetValueExpression,
): RulesetValueExpression {
  return immutable({ kind: 'subtract' as const, minuend, subtrahend });
}

export function floorDivideRulesetValues(
  dividend: RulesetValueExpression,
  divisor: RulesetValueExpression,
): RulesetValueExpression {
  return immutable({ kind: 'floorDivide' as const, dividend, divisor });
}

export function derivedRulesetValue(
  expression: RulesetValueExpression,
): RulesetValueSource {
  return immutable({
    kind: 'derived' as const,
    formula: {
      schema: {
        identity: 'asha.rpg.ruleset-value-formula' as const,
        version: 1 as const,
      },
      expression,
    },
  });
}

function compareVersionedProvision(
  left: { readonly id: string; readonly version: number },
  right: { readonly id: string; readonly version: number },
): number {
  return left.id.localeCompare(right.id) || left.version - right.version;
}

export function rulesetStat<
  const RulesetId extends string,
  const StatId extends string,
>(
  ruleset: Ruleset & { readonly identity: RulesetIdentity & { readonly id: RulesetId } },
  id: StatId,
): RulesetValueReference<'stat', RulesetId, StatId> {
  return rulesetValueReference(ruleset, 'stat', id);
}

export function rulesetDefense<
  const RulesetId extends string,
  const DefenseId extends string,
>(
  ruleset: Ruleset & { readonly identity: RulesetIdentity & { readonly id: RulesetId } },
  id: DefenseId,
): RulesetValueReference<'defense', RulesetId, DefenseId> {
  return rulesetValueReference(ruleset, 'defense', id);
}

export function rulesetCalculationSelector<
  const RulesetId extends string,
  const SelectorId extends string,
>(
  ruleset: Ruleset & { readonly identity: RulesetIdentity & { readonly id: RulesetId } },
  id: SelectorId,
): RulesetCalculationSelectorReference<RulesetId, SelectorId> {
  if (
    !ruleset.provides.calculationSelectors.some(
      (candidate) => candidate.id === id,
    )
  ) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide calculation selector ${id}`,
    );
  }
  return immutable({
    rulesetId: ruleset.identity.id,
    id,
    [rulesetCalculationSelectorReferenceBrand]: true as const,
  });
}

export function rulesetContributionStackingGroup<
  const RulesetId extends string,
  const GroupId extends string,
>(
  ruleset: Ruleset & { readonly identity: RulesetIdentity & { readonly id: RulesetId } },
  id: GroupId,
): RulesetContributionStackingGroupReference<RulesetId, GroupId> {
  if (
    !ruleset.provides.contributionStackingGroups.some(
      (candidate) => candidate.id === id,
    )
  ) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide contribution stacking group ${id}`,
    );
  }
  return immutable({
    rulesetId: ruleset.identity.id,
    id,
    [rulesetContributionStackingGroupReferenceBrand]: true as const,
  });
}

export function rulesetScalarTestProfile<
  const RulesetId extends string,
  const ProfileId extends string,
>(
  ruleset: Ruleset & {
    readonly identity: RulesetIdentity & { readonly id: RulesetId };
  },
  id: ProfileId,
): RulesetScalarTestProfileReference<RulesetId, ProfileId> {
  if (
    !ruleset.provides.scalarTestProfiles.some(
      (candidate) => candidate.id === id,
    )
  ) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide scalar test profile ${id}`,
    );
  }
  return immutable({
    rulesetId: ruleset.identity.id,
    id,
    [rulesetScalarTestProfileReferenceBrand]: true as const,
  });
}

export function rulesetActivationBudget<
  const RulesetId extends string,
  const BudgetId extends string,
>(
  ruleset: Ruleset & {
    readonly identity: RulesetIdentity & { readonly id: RulesetId };
  },
  id: BudgetId,
): RulesetActivationBudgetReference<RulesetId, BudgetId> {
  if (
    !ruleset.provides.activationBudgets.some(
      (candidate) => candidate.id === id,
    )
  ) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide activation budget ${id}`,
    );
  }
  return immutable({
    rulesetId: ruleset.identity.id,
    id,
    [rulesetActivationBudgetReferenceBrand]: true as const,
  });
}

export function rulesetHeterogeneousPoolProfile<
  const RulesetId extends string,
  const ProfileId extends string,
>(
  ruleset: Ruleset & {
    readonly identity: RulesetIdentity & { readonly id: RulesetId };
  },
  id: ProfileId,
): RulesetHeterogeneousPoolProfileReference<RulesetId, ProfileId> {
  if (
    !ruleset.provides.heterogeneousPoolProfiles.some(
      (candidate) => candidate.id === id,
    )
  ) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide heterogeneous pool profile ${id}`,
    );
  }
  return immutable({
    rulesetId: ruleset.identity.id,
    id,
    [rulesetHeterogeneousPoolProfileReferenceBrand]: true as const,
  });
}

export function rulesetValueId<Kind extends RulesetValueKind>(
  reference: RulesetValueReference<Kind, string, string>,
): RulesetValueId<Kind> {
  return reference.id;
}

/** @internal Retains Ruleset owner identity on an AST node without serializing it. */
export function retainRulesetValueOwnership<Value extends object>(
  value: Value,
  fields: readonly {
    readonly field: string;
    readonly reference: unknown;
  }[],
): Value {
  const ownership = fields.flatMap(({ field, reference }) =>
    isRulesetValueReference(reference)
      ? [
          immutable({
            field,
            kind: reference.kind,
            id: reference.id,
            rulesetId: reference.rulesetId,
          }),
        ]
      : [],
  );
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
export function rulesetValueOwnershipOf(
  value: object,
): readonly AuthoredRulesetValueOwnership[] {
  if (!(authoredRulesetValueOwnership in value)) return [];
  const ownership = value[authoredRulesetValueOwnership];
  return Array.isArray(ownership) ? ownership : [];
}

function rulesetValueReference<
  const Kind extends RulesetValueKind,
  const RulesetId extends string,
  const ValueId extends string,
>(
  ruleset: Ruleset & { readonly identity: RulesetIdentity & { readonly id: RulesetId } },
  kind: Kind,
  id: ValueId,
): RulesetValueReference<Kind, RulesetId, ValueId> {
  const contract = ruleset.provides.values.find(
    (candidate) => candidate.kind === kind && candidate.id === id,
  );
  if (contract === undefined) {
    throw new Error(
      `ruleset ${ruleset.identity.id}@${ruleset.identity.version} does not provide ${kind} ${id}`,
    );
  }
  return immutable({
    kind,
    id: id as RulesetValueId<Kind> & ValueId,
    rulesetId: ruleset.identity.id,
    [rulesetValueReferenceBrand]: true as const,
  });
}

function isRulesetValueReference(
  value: unknown,
): value is RulesetValueReference<RulesetValueKind, string, string> {
  return (
    value !== null &&
    typeof value === 'object' &&
    rulesetValueReferenceBrand in value
  );
}
