import type { RpgDefenseId, RpgStatId } from '@asha-rpg/ir';
import type { Ruleset, RulesetIdentity, RulesetValueContract, RulesetValueExpression, RulesetValueKind, RulesetValueSource } from './play-bundle-types.js';
declare const rulesetValueReferenceBrand: unique symbol;
export interface AuthoredRulesetValueOwnership {
    readonly field: string;
    readonly kind: RulesetValueKind;
    readonly id: string;
    readonly rulesetId: string;
}
type RulesetValueId<Kind extends RulesetValueKind> = Kind extends 'stat' ? RpgStatId : RpgDefenseId;
export type RulesetValueReference<Kind extends RulesetValueKind, RulesetId extends string, ValueId extends string> = Readonly<{
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
export declare function defineRuleset(input: RulesetInput): Ruleset;
export declare function rulesetValueConstant(value: number): RulesetValueExpression;
export declare function readRulesetValue(reference: RulesetValueReference<RulesetValueKind, string, string>): RulesetValueExpression;
export declare function subtractRulesetValues(minuend: RulesetValueExpression, subtrahend: RulesetValueExpression): RulesetValueExpression;
export declare function floorDivideRulesetValues(dividend: RulesetValueExpression, divisor: RulesetValueExpression): RulesetValueExpression;
export declare function derivedRulesetValue(expression: RulesetValueExpression): RulesetValueSource;
export declare function rulesetStat<const RulesetId extends string, const StatId extends string>(ruleset: Ruleset & {
    readonly identity: RulesetIdentity & {
        readonly id: RulesetId;
    };
}, id: StatId): RulesetValueReference<'stat', RulesetId, StatId>;
export declare function rulesetDefense<const RulesetId extends string, const DefenseId extends string>(ruleset: Ruleset & {
    readonly identity: RulesetIdentity & {
        readonly id: RulesetId;
    };
}, id: DefenseId): RulesetValueReference<'defense', RulesetId, DefenseId>;
export declare function rulesetValueId<Kind extends RulesetValueKind>(reference: RulesetValueReference<Kind, string, string>): RulesetValueId<Kind>;
/** @internal Retains Ruleset owner identity on an AST node without serializing it. */
export declare function retainRulesetValueOwnership<Value extends object>(value: Value, fields: readonly {
    readonly field: string;
    readonly reference: unknown;
}[]): Value;
/** @internal Reads Ruleset owner identity retained by typed authoring builders. */
export declare function rulesetValueOwnershipOf(value: object): readonly AuthoredRulesetValueOwnership[];
export {};
//# sourceMappingURL=ruleset-builders.d.ts.map