import type { RpgActionId, RpgIrActivation, RpgIrActivationTiming, RpgIrComparison, RpgIrFormula, RpgIrPoolAxisValue, RpgIrPoolDieTerm, RpgIrPredicate, RpgIrResourceCost, RpgIrScalarExpression, RpgIrSubject, RpgIrTargetSelector, RpgReactionId, RpgReactionOptionId, RpgStackingGroup } from '@asha-rpg/ir';
import type { ActionInput, AuthoredAction, AuthoredActionSource, AuthoredPackage, AuthoringDuration, AuthoringProgram, AuthoringStacking, AuthoringTiming, CheckBranchInput, OutcomeBranchInput } from './types.js';
import type { ContentCatalogReference } from './catalogs.js';
import type { ContentDefinitionReference } from './play-bundle-types.js';
export interface AuthoredDefinitionOwnership {
    readonly field: string;
    readonly reference: ContentDefinitionReference;
}
import type { RulesetCalculationSelectorReference, RulesetActivationBudgetReference, RulesetHeterogeneousPoolProfileReference, RulesetScalarTestProfileReference, RulesetValueReference } from './ruleset-builders.js';
type AuthoredStatReference = ContentCatalogReference<'stat', string> | RulesetValueReference<'stat', string, string>;
type AuthoredDefenseReference = ContentCatalogReference<'defense', string> | RulesetValueReference<'defense', string, string>;
export declare function actionId(value: string): RpgActionId;
export declare function stackingGroup(value: string): RpgStackingGroup;
export declare function reactionId(value: string): RpgReactionId;
export declare function reactionOptionId(value: string): RpgReactionOptionId;
export declare function targets(options: {
    readonly team: 'hostile' | 'ally' | 'any';
    readonly maximumRange: number;
    readonly maximumTargets?: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function cells(options: {
    readonly range: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function diamondArea(options: {
    readonly range: number;
    readonly radius: number;
    readonly team: 'hostile' | 'ally' | 'any';
    readonly livingRequired?: boolean;
    readonly minimumTargets?: number;
    readonly maximumTargets?: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function orthogonalLineArea(options: {
    readonly range: number;
    readonly length: number;
    readonly team: 'hostile' | 'ally' | 'any';
    readonly livingRequired?: boolean;
    readonly minimumTargets?: number;
    readonly maximumTargets?: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function hostile(options: {
    readonly range: number;
    readonly maximum?: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function ally(options: {
    readonly range: number;
    readonly maximum?: number;
    readonly lineOfEffect?: 'ignored' | 'required';
}): RpgIrTargetSelector;
export declare function constant(value: number): RpgIrScalarExpression;
export declare function readStat(subject: RpgIrSubject, id: AuthoredStatReference): RpgIrScalarExpression;
export declare function add(...terms: readonly RpgIrScalarExpression[]): RpgIrScalarExpression;
export declare function add(...terms: readonly RpgIrFormula[]): RpgIrFormula;
export declare function dice(options: {
    readonly count: number;
    readonly sides: number;
    readonly bonus?: number;
}): RpgIrFormula;
export declare function half(value: RpgIrScalarExpression): RpgIrScalarExpression;
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
    readonly defense: AuthoredDefenseReference;
    readonly contributionSelector?: RulesetCalculationSelectorReference<string, string>;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'attack';
}>;
export declare function savingThrow(options: {
    readonly difficulty: RpgIrFormula;
    readonly defense: AuthoredDefenseReference;
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'savingThrow';
}>;
export declare function scalarTest(options: {
    readonly profile: RulesetScalarTestProfileReference<string, string>;
    readonly base: RpgIrScalarExpression;
    readonly difficulty: {
        readonly kind: 'explicit';
        readonly value: RpgIrScalarExpression;
    } | {
        readonly kind: 'targetDefense';
        readonly defense: AuthoredDefenseReference;
    };
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'scalarTest';
}>;
export declare function heterogeneousPool(options: {
    readonly profile: RulesetHeterogeneousPoolProfileReference<string, string>;
    readonly baseDice: readonly RpgIrPoolDieTerm[];
    readonly automaticAxes?: readonly RpgIrPoolAxisValue[];
}): Extract<import('@asha-rpg/ir').RpgIrCheck, {
    kind: 'heterogeneousPool';
}>;
export declare function spend(resource: ContentCatalogReference<'resource', string>, amount: number): RpgIrResourceCost;
export declare function activation(options: {
    readonly timing: RpgIrActivationTiming;
    readonly costs?: readonly {
        readonly budget: RulesetActivationBudgetReference<string, string>;
        readonly amount: number;
    }[];
}): RpgIrActivation;
export declare function immediate(): AuthoringTiming;
export declare function turns(count: number): AuthoringDuration;
export declare function replace(group: RpgStackingGroup): AuthoringStacking;
export declare function refresh(group: RpgStackingGroup): AuthoringStacking;
export interface DamagePartInput {
    readonly id: string;
    readonly amount: RpgIrFormula;
    readonly type: ContentCatalogReference<'damageType', string>;
    readonly tags?: readonly string[];
}
export declare function damage(options: {
    readonly parts: readonly DamagePartInput[];
    readonly timing?: AuthoringTiming;
} | {
    readonly amount: RpgIrFormula;
    readonly type: ContentCatalogReference<'damageType', string>;
    readonly tags?: readonly string[];
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function heal(options: {
    readonly amount: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function changeResource(options: {
    readonly subject: RpgIrSubject;
    readonly resource: ContentCatalogReference<'resource', string>;
    readonly delta: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function applyModifier(options: {
    readonly modifier: ContentCatalogReference<'modifier', string>;
    readonly value: RpgIrFormula;
    readonly duration: AuthoringDuration;
    readonly stacking: AuthoringStacking;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function applyEffect(options: {
    readonly effect: ContentDefinitionReference;
    readonly rank: RpgIrFormula;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function removeEffect(options: {
    readonly effect: ContentDefinitionReference;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
/** @internal Reads typed definition edges retained by operation builders. */
export declare function definitionOwnershipOf(value: object): readonly AuthoredDefinitionOwnership[];
export declare function moveEntity(options: {
    readonly subject: RpgIrSubject;
    readonly deltaX: RpgIrFormula;
    readonly deltaY: RpgIrFormula;
    readonly maximumDistance: number;
    readonly provokes: boolean;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function moveToCell(options: {
    readonly maximumDistance: number;
    readonly provokes: boolean;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function pushEntity(options: {
    readonly subject: RpgIrSubject;
    readonly distance: number;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function slideEntity(options: {
    readonly subject: RpgIrSubject;
    readonly maximumDistance: number;
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function openReaction(options: {
    readonly id: RpgReactionId;
    readonly options: readonly {
        readonly id: RpgReactionOptionId;
        readonly label: string;
        readonly damageReduction: number;
        readonly activation?: RpgIrActivation;
    }[];
    readonly timing?: AuthoringTiming;
}): AuthoringProgram;
export declare function sequence(...steps: readonly AuthoringProgram[]): AuthoringProgram;
export declare function when(predicate: RpgIrPredicate, then: AuthoringProgram, otherwise?: AuthoringProgram): AuthoringProgram;
export declare function repeat(count: number, body: AuthoringProgram): AuthoringProgram;
export declare function forEachTarget(maximum: number, body: AuthoringProgram): AuthoringProgram;
export declare function onCheck(branches: CheckBranchInput): AuthoringProgram;
export declare function onOutcome(branches: OutcomeBranchInput): AuthoringProgram;
export declare function action(input: ActionInput): AuthoredAction;
export declare function defineActions(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function defineArchetype(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function defineItem(id: string, actions: readonly AuthoredAction[]): AuthoredActionSource;
export declare function definePackage(options: {
    readonly id: string;
    readonly version: string;
    readonly sources: readonly AuthoredActionSource[];
}): AuthoredPackage;
export {};
//# sourceMappingURL=builders.d.ts.map