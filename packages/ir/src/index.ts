export {
  RPG_CAPABILITY_VERSIONS,
  RPG_IR_IDENTITY,
  RPG_IR_MAJOR,
  RPG_OPERATION_VERSIONS,
} from './generated-vocabulary.js';
export type {
  RpgCapabilityId,
  RpgOperationId,
} from './generated-vocabulary.js';

declare const identifierBrand: unique symbol;

export type RpgIdentifier<Kind extends string = string> = string & {
  readonly [identifierBrand]: Kind;
};
export type RpgActionId = RpgIdentifier<'action'>;
export type RpgStatId = RpgIdentifier<'stat'>;
export type RpgDefenseId = RpgIdentifier<'defense'>;
export type RpgResourceId = RpgIdentifier<'resource'>;
export type RpgModifierId = RpgIdentifier<'modifier'>;
export type RpgDamageType = RpgIdentifier<'damageType'>;
export type RpgStackingGroup = RpgIdentifier<'stackingGroup'>;
export type RpgReactionId = RpgIdentifier<'reaction'>;
export type RpgReactionOptionId = RpgIdentifier<'reactionOption'>;

export interface RpgIrSchema {
  readonly identity: 'asha.rpg.ir';
  readonly major: 1;
}

export interface RpgIrPackageIdentity {
  readonly id: string;
  readonly version: string;
}

export interface RpgIrCatalogs {
  readonly stats: readonly RpgStatId[];
  readonly defenses: readonly RpgDefenseId[];
  readonly resources: readonly RpgResourceId[];
  readonly modifiers: readonly RpgModifierId[];
  readonly capabilities: readonly import('./generated-vocabulary.js').RpgCapabilityId[];
}

export type RpgIrRequirement =
  | {
      readonly kind: 'operation';
      readonly id: import('./generated-vocabulary.js').RpgOperationId;
      readonly version: number;
    }
  | {
      readonly kind: 'capability';
      readonly id: import('./generated-vocabulary.js').RpgCapabilityId;
      readonly version: number;
    };

export type RpgIrTeamConstraint = 'hostile' | 'ally' | 'any';
export type RpgIrSubject = 'actor' | 'target';
export type RpgIrRollScope = 'shared' | 'perTarget' | 'none';
export type RpgIrComparison =
  | 'equal'
  | 'notEqual'
  | 'lessThan'
  | 'lessThanOrEqual'
  | 'greaterThan'
  | 'greaterThanOrEqual';
export type RpgIrStackingPolicy = 'replace' | 'refresh';

export interface RpgIrTargetSelector {
  readonly team: RpgIrTeamConstraint;
  readonly maximumRange: number;
  readonly maximumTargets: number;
}

export interface RpgIrResourceCost {
  readonly resourceId: RpgResourceId;
  readonly amount: number;
}

export interface RpgIrReactionOption {
  readonly id: RpgReactionOptionId;
  readonly label: string;
  readonly damageReduction: number;
}

export type RpgIrCheck =
  | { readonly kind: 'noRoll' }
  | {
      readonly kind: 'attack';
      readonly modifier: RpgIrFormula;
      readonly defenseId: RpgDefenseId;
    }
  | {
      readonly kind: 'savingThrow';
      readonly difficulty: RpgIrFormula;
      readonly defenseId: RpgDefenseId;
    };

export type RpgIrFormula =
  | { readonly kind: 'constant'; readonly value: number }
  | {
      readonly kind: 'readStat';
      readonly subject: RpgIrSubject;
      readonly statId: RpgStatId;
    }
  | { readonly kind: 'add'; readonly terms: readonly RpgIrFormula[] }
  | {
      readonly kind: 'dice';
      readonly count: number;
      readonly sides: number;
      readonly bonus: number;
    }
  | { readonly kind: 'half'; readonly value: RpgIrFormula };

export type RpgIrPredicate =
  | { readonly kind: 'always' }
  | {
      readonly kind: 'compare';
      readonly left: RpgIrFormula;
      readonly comparison: RpgIrComparison;
      readonly right: RpgIrFormula;
    }
  | { readonly kind: 'not'; readonly predicate: RpgIrPredicate }
  | { readonly kind: 'all'; readonly predicates: readonly RpgIrPredicate[] }
  | { readonly kind: 'any'; readonly predicates: readonly RpgIrPredicate[] };

export type RpgIrOperation =
  | {
      readonly kind: 'damage';
      readonly amount: RpgIrFormula;
      readonly damageType: RpgDamageType;
    }
  | { readonly kind: 'heal'; readonly amount: RpgIrFormula }
  | {
      readonly kind: 'changeResource';
      readonly subject: RpgIrSubject;
      readonly resourceId: RpgResourceId;
      readonly delta: RpgIrFormula;
    }
  | {
      readonly kind: 'applyModifier';
      readonly modifierId: RpgModifierId;
      readonly stackingGroup: RpgStackingGroup;
      readonly stacking: RpgIrStackingPolicy;
      readonly value: RpgIrFormula;
      readonly durationTurns: number;
    }
  | {
      readonly kind: 'move';
      readonly subject: RpgIrSubject;
      readonly deltaX: RpgIrFormula;
      readonly deltaY: RpgIrFormula;
      readonly maximumDistance: number;
      readonly provokes: boolean;
    }
  | {
      readonly kind: 'openReaction';
      readonly reactionId: RpgReactionId;
      readonly options: readonly RpgIrReactionOption[];
    };

export type RpgIrProgram =
  | { readonly kind: 'operation'; readonly operation: RpgIrOperation }
  | { readonly kind: 'sequence'; readonly steps: readonly RpgIrProgram[] }
  | {
      readonly kind: 'when';
      readonly predicate: RpgIrPredicate;
      readonly then: RpgIrProgram;
      readonly otherwise?: RpgIrProgram;
    }
  | { readonly kind: 'repeat'; readonly count: number; readonly body: RpgIrProgram }
  | {
      readonly kind: 'forEachTarget';
      readonly maximum: number;
      readonly body: RpgIrProgram;
    }
  | {
      readonly kind: 'onCheck';
      readonly hit?: RpgIrProgram;
      readonly miss?: RpgIrProgram;
      readonly saved?: RpgIrProgram;
      readonly failed?: RpgIrProgram;
      readonly noRoll?: RpgIrProgram;
    }
  | { readonly kind: 'atomic'; readonly body: RpgIrProgram };

export interface RpgIrAction {
  readonly id: RpgActionId;
  readonly name: string;
  readonly sourcePath: string;
  readonly targets: RpgIrTargetSelector;
  readonly check: RpgIrCheck;
  readonly rollScope: RpgIrRollScope;
  readonly costs: readonly RpgIrResourceCost[];
  readonly program: RpgIrProgram;
}

export interface NormalizedRpgIr {
  readonly schema: RpgIrSchema;
  readonly package: RpgIrPackageIdentity;
  readonly catalogs: RpgIrCatalogs;
  readonly requirements: readonly RpgIrRequirement[];
  readonly actions: readonly RpgIrAction[];
}
