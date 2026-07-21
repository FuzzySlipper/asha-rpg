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

type RulesetInput = Omit<Ruleset, 'provides'> & {
  readonly provides: Omit<Ruleset['provides'], 'values'> & {
    readonly values: readonly RulesetValueInput[];
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
