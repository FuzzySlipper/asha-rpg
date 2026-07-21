import type { RpgDefenseId, RpgStatId } from '@asha-rpg/ir';

import { immutable } from './canonical.js';
import type {
  Ruleset,
  RulesetIdentity,
  RulesetValueKind,
} from './play-bundle-types.js';

const rulesetValueReferenceBrand: unique symbol = Symbol(
  'asha-rpg.ruleset-value-reference',
);

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

export function defineRuleset(input: Ruleset): Ruleset {
  return immutable({
    ...input,
    schema: { identity: 'asha.rpg.ruleset', major: 1 },
    provides: {
      operations: [...input.provides.operations].sort(compareVersionedProvision),
      capabilities: [...input.provides.capabilities].sort(compareVersionedProvision),
      values: [...input.provides.values].sort(
        (left, right) =>
          left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id),
      ),
      numericDomains: [...input.provides.numericDomains].sort((left, right) =>
        left.id.localeCompare(right.id),
      ),
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
