import type {
  NormalizedRpgIr,
  RpgCapabilityId,
  RpgOperationId,
} from '@asha-rpg/ir';

import type { AuthoredAction } from './types.js';

export interface RulesetIdentity {
  readonly id: string;
  readonly version: string;
}

export interface RulesetLanguageCompatibility {
  readonly id: 'asha-rpg';
  readonly version: string;
}

export type RulesetRelationshipKind =
  | 'dependsOn'
  | 'contributes'
  | 'derivesFrom'
  | 'patches'
  | 'configures'
  | 'exports';

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
  readonly references: readonly RulesetDefinitionReference[];
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

export type RulesetDefinition =
  | RulesetActionDefinition
  | RulesetSupportDefinition
  | RulesetTemplateDefinition;

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

export type RulesetReservedRelationship =
  | {
      readonly kind: 'derivesFrom';
      readonly definitionId: string;
      readonly target: RulesetDefinitionReference;
      readonly version: 1;
    }
  | {
      readonly kind: 'patches';
      readonly definitionId: string;
      readonly target: RulesetDefinitionReference;
      readonly expectedFingerprint: string;
      readonly plane: 'semantic' | 'presentation' | 'both';
      readonly conflictPolicy: 'reject';
      readonly version: 1;
    }
  | {
      readonly kind: 'configures';
      readonly optionId: string;
      readonly target: RulesetDefinitionReference;
      readonly value: string | number | boolean;
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

export type RulesetCompilerStage =
  | 'source'
  | 'resolution'
  | 'compatibility'
  | 'graph'
  | 'materialization'
  | 'normalization';

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

export interface MaterializedRulesetDefinition {
  readonly id: string;
  readonly kind: 'action' | 'support';
  readonly visibility: 'exported' | 'support';
  readonly extensionPolicy: RulesetExtensionPolicy;
  readonly semantic: unknown;
  readonly presentation: RulesetPresentation | null;
  readonly references: readonly string[];
  readonly provenance: RulesetDefinitionProvenance;
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
  readonly requiredOperations: readonly { readonly id: RpgOperationId; readonly version: number }[];
  readonly requiredCapabilities: readonly { readonly id: RpgCapabilityId; readonly version: number }[];
  readonly exportedRoots: readonly string[];
  readonly materializedDefinitions: readonly MaterializedRulesetDefinition[];
  readonly compiledPolicyBindings: readonly RulesetPolicyBinding[];
  readonly definitionProvenance: readonly RulesetDefinitionProvenance[];
  readonly relationships: readonly RulesetRelationshipProvenance[];
  readonly derivationProvenance: readonly [];
  readonly overlayProvenance: readonly [];
  readonly normalizedIr: NormalizedRpgIr;
}

export type PrepareRulesetResult =
  | {
      readonly ok: true;
      readonly prepared: PreparedRulesetCompilation;
      readonly diagnostics: readonly [];
    }
  | {
      readonly ok: false;
      readonly diagnostics: readonly RulesetCompilerDiagnostic[];
    };
