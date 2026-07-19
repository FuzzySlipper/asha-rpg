import type { RulesetCatalogInput } from './catalogs.js';
import type { RulesetPatch, RulesetPatchNumber } from './ruleset-types.js';
export interface NumberAdjustment {
    readonly multiply?: RulesetPatchNumber;
    readonly add?: RulesetPatchNumber;
}
export declare function patchParameter(id: string): {
    readonly parameter: string;
};
export declare function combineRulesetPatches(...patches: readonly RulesetPatch[]): RulesetPatch;
export declare const actionPatch: {
    semantic: {
        maximumRange: {
            set(value: number | {
                readonly parameter: string;
            }): RulesetPatch;
            adjust(options: NumberAdjustment): RulesetPatch;
        };
        maximumTargets: {
            set(value: number | {
                readonly parameter: string;
            }): RulesetPatch;
            adjust(options: NumberAdjustment): RulesetPatch;
        };
        cost(resource: RulesetCatalogInput<'resource'>): {
            amount: {
                set(value: number | {
                    readonly parameter: string;
                }): RulesetPatch;
                adjust(options: NumberAdjustment): RulesetPatch;
            };
            remove(): RulesetPatch;
        };
    };
    presentation: {
        label: {
            set(value: string | {
                readonly parameter: string;
            }): RulesetPatch;
        };
        description: {
            set(value: string | {
                readonly parameter: string;
            }): RulesetPatch;
        };
    };
};
//# sourceMappingURL=ruleset-patch-builders.d.ts.map