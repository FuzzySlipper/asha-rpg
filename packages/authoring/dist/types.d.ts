import type { NormalizedRpgIr, RpgActionId, RpgIrAction, RpgIrCheck, RpgIrFormula, RpgIrOperation, RpgIrPredicate, RpgIrResourceCost, RpgIrRollScope, RpgIrTargetSelector } from '@asha-rpg/ir';
export interface AuthoringTiming {
    readonly kind: 'immediate';
}
export interface AuthoringDuration {
    readonly kind: 'turns';
    readonly count: number;
}
export interface AuthoringStacking {
    readonly kind: 'replace' | 'refresh';
    readonly group: import('@asha-rpg/ir').RpgStackingGroup;
}
export type AuthoringProgram = {
    readonly kind: 'operation';
    readonly operation: RpgIrOperation;
    readonly timing: AuthoringTiming;
} | {
    readonly kind: 'sequence';
    readonly steps: readonly AuthoringProgram[];
} | {
    readonly kind: 'when';
    readonly predicate: RpgIrPredicate;
    readonly then: AuthoringProgram;
    readonly otherwise?: AuthoringProgram;
} | {
    readonly kind: 'repeat';
    readonly count: number;
    readonly body: AuthoringProgram;
} | {
    readonly kind: 'forEachTarget';
    readonly maximum: number;
    readonly body: AuthoringProgram;
} | {
    readonly kind: 'onCheck';
    readonly hit?: AuthoringProgram;
    readonly miss?: AuthoringProgram;
    readonly saved?: AuthoringProgram;
    readonly failed?: AuthoringProgram;
    readonly noRoll?: AuthoringProgram;
};
export interface AuthoredAction {
    readonly id: RpgActionId;
    readonly name: string;
    readonly sourcePath: string;
    readonly targets: RpgIrTargetSelector;
    readonly check: RpgIrCheck;
    readonly rollScope: RpgIrRollScope | undefined;
    readonly costs: readonly RpgIrResourceCost[];
    readonly program: AuthoringProgram;
}
export type AuthoredSourceKind = 'actions' | 'archetype' | 'item';
export interface AuthoredActionSource {
    readonly kind: AuthoredSourceKind;
    readonly id: string;
    readonly actions: readonly AuthoredAction[];
}
export interface AuthoredPackage {
    readonly id: string;
    readonly version: string;
    readonly sources: readonly AuthoredActionSource[];
}
export interface AuthoringDiagnostic {
    readonly stage: 'normalization';
    readonly severity: 'error';
    readonly code: string;
    readonly message: string;
    readonly path: string;
    readonly sourcePath?: string;
}
export type NormalizationResult = {
    readonly ok: true;
    readonly artifact: NormalizedRpgIr;
    readonly diagnostics: readonly [];
} | {
    readonly ok: false;
    readonly diagnostics: readonly AuthoringDiagnostic[];
};
export interface ActionInputBase {
    readonly id: RpgActionId;
    readonly name: string;
    readonly sourcePath: string;
    readonly targets: RpgIrTargetSelector;
    readonly costs?: readonly RpgIrResourceCost[];
    readonly program: AuthoringProgram;
}
export type ActionInput = ActionInputBase & ({
    readonly check: Extract<RpgIrCheck, {
        readonly kind: 'noRoll';
    }>;
    readonly rollScope?: never;
} | {
    readonly check: Exclude<RpgIrCheck, {
        readonly kind: 'noRoll';
    }>;
    readonly rollScope: Exclude<RpgIrRollScope, 'none'>;
});
export type CheckBranchInput = Omit<Extract<AuthoringProgram, {
    readonly kind: 'onCheck';
}>, 'kind'>;
export type { NormalizedRpgIr, RpgIrAction, RpgIrFormula, RpgIrPredicate };
//# sourceMappingURL=types.d.ts.map