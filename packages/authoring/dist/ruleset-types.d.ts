import type { RpgCapabilityId, RpgOperationId } from '@asha-rpg/ir';
import type { AuthoredAction } from './types.js';
export interface RulesetIdentity {
    readonly id: string;
    readonly version: string;
}
export interface RulesetLanguageCompatibility {
    readonly id: 'asha-rpg';
    readonly version: string;
}
export type RulesetRelationshipKind = 'dependsOn' | 'contributes' | 'derivesFrom' | 'patches' | 'configures' | 'exports';
export interface RulesetPackageRequest {
    readonly id: string;
    readonly version: string;
}
export interface RulesetDependency extends RulesetPackageRequest {
    readonly importAs: string;
    readonly relationship: 'dependsOn';
}
export interface RulesetRequirements {
    readonly operations: readonly {
        readonly id: RpgOperationId;
        readonly version: number;
    }[];
    readonly capabilities: readonly {
        readonly id: RpgCapabilityId;
        readonly version: number;
    }[];
}
export interface RulesetSourceLocation {
    readonly module: string;
    readonly declaration: string;
}
export interface RulesetDefinitionReference {
    readonly definitionId: string;
    readonly importAs?: string;
}
export type RulesetDefinitionVisibility = 'public' | 'private';
export type RulesetExtensionPolicy = 'sealed' | 'derivable' | 'patchable' | 'configurable';
export type RulesetPatchScalar = string | number | boolean | null;
export type RulesetPatchPathSegment = {
    readonly kind: 'field';
    readonly name: string;
} | {
    readonly kind: 'member';
    readonly key: 'id' | 'resourceId' | 'statId' | 'defenseId' | 'modifierId' | 'damageType' | 'kind';
    readonly value: string;
};
export type RulesetPatchNumber = number | {
    readonly parameter: string;
};
export type RulesetPatchOperation = {
    readonly kind: 'setScalar';
    readonly plane: 'semantic' | 'presentation';
    readonly path: readonly RulesetPatchPathSegment[];
    readonly value: RulesetPatchScalar | {
        readonly parameter: string;
    };
} | {
    readonly kind: 'upsertScalar';
    readonly plane: 'semantic' | 'presentation';
    readonly path: readonly RulesetPatchPathSegment[];
    readonly value: RulesetPatchScalar | {
        readonly parameter: string;
    };
} | {
    readonly kind: 'adjustNumber';
    readonly plane: 'semantic' | 'presentation';
    readonly path: readonly RulesetPatchPathSegment[];
    readonly multiply: RulesetPatchNumber;
    readonly add: RulesetPatchNumber;
} | {
    readonly kind: 'appendMember';
    readonly plane: 'semantic' | 'presentation';
    readonly path: readonly RulesetPatchPathSegment[];
    readonly identity: {
        readonly key: Exclude<Extract<RulesetPatchPathSegment, {
            readonly kind: 'member';
        }>['key'], 'kind'>;
        readonly value: string;
    };
    readonly value: Readonly<Record<string, RulesetPatchScalar>>;
    readonly position: {
        readonly kind: 'start';
    } | {
        readonly kind: 'end';
    } | {
        readonly kind: 'before' | 'after';
        readonly anchor: Extract<RulesetPatchPathSegment, {
            readonly kind: 'member';
        }>;
    };
} | {
    readonly kind: 'removeMember';
    readonly plane: 'semantic' | 'presentation';
    readonly path: readonly RulesetPatchPathSegment[];
    readonly identity: Extract<RulesetPatchPathSegment, {
        readonly kind: 'member';
    }>;
};
export interface RulesetPatch {
    readonly version: 1;
    readonly operations: readonly RulesetPatchOperation[];
}
export interface RulesetMixinParameter {
    readonly id: string;
    readonly type: 'string' | 'number' | 'boolean';
    readonly default?: string | number | boolean;
}
export interface RulesetMixinApplication {
    readonly target: RulesetDefinitionReference;
    readonly parameters: Readonly<Record<string, string | number | boolean>>;
}
export interface RulesetPresentation {
    readonly label: string;
    readonly description?: string;
    readonly tags?: readonly string[];
}
interface RulesetDefinitionBase {
    readonly id: string;
    readonly visibility: RulesetDefinitionVisibility;
    readonly extensionPolicy: RulesetExtensionPolicy;
    readonly source: RulesetSourceLocation;
    /**
     * Explicit graph edges for low-level compiler fixtures and non-action data.
     * Ordinary action dependencies are derived from the authored action AST.
     */
    readonly lowLevelReferences?: readonly RulesetDefinitionReference[];
    readonly presentation?: RulesetPresentation;
}
export interface RulesetActionDefinition extends RulesetDefinitionBase {
    readonly kind: 'action';
    readonly action: AuthoredAction;
}
export interface RulesetSupportDefinition extends RulesetDefinitionBase {
    readonly kind: 'support';
    readonly semantic: {
        readonly catalog: 'stat' | 'defense' | 'resource' | 'modifier' | 'damageType';
        readonly id: string;
    };
}
export interface RulesetTemplateDefinition extends RulesetDefinitionBase {
    readonly kind: 'template';
}
export interface RulesetDerivedDefinition extends RulesetDefinitionBase {
    readonly kind: 'derived';
    readonly materializesAs: 'action' | 'support';
}
export interface RulesetMixinDefinition extends RulesetDefinitionBase {
    readonly kind: 'mixin';
    readonly parameters: readonly RulesetMixinParameter[];
    readonly patch: RulesetPatch;
}
export type RulesetDefinition = RulesetActionDefinition | RulesetSupportDefinition | RulesetTemplateDefinition | RulesetDerivedDefinition | RulesetMixinDefinition;
export interface RulesetPolicyBinding {
    readonly id: string;
    readonly policyId: string;
    readonly policyVersion: string;
    readonly viewKind: string;
    readonly viewVersion: number;
    readonly intentKinds: readonly string[];
    readonly decisionMoments: readonly string[];
    readonly label: string;
}
export type RulesetReservedRelationship = {
    readonly kind: 'derivesFrom';
    readonly definitionId: string;
    readonly target: RulesetDefinitionReference;
    readonly mixins: readonly RulesetMixinApplication[];
    readonly localPatch: RulesetPatch;
    readonly version: 1;
} | {
    readonly kind: 'patches';
    readonly definitionId: string;
    readonly target: RulesetDefinitionReference;
    readonly targetPackage: RulesetIdentity;
    readonly expectedFingerprint: string;
    readonly patch: RulesetPatch;
    readonly plane: 'semantic' | 'presentation' | 'both';
    readonly conflictPolicy: 'reject' | 'replace';
    readonly version: 1;
} | {
    readonly kind: 'configures';
    readonly optionId: string;
    readonly target: RulesetDefinitionReference;
    readonly value: string | number | boolean;
    readonly patch: RulesetPatch;
    readonly version: 1;
};
export interface RulesetPackageManifest {
    readonly identity: RulesetIdentity;
    readonly entry: RulesetSourceLocation;
    readonly language: RulesetLanguageCompatibility;
    readonly dependencies: readonly RulesetDependency[];
    readonly requirements: RulesetRequirements;
    readonly definitions: readonly RulesetDefinition[];
    readonly exports: readonly string[];
    readonly policyBindings: readonly RulesetPolicyBinding[];
    readonly relationships: readonly RulesetReservedRelationship[];
}
export interface RulesetPackageSource {
    readonly manifest: RulesetPackageManifest;
    readonly sourceFingerprint: string;
}
export interface RulesetCompositionManifest {
    readonly identity: RulesetIdentity;
    readonly language: RulesetLanguageCompatibility;
    readonly base: RulesetPackageRequest;
    readonly add: readonly RulesetPackageRequest[];
    readonly overlays: readonly RulesetPackageRequest[];
    readonly configure: Readonly<Record<string, string | number | boolean>>;
}
export interface RulesetCompilerTarget {
    readonly language: RulesetIdentity;
    readonly operations: Readonly<Record<RpgOperationId, number>>;
    readonly capabilities: Readonly<Record<RpgCapabilityId, number>>;
}
export type RulesetCompilerStage = 'source' | 'resolution' | 'compatibility' | 'graph' | 'materialization' | 'normalization';
export interface RulesetCompilerDiagnostic {
    readonly stage: RulesetCompilerStage;
    readonly severity: 'error';
    readonly code: string;
    readonly path: string;
    readonly message: string;
    readonly packageId?: string;
    readonly definitionId?: string;
    readonly source?: RulesetSourceLocation;
    readonly graphPath?: readonly string[];
    readonly expected?: string;
    readonly actual?: string;
}
export interface ResolvedRulesetSourcePackage {
    readonly id: string;
    readonly version: string;
    readonly sourceFingerprint: string;
}
export interface RulesetDependencyLockEntry {
    readonly requester: string;
    readonly packageId: string;
    readonly requestedVersion: string;
    readonly resolvedVersion: string;
    readonly sourceFingerprint: string;
    readonly importAs: string;
    readonly relationship: 'dependsOn' | 'contributes' | 'patches';
}
export interface RulesetRelationshipProvenance {
    readonly kind: RulesetRelationshipKind;
    readonly source: string;
    readonly target: string;
    readonly order: number;
}
export interface RulesetDefinitionProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly source: RulesetSourceLocation;
}
export interface RulesetPatchChangeProvenance {
    readonly plane: 'semantic' | 'presentation';
    readonly path: string;
    readonly pathSegments: readonly RulesetPatchPathSegment[];
    readonly before: unknown;
    readonly after: unknown;
    readonly effective: boolean;
}
export interface RulesetMaterializationStage {
    readonly id: string;
    readonly kind: 'action' | 'support';
    readonly extensionPolicy: RulesetExtensionPolicy;
    readonly value: {
        readonly semantic: unknown;
        readonly presentation: RulesetPresentation | null;
    };
    readonly references: readonly string[];
}
export type RulesetDefinitionCommitment = {
    readonly kind: 'concrete';
    readonly packageId: string;
    readonly packageVersion: string;
    readonly packageSourceFingerprint: string;
    readonly definitionId: string;
    readonly fingerprint: string;
    readonly stage: RulesetMaterializationStage;
} | {
    readonly kind: 'mixin';
    readonly packageId: string;
    readonly packageVersion: string;
    readonly packageSourceFingerprint: string;
    readonly definitionId: string;
    readonly fingerprint: string;
    readonly value: {
        readonly parameters: readonly RulesetMixinParameter[];
        readonly patch: RulesetPatch;
    };
};
export interface RulesetDerivationMixinProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly fingerprint: string;
    readonly patch: RulesetPatch;
    readonly parameters: Readonly<Record<string, string | number | boolean>>;
    readonly order: number;
}
export interface RulesetDerivationProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly baseDefinitionId: string;
    readonly basePackageId: string;
    readonly basePackageVersion: string;
    readonly baseFingerprint: string;
    readonly base: RulesetMaterializationStage;
    readonly mixins: readonly RulesetDerivationMixinProvenance[];
    readonly localPatchFingerprint: string;
    readonly localPatch: RulesetPatch;
    readonly materializedFingerprint: string;
    readonly materialized: RulesetMaterializationStage;
    readonly changes: readonly RulesetPatchChangeProvenance[];
}
export interface RulesetOverlayProvenance {
    readonly overlayPackageId: string;
    readonly overlayPackageVersion: string;
    readonly targetDefinitionId: string;
    readonly targetPackageId: string;
    readonly targetPackageVersion: string;
    readonly expectedFingerprint: string;
    readonly beforeFingerprint: string;
    readonly afterFingerprint: string;
    readonly plane: 'semantic' | 'presentation' | 'both';
    readonly conflictPolicy: 'reject' | 'replace';
    readonly patchFingerprint: string;
    readonly patch: RulesetPatch;
    readonly before: RulesetMaterializationStage;
    readonly order: number;
    readonly changes: readonly RulesetPatchChangeProvenance[];
}
export interface MaterializedRulesetDefinition {
    readonly id: string;
    readonly kind: 'action' | 'support';
    readonly visibility: 'exported' | 'support';
    readonly extensionPolicy: RulesetExtensionPolicy;
    readonly semantic: unknown;
    readonly presentation: RulesetPresentation | null;
    readonly references: readonly string[];
    readonly provenance: RulesetDefinitionProvenance;
    readonly fingerprint: string;
}
export interface PreparedRulesetCompilation {
    readonly schema: {
        readonly identity: 'asha.rpg.ruleset.prepared';
        readonly major: 1;
    };
    readonly compositionIdentity: RulesetIdentity;
    readonly languageIdentity: RulesetIdentity;
    readonly sourcePackages: readonly ResolvedRulesetSourcePackage[];
    readonly dependencyLock: readonly RulesetDependencyLockEntry[];
    readonly requiredOperations: readonly {
        readonly id: RpgOperationId;
        readonly version: number;
    }[];
    readonly requiredCapabilities: readonly {
        readonly id: RpgCapabilityId;
        readonly version: number;
    }[];
    readonly exportedRoots: readonly string[];
    readonly materializedDefinitions: readonly MaterializedRulesetDefinition[];
    readonly compiledPolicyBindings: readonly RulesetPolicyBinding[];
    readonly definitionProvenance: readonly RulesetDefinitionProvenance[];
    readonly definitionCommitments: readonly RulesetDefinitionCommitment[];
    readonly relationships: readonly RulesetRelationshipProvenance[];
    readonly derivationProvenance: readonly RulesetDerivationProvenance[];
    readonly overlayProvenance: readonly RulesetOverlayProvenance[];
}
export type PrepareRulesetResult = {
    readonly ok: true;
    readonly prepared: PreparedRulesetCompilation;
    readonly diagnostics: readonly [];
} | {
    readonly ok: false;
    readonly diagnostics: readonly RulesetCompilerDiagnostic[];
};
export {};
//# sourceMappingURL=ruleset-types.d.ts.map