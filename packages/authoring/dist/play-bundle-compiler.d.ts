import type { PreparePlayBundleResult, PlayBundleCompilerTarget, ContentDefinition, ContentPackSource } from './play-bundle-types.js';
export declare const ASHA_RPG_PLAY_BUNDLE_TARGET: PlayBundleCompilerTarget;
export declare function preparePlayBundle(options: {
    readonly bundle: import('./play-bundle-types.js').PlayBundleManifest;
    readonly contentPacks: readonly ContentPackSource[];
    readonly target?: PlayBundleCompilerTarget;
}): PreparePlayBundleResult;
export declare function contentDefinitionMaterializationFingerprint(definition: Extract<ContentDefinition, {
    readonly kind: 'action' | 'actionProcedure' | 'item' | 'support';
}>): string;
//# sourceMappingURL=play-bundle-compiler.d.ts.map