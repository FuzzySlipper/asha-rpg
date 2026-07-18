import type { NormalizedRpgIr, RpgIrAction } from '@asha-rpg/ir';
import type { AuthoredAction, AuthoredPackage, NormalizationResult } from './types.js';
export declare function normalizePackage(source: AuthoredPackage): NormalizationResult;
export declare function canonicalRpgJson(artifact: NormalizedRpgIr): string;
export declare function normalizeAction(action: AuthoredAction): RpgIrAction;
//# sourceMappingURL=normalize.d.ts.map