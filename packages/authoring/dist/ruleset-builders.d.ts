import type { RpgDefenseId, RpgStatId } from '@asha-rpg/ir';
import type { Ruleset, RulesetIdentity, RulesetValueKind } from './play-bundle-types.js';
declare const rulesetValueReferenceBrand: unique symbol;
type RulesetValueId<Kind extends RulesetValueKind> = Kind extends 'stat' ? RpgStatId : RpgDefenseId;
export type RulesetValueReference<Kind extends RulesetValueKind, RulesetId extends string, ValueId extends string> = Readonly<{
    readonly kind: Kind;
    readonly id: RulesetValueId<Kind> & ValueId;
    readonly rulesetId: RulesetId;
    readonly [rulesetValueReferenceBrand]: true;
}>;
export declare function defineRuleset(input: Ruleset): Ruleset;
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
export {};
//# sourceMappingURL=ruleset-builders.d.ts.map