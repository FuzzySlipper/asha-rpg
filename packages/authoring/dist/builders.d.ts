import type { RpgActionId, RpgIrComparison, RpgIrFormula, RpgIrPredicate, RpgIrResourceCost, RpgIrSubject, RpgIrTargetSelector, RpgReactionId, RpgReactionOptionId, RpgStackingGroup } from '@asha-rpg/ir';
import type { ActionInput, AuthoredAction, AuthoredActionSource, AuthoredPackage, AuthoringDuration, AuthoringProgram, AuthoringStacking, AuthoringTiming, CheckBranchInput } from './types.js';
import type { RulesetCatalogReference } from './catalogs.js';
export declare function actionId(value: string): RpgActionId;
export declare function stackingGroup(value: string): RpgStackingGroup;
export declare function reactionId(value: string): RpgReactionId;
export declare function reactionOptionId(value: string): RpgReactionOptionId;
export declare function targets(options: {
    readonly team: 'hostile' | 'ally' | 'any';
    readonly maximumRange: number;
    readonly maximumTargets?: number;
}): RpgIrTargetSelector;
export declare function hostile(options: {
    readonly range: number;
    readonly maximum?: number;
}): RpgIrTargetSelector;
export declare function ally(options: {
    readonly range: number;
    readonly maximum?: number;
}): RpgIrTargetSelector;
export declare function constant(value: number): RpgIrFormula;
export declare function readStat(subject: RpgIrSubject, id: RulesetCatalogReference<'stat', string>): RpgIrFormula;
export declare function add(...terms: readonly RpgIrFormula[]): RpgIrFormula;
export declare function dice(options: {
    readonly count: number;
    readonly sides: number;
    readonly bonus?: number;
}): RpgIrFormula;
export declare function half(value: RpgIrFormula): RpgIrFormula;
export declare function always(): RpgIrPredicate;
export declare function compare(left: RpgIrFormula, comparison: RpgIrComparison, right: RpgIrFormula): RpgIrPredicate;
export declare function not(predicate: RpgIrPredicate): RpgIrPredicate;
export declare function all(...predicates: readonly RpgIrPredicate[]): RpgIrPredicate;
export declare function any(...predicates: readonly RpgIrPredicate[]): RpgIrPredicate;
export declare function noRoll(): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'noRoll';
}>;
export declare function attack(options: {
    readonly modifier: RpgIrFormula;
    readonly defense: RulesetCatalogReference<'defense', string>;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'attack';
}>;
export declare function savingThrow(options: {
    readonly difficulty: RpgIrFormula;
    readonly defense: RulesetCatalogReference<'defense', string>;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'savingThrow';
}>;
export declare function spend(resource: RulesetCatalogReference<'resource', string>, amount: number): RpgIrResourceCost;
export declare function immediate(): AuthoringTiming;
export declare function turns(count: number): AuthoringDuration;
export declare function replace(group: RpgStackingGroup): AuthoringStacking;
export declare function refresh(group: RpgStackingGroup): AuthoringStacking;
export declare function damage(options: {
    readonly amount: RpgIrFormula;
    readonly type: RulesetCatalogReference<'damageType', string>;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function heal(options: {
    readonly amount: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function changeResource(options: {
    readonly subject: RpgIrSubject;
    readonly resource: RulesetCatalogReference<'resource', string>;
    readonly delta: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function applyModifier(options: {
    readonly modifier: RulesetCatalogReference<'modifier', string>;
    readonly value: RpgIrFormula;
    readonly duration: AuthoringDuration;
    readonly stacking: AuthoringStacking;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function moveEntity(options: {
    readonly subject: RpgIrSubject;
    readonly deltaX: RpgIrFormula;
    readonly deltaY: RpgIrFormula;
    readonly maximumDistance: number;
    readonly provokes: boolean;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function openReaction(options: {
    readonly id: RpgReactionId;
    readonly options: readonly {
        readonly id: RpgReactionOptionId;
        readonly label: string;
        readonly damageReduction: number;
    }[];
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function sequence(...steps: readonly AuthoringProgram[]): AuthoringProgram;
export declare function when(predicate: RpgIrPredicate, then: AuthoringProgram, otherwise?: AuthoringProgram): AuthoringProgram;
export declare function repeat(count: number, body: AuthoringProgram): AuthoringProgram;
export declare function forEachTarget(maximum: number, body: AuthoringProgram): AuthoringProgram;
export declare function onCheck(branches: CheckBranchInput): AuthoringProgram;
export declare function action(input: ActionInput): AuthoredAction;
export declare function defineActions(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function defineArchetype(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function defineItem(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function defineScenario(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function definePackage(options: {
    readonly id: string;
    readonly version: string;
    readonly sources: readonly AuthoredActionSource[];
}): AuthoredPackage;
//# sourceMappingURL=builders.d.ts.map