import type { RpgActionId, RpgDamageType, RpgDefenseId, RpgIrComparison, RpgIrFormula, RpgIrPredicate, RpgIrResourceCost, RpgIrSubject, RpgIrTargetSelector, RpgModifierId, RpgResourceId, RpgStackingGroup, RpgStatId } from '@asha-rpg/ir';
import type { ActionInput, AuthoredAction, AuthoredActionSource, AuthoredPackage, AuthoringDuration, AuthoringProgram, AuthoringStacking, AuthoringTiming, CheckBranchInput } from './types.js';
export declare function actionId(value: string): RpgActionId;
export declare function statId(value: string): RpgStatId;
export declare function defenseId(value: string): RpgDefenseId;
export declare function resourceId(value: string): RpgResourceId;
export declare function modifierId(value: string): RpgModifierId;
export declare function damageType(value: string): RpgDamageType;
export declare function stackingGroup(value: string): RpgStackingGroup;
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
export declare function readStat(subject: RpgIrSubject, id: RpgStatId): RpgIrFormula;
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
    readonly defense: RpgDefenseId;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'attack';
}>;
export declare function savingThrow(options: {
    readonly difficulty: RpgIrFormula;
    readonly defense: RpgDefenseId;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'savingThrow';
}>;
export declare function spend(resource: RpgResourceId, amount: number): RpgIrResourceCost;
export declare function immediate(): AuthoringTiming;
export declare function turns(count: number): AuthoringDuration;
export declare function replace(group: RpgStackingGroup): AuthoringStacking;
export declare function refresh(group: RpgStackingGroup): AuthoringStacking;
export declare function damage(options: {
    readonly amount: RpgIrFormula;
    readonly type: RpgDamageType;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function heal(options: {
    readonly amount: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function changeResource(options: {
    readonly subject: RpgIrSubject;
    readonly resource: RpgResourceId;
    readonly delta: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function applyModifier(options: {
    readonly modifier: RpgModifierId;
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