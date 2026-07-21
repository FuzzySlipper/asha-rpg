import type { ContentCatalogReference } from './catalogs.js';
import type { ContentPatch, ContentPatchNumber } from './play-bundle-types.js';
export interface NumberAdjustment {
    readonly multiply?: ContentPatchNumber;
    readonly add?: ContentPatchNumber;
}
export declare function patchParameter(id: string): {
    readonly parameter: string;
};
export declare function combineContentPatches(...patches: readonly ContentPatch[]): ContentPatch;
export declare const actionPatch: {
    semantic: {
        maximumRange: {
            set(value: number | {
                readonly parameter: string;
            }): ContentPatch;
            adjust(options: NumberAdjustment): ContentPatch;
        };
        maximumTargets: {
            set(value: number | {
                readonly parameter: string;
            }): ContentPatch;
            adjust(options: NumberAdjustment): ContentPatch;
        };
        cost(resource: ContentCatalogReference<'resource', string>): {
            amount: {
                set(value: number | {
                    readonly parameter: string;
                }): ContentPatch;
                adjust(options: NumberAdjustment): ContentPatch;
            };
            remove(): ContentPatch;
        };
    };
    presentation: {
        label: {
            set(value: string | {
                readonly parameter: string;
            }): ContentPatch;
        };
        description: {
            set(value: string | {
                readonly parameter: string;
            }): ContentPatch;
        };
    };
};
//# sourceMappingURL=content-patch-builders.d.ts.map