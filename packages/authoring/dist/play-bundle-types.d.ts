import type { RpgCapabilityId, RpgIrCheck, RpgIrFormula, RpgIrProgram, RpgIrResourceCost, RpgIrTargetSelector, RpgOperationId } from "@asha-rpg/ir";
import type { AuthoredAction } from "./types.js";
import type { ContentCatalogCategory, ContentCatalogReference } from "./catalogs.js";
import type { RulesetValueReference } from "./ruleset-builders.js";
export interface RpgVersionedIdentity {
    readonly id: string;
    readonly version: string;
}
export interface RulesetIdentity extends RpgVersionedIdentity {
}
export interface ContentPackIdentity extends RpgVersionedIdentity {
}
export interface PlayBundleIdentity extends RpgVersionedIdentity {
}
export interface RpgLanguageCompatibility {
    readonly id: "asha-rpg";
    readonly version: string;
}
export type RulesetValueKind = "stat" | "defense";
export interface RulesetValueFormulaSchema {
    readonly identity: "asha.rpg.ruleset-value-formula";
    readonly version: 1;
}
export type RulesetValueExpression = {
    readonly kind: "constant";
    readonly value: number;
} | {
    readonly kind: "readValue";
    readonly rulesetId: string;
    readonly valueKind: RulesetValueKind;
    readonly valueId: string;
} | {
    readonly kind: "subtract";
    readonly minuend: RulesetValueExpression;
    readonly subtrahend: RulesetValueExpression;
} | {
    readonly kind: "floorDivide";
    readonly dividend: RulesetValueExpression;
    readonly divisor: RulesetValueExpression;
};
export type RulesetValueSource = {
    readonly kind: "input";
} | {
    readonly kind: "derived";
    readonly formula: {
        readonly schema: RulesetValueFormulaSchema;
        readonly expression: RulesetValueExpression;
    };
};
export interface RulesetValueContract {
    readonly kind: RulesetValueKind;
    readonly id: string;
    readonly label: string;
    readonly numericDomainId: string;
    readonly source: RulesetValueSource;
}
export interface RulesetNumericDomain {
    readonly id: string;
    readonly minimum: number;
    readonly maximum: number;
}
export interface VersionedRpgRequirement {
    readonly id: string;
    readonly version: number;
}
export interface RulesetProvisions {
    readonly operations: readonly VersionedRpgRequirement[];
    readonly capabilities: readonly VersionedRpgRequirement[];
    readonly values: readonly RulesetValueContract[];
    readonly numericDomains: readonly RulesetNumericDomain[];
}
export interface RulesetModels {
    readonly checks: VersionedRpgRequirement;
    readonly turns: VersionedRpgRequirement;
    readonly initiative: VersionedRpgRequirement;
    readonly reactions: VersionedRpgRequirement;
    readonly actionEconomy: VersionedRpgRequirement;
}
/** Rust-executed semantic vocabulary. This contract never contains content definitions. */
export interface Ruleset {
    readonly schema: {
        readonly identity: "asha.rpg.ruleset";
        readonly major: 1;
    };
    readonly identity: RulesetIdentity;
    readonly language: RpgLanguageCompatibility;
    readonly models: RulesetModels;
    readonly provides: RulesetProvisions;
}
export type ContentRelationshipKind = "dependsOn" | "contributes" | "derivesFrom" | "patches" | "configures" | "exports";
export interface ContentPackRequest {
    readonly id: string;
    readonly version: string;
}
export interface ContentPackDependency extends ContentPackRequest {
    readonly importAs: string;
    readonly relationship: "dependsOn";
}
export interface ContentPackRequirements {
    readonly operations: readonly {
        readonly id: RpgOperationId;
        readonly version: number;
    }[];
    readonly capabilities: readonly {
        readonly id: RpgCapabilityId;
        readonly version: number;
    }[];
    readonly values: readonly {
        readonly kind: RulesetValueKind;
        readonly id: string;
    }[];
    readonly numericDomains: readonly string[];
}
export interface ContentSourceLocation {
    readonly module: string;
    readonly declaration: string;
}
export interface ContentDefinitionReference {
    readonly definitionId: string;
    readonly importAs?: string;
}
export type ContentDefinitionVisibility = "public" | "private";
export type ContentExtensionPolicy = "sealed" | "derivable" | "patchable" | "configurable";
export type ContentPatchScalar = string | number | boolean | null;
export type ContentPatchPathSegment = {
    readonly kind: "field";
    readonly name: string;
} | {
    readonly kind: "member";
    readonly key: "id" | "resourceId" | "statId" | "defenseId" | "modifierId" | "damageType" | "kind";
    readonly value: string;
};
export type ContentPatchNumber = number | {
    readonly parameter: string;
};
export type ContentPatchOperation = {
    readonly kind: "setScalar";
    readonly plane: "semantic" | "presentation";
    readonly path: readonly ContentPatchPathSegment[];
    readonly value: ContentPatchScalar | {
        readonly parameter: string;
    };
} | {
    readonly kind: "upsertScalar";
    readonly plane: "semantic" | "presentation";
    readonly path: readonly ContentPatchPathSegment[];
    readonly value: ContentPatchScalar | {
        readonly parameter: string;
    };
} | {
    readonly kind: "adjustNumber";
    readonly plane: "semantic" | "presentation";
    readonly path: readonly ContentPatchPathSegment[];
    readonly multiply: ContentPatchNumber;
    readonly add: ContentPatchNumber;
} | {
    readonly kind: "appendMember";
    readonly plane: "semantic" | "presentation";
    readonly path: readonly ContentPatchPathSegment[];
    readonly identity: {
        readonly key: Exclude<Extract<ContentPatchPathSegment, {
            readonly kind: "member";
        }>["key"], "kind">;
        readonly value: string;
    };
    readonly value: Readonly<Record<string, ContentPatchScalar>>;
    readonly position: {
        readonly kind: "start";
    } | {
        readonly kind: "end";
    } | {
        readonly kind: "before" | "after";
        readonly anchor: Extract<ContentPatchPathSegment, {
            readonly kind: "member";
        }>;
    };
} | {
    readonly kind: "removeMember";
    readonly plane: "semantic" | "presentation";
    readonly path: readonly ContentPatchPathSegment[];
    readonly identity: Extract<ContentPatchPathSegment, {
        readonly kind: "member";
    }>;
};
export interface ContentPatch {
    readonly version: 1;
    readonly operations: readonly ContentPatchOperation[];
}
export interface ContentMixinParameter {
    readonly id: string;
    readonly type: "string" | "number" | "boolean";
    readonly default?: string | number | boolean;
}
export interface ContentMixinApplication {
    readonly target: ContentDefinitionReference;
    readonly parameters: Readonly<Record<string, string | number | boolean>>;
}
export interface ContentPresentation {
    readonly label: string;
    readonly description?: string;
    readonly tags?: readonly string[];
}
interface ContentDefinitionBase {
    readonly id: string;
    readonly visibility: ContentDefinitionVisibility;
    readonly extensionPolicy: ContentExtensionPolicy;
    readonly source: ContentSourceLocation;
    /**
     * Explicit graph edges for low-level compiler fixtures and non-action data.
     * Ordinary action dependencies are derived from the authored action AST.
     */
    readonly lowLevelReferences?: readonly ContentDefinitionReference[];
    readonly presentation?: ContentPresentation;
}
export interface ContentActionDefinition extends ContentDefinitionBase {
    readonly kind: "action";
    readonly action: AuthoredAction;
}
export type ActionProcedureParameterType = "boundedInteger" | "identifier" | "boolean" | "formula" | "rulesetValueReference" | "catalogReference" | "targeting" | "check" | "costs" | "program" | "semanticBranches";
export type ActionProcedureParameter = {
    readonly id: string;
    readonly type: "boundedInteger";
    readonly minimum: number;
    readonly maximum: number;
} | {
    readonly id: string;
    readonly type: Exclude<ActionProcedureParameterType, "boundedInteger">;
};
export interface ActionProcedureParameterReference<Type extends ActionProcedureParameterType = ActionProcedureParameterType> {
    readonly kind: "parameter";
    readonly parameterId: string;
    readonly parameterType: Type;
}
export interface EquippedItemAttributeReference<Type extends ActionProcedureParameterType = ActionProcedureParameterType> {
    readonly kind: "equippedItemAttribute";
    readonly bindingId: string;
    readonly attributeId: string;
    readonly parameterType: Type;
}
export interface EquippedItemBindingRequirement {
    readonly id: string;
    readonly requiredTags: readonly string[];
    readonly requiredTraits: readonly string[];
    readonly slotIds: readonly string[];
}
type ProcedureReferenceType<Value> = [
    Value
] extends [number] ? "boundedInteger" : [Value] extends [boolean] ? "boolean" : [Value] extends [string] ? "identifier" | "catalogReference" | "rulesetValueReference" : [Value] extends [RpgIrFormula] ? "formula" : [Value] extends [RpgIrTargetSelector] ? "targeting" : [Value] extends [RpgIrCheck] ? "check" : [Value] extends [readonly RpgIrResourceCost[]] ? "costs" : [Value] extends [RpgIrProgram] ? "program" | "semanticBranches" : never;
type ActionProcedureTemplateNode<Value> = ActionProcedureParameterReference<ProcedureReferenceType<Value>> | (Value extends readonly (infer Entry)[] ? readonly ActionProcedureTemplateNode<Entry>[] : Value extends object ? {
    readonly [Key in keyof Value]: ActionProcedureTemplateNode<Value[Key]>;
} : Value);
/** A normalized action body whose leaves may be supplied by typed parameters. */
export interface ActionProcedureTemplate {
    readonly targets: ActionProcedureTemplateNode<RpgIrTargetSelector>;
    readonly check: ActionProcedureTemplateNode<RpgIrCheck>;
    readonly rollScope: ActionProcedureTemplateNode<import("@asha-rpg/ir").RpgIrRollScope>;
    readonly costs: ActionProcedureTemplateNode<readonly RpgIrResourceCost[]>;
    readonly program: ActionProcedureTemplateNode<RpgIrProgram>;
}
export type ActionProcedureArgument = number | string | boolean | RpgIrFormula | RulesetValueReference<RulesetValueKind, string, string> | ContentCatalogReference<ContentCatalogCategory, string> | RpgIrTargetSelector | RpgIrCheck | readonly RpgIrResourceCost[] | RpgIrProgram;
type ActionProcedureArgumentFor<Parameter extends ActionProcedureParameter> = Parameter["type"] extends "boundedInteger" ? number : Parameter["type"] extends "identifier" ? string : Parameter["type"] extends "boolean" ? boolean : Parameter["type"] extends "formula" ? RpgIrFormula : Parameter["type"] extends "rulesetValueReference" ? RulesetValueReference<RulesetValueKind, string, string> : Parameter["type"] extends "catalogReference" ? ContentCatalogReference<ContentCatalogCategory, string> : Parameter["type"] extends "targeting" ? RpgIrTargetSelector : Parameter["type"] extends "check" ? RpgIrCheck : Parameter["type"] extends "costs" ? readonly RpgIrResourceCost[] : RpgIrProgram;
export type ActionProcedureArgumentsFor<Parameters extends readonly ActionProcedureParameter[]> = {
    readonly [Parameter in Parameters[number] as Parameter["id"]]: ActionProcedureArgumentFor<Parameter> | EquippedItemAttributeReference<Parameter["type"]>;
};
export type ActionProcedureCompositionArgumentsFor<Parameters extends readonly ActionProcedureParameter[]> = {
    readonly [Parameter in Parameters[number] as Parameter["id"]]: ActionProcedureArgumentFor<Parameter> | ActionProcedureParameterReference<Parameter["type"]>;
};
export interface ActionProcedureInvocation<Arguments extends Readonly<Record<string, ActionProcedureArgument | EquippedItemAttributeReference>> = Readonly<Record<string, ActionProcedureArgument | EquippedItemAttributeReference>>> {
    readonly procedure: ContentDefinitionReference;
    readonly procedureOwnerPackageId: string;
    readonly arguments: Arguments;
    readonly binding?: EquippedItemBindingRequirement;
}
export type ActionProcedureImplementation = {
    readonly kind: "inline";
    readonly template: ActionProcedureTemplate;
} | {
    readonly kind: "invocation";
    readonly invocation: {
        readonly procedure: ContentDefinitionReference;
        readonly procedureOwnerPackageId: string;
        readonly arguments: Readonly<Record<string, ActionProcedureArgument | ActionProcedureParameterReference>>;
    };
};
export interface ContentActionProcedureDefinition<Parameters extends readonly ActionProcedureParameter[] = readonly ActionProcedureParameter[]> extends ContentDefinitionBase {
    readonly kind: "actionProcedure";
    readonly ownerPackageId: string;
    readonly parameters: Parameters;
    readonly implementation: ActionProcedureImplementation;
}
export interface ContentInvokedActionDefinition extends ContentDefinitionBase {
    readonly kind: "action";
    readonly invocation: ActionProcedureInvocation;
    readonly action?: never;
}
export type ContentConcreteActionDefinition = ContentActionDefinition | ContentInvokedActionDefinition;
export type ItemAttributeType = "boundedInteger" | "identifier" | "dice" | "catalogReference" | "rulesetValueReference";
export type ContentItemAttribute = {
    readonly id: string;
    readonly type: "boundedInteger";
    readonly value: number;
    readonly minimum: number;
    readonly maximum: number;
} | {
    readonly id: string;
    readonly type: "identifier";
    readonly valueId: string;
} | {
    readonly id: string;
    readonly type: "dice";
    readonly count: number;
    readonly sides: number;
    readonly bonus: number;
} | {
    readonly id: string;
    readonly type: "catalogReference";
    readonly value: ContentCatalogReference<ContentCatalogCategory, string>;
} | {
    readonly id: string;
    readonly type: "rulesetValueReference";
    readonly value: RulesetValueReference<RulesetValueKind, string, string>;
};
export interface ContentItemData {
    readonly schema: {
        readonly identity: "asha.rpg.item";
        readonly version: 1;
    };
    readonly tags: readonly string[];
    readonly traits: readonly string[];
    readonly allowedSlots: readonly string[];
    readonly attributes: readonly ContentItemAttribute[];
}
export interface ContentItemDefinition extends ContentDefinitionBase {
    readonly kind: "item";
    readonly item: ContentItemData;
}
export interface ContentSupportDefinition extends ContentDefinitionBase {
    readonly kind: "support";
    readonly semantic: {
        /**
         * Rust-owned action catalogs use the well-known stat, defense, resource,
         * modifier, and damageType names. Consumer repositories may add inert
         * support catalogs for setup and presentation without extending Rust.
         */
        readonly catalog: string;
        readonly id: string;
        readonly data?: unknown;
    };
}
declare const participantProfileCapabilityBrand: unique symbol;
/** A typed base fact accepted by the participant-profile authoring builders. */
export type ContentParticipantProfileCapability = ScenarioInitialCapability & {
    readonly [participantProfileCapabilityBrand]: true;
};
/** Inert, portable defaults that a host may use to construct Scenario participants. */
export interface ContentParticipantProfileData {
    readonly schema: {
        readonly identity: "asha.rpg.participant-profile";
        readonly version: 1;
    };
    readonly role: "player" | "creature";
    readonly definitionReferences: readonly ContentDefinitionReference[];
    readonly items: readonly {
        readonly id: string;
        readonly definition: ContentDefinitionReference;
    }[];
    readonly equipment: readonly {
        readonly slotId: string;
        readonly itemInstanceId: string;
    }[];
    readonly capabilities: readonly ContentParticipantProfileCapability[];
}
/** Rust-validated profile payload retained in a materialized support definition. */
export interface MaterializedParticipantProfileData {
    readonly schema: {
        readonly identity: "asha.rpg.participant-profile";
        readonly version: 1;
    };
    readonly role: "player" | "creature";
    readonly definitionIds: readonly string[];
    readonly items: readonly {
        readonly id: string;
        readonly definitionId: string;
    }[];
    readonly equipment: readonly {
        readonly slotId: string;
        readonly itemInstanceId: string;
    }[];
    readonly capabilities: readonly ScenarioInitialCapability[];
}
export interface ContentTemplateDefinition extends ContentDefinitionBase {
    readonly kind: "template";
}
export interface ContentDerivedDefinition extends ContentDefinitionBase {
    readonly kind: "derived";
    readonly materializesAs: "action" | "support";
}
export interface ContentMixinDefinition extends ContentDefinitionBase {
    readonly kind: "mixin";
    readonly parameters: readonly ContentMixinParameter[];
    readonly patch: ContentPatch;
}
export type ContentDefinition = ContentConcreteActionDefinition | ContentActionProcedureDefinition | ContentItemDefinition | ContentSupportDefinition | ContentTemplateDefinition | ContentDerivedDefinition | ContentMixinDefinition;
export interface ContentPolicyBinding {
    readonly id: string;
    readonly policyId: string;
    readonly policyVersion: string;
    readonly viewKind: string;
    readonly viewVersion: number;
    readonly intentKinds: readonly string[];
    readonly decisionMoments: readonly string[];
    readonly label: string;
}
export type ContentReservedRelationship = {
    readonly kind: "derivesFrom";
    readonly definitionId: string;
    readonly target: ContentDefinitionReference;
    readonly mixins: readonly ContentMixinApplication[];
    readonly localPatch: ContentPatch;
    readonly version: 1;
} | {
    readonly kind: "patches";
    readonly definitionId: string;
    readonly target: ContentDefinitionReference;
    readonly targetPackage: ContentPackIdentity;
    readonly expectedFingerprint: string;
    readonly patch: ContentPatch;
    readonly plane: "semantic" | "presentation" | "both";
    readonly conflictPolicy: "reject" | "replace";
    readonly version: 1;
} | {
    readonly kind: "configures";
    readonly optionId: string;
    readonly target: ContentDefinitionReference;
    readonly value: string | number | boolean;
    readonly patch: ContentPatch;
    readonly version: 1;
};
export interface ContentPackManifest {
    readonly identity: ContentPackIdentity;
    readonly entry: ContentSourceLocation;
    readonly language: RpgLanguageCompatibility;
    readonly dependencies: readonly ContentPackDependency[];
    readonly requirements: ContentPackRequirements;
    readonly definitions: readonly ContentDefinition[];
    readonly exports: readonly string[];
    readonly policyBindings: readonly ContentPolicyBinding[];
    readonly relationships: readonly ContentReservedRelationship[];
}
export interface ContentPackSource {
    readonly manifest: ContentPackManifest;
    readonly sourceFingerprint: string;
}
export interface PlayBundleManifest {
    readonly identity: PlayBundleIdentity;
    readonly ruleset: Ruleset;
    readonly base: ContentPackRequest;
    readonly add: readonly ContentPackRequest[];
    readonly overlays: readonly ContentPackRequest[];
    readonly configure: Readonly<Record<string, string | number | boolean>>;
}
export interface PlayBundleCompilerTarget {
    readonly language: RpgVersionedIdentity;
    readonly operations: Readonly<Record<RpgOperationId, number>>;
    readonly capabilities: Readonly<Record<RpgCapabilityId, number>>;
    readonly models: {
        readonly checks: Readonly<Record<string, number>>;
        readonly turns: Readonly<Record<string, number>>;
        readonly initiative: Readonly<Record<string, number>>;
        readonly reactions: Readonly<Record<string, number>>;
        readonly actionEconomy: Readonly<Record<string, number>>;
    };
}
export type PlayBundleCompilerStage = "source" | "resolution" | "compatibility" | "graph" | "materialization" | "normalization";
export interface PlayBundleCompilerDiagnostic {
    readonly stage: PlayBundleCompilerStage;
    readonly severity: "error";
    readonly code: string;
    readonly path: string;
    readonly message: string;
    readonly packageId?: string;
    readonly definitionId?: string;
    readonly source?: ContentSourceLocation;
    readonly graphPath?: readonly string[];
    readonly expected?: string;
    readonly actual?: string;
}
export interface ResolvedContentPack {
    readonly id: string;
    readonly version: string;
    readonly sourceFingerprint: string;
}
export interface ContentPackDependencyLockEntry {
    readonly requester: string;
    readonly packageId: string;
    readonly requestedVersion: string;
    readonly resolvedVersion: string;
    readonly sourceFingerprint: string;
    readonly importAs: string;
    readonly relationship: "dependsOn" | "contributes" | "patches";
}
export interface ContentRelationshipProvenance {
    readonly kind: ContentRelationshipKind;
    readonly source: string;
    readonly target: string;
    readonly order: number;
}
export interface ContentDefinitionProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly source: ContentSourceLocation;
}
export interface ContentPatchChangeProvenance {
    readonly plane: "semantic" | "presentation";
    readonly path: string;
    readonly pathSegments: readonly ContentPatchPathSegment[];
    readonly before: unknown;
    readonly after: unknown;
    readonly effective: boolean;
}
export interface ContentMaterializationStage {
    readonly id: string;
    readonly kind: "action" | "actionProcedure" | "item" | "support";
    readonly extensionPolicy: ContentExtensionPolicy;
    readonly value: {
        readonly semantic: unknown;
        readonly presentation: ContentPresentation | null;
    };
    readonly references: readonly string[];
}
export type ContentDefinitionCommitment = {
    readonly kind: "concrete";
    readonly packageId: string;
    readonly packageVersion: string;
    readonly packageSourceFingerprint: string;
    readonly definitionId: string;
    readonly fingerprint: string;
    readonly stage: ContentMaterializationStage;
} | {
    readonly kind: "mixin";
    readonly packageId: string;
    readonly packageVersion: string;
    readonly packageSourceFingerprint: string;
    readonly definitionId: string;
    readonly fingerprint: string;
    readonly value: {
        readonly parameters: readonly ContentMixinParameter[];
        readonly patch: ContentPatch;
    };
};
export interface ContentDerivationMixinProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly fingerprint: string;
    readonly patch: ContentPatch;
    readonly parameters: Readonly<Record<string, string | number | boolean>>;
    readonly order: number;
}
export interface ContentDerivationProvenance {
    readonly definitionId: string;
    readonly packageId: string;
    readonly packageVersion: string;
    readonly baseDefinitionId: string;
    readonly basePackageId: string;
    readonly basePackageVersion: string;
    readonly baseFingerprint: string;
    readonly base: ContentMaterializationStage;
    readonly mixins: readonly ContentDerivationMixinProvenance[];
    readonly localPatchFingerprint: string;
    readonly localPatch: ContentPatch;
    readonly materializedFingerprint: string;
    readonly materialized: ContentMaterializationStage;
    readonly changes: readonly ContentPatchChangeProvenance[];
}
export interface ContentOverlayProvenance {
    readonly overlayPackageId: string;
    readonly overlayPackageVersion: string;
    readonly targetDefinitionId: string;
    readonly targetPackageId: string;
    readonly targetPackageVersion: string;
    readonly expectedFingerprint: string;
    readonly beforeFingerprint: string;
    readonly afterFingerprint: string;
    readonly plane: "semantic" | "presentation" | "both";
    readonly conflictPolicy: "reject" | "replace";
    readonly patchFingerprint: string;
    readonly patch: ContentPatch;
    readonly before: ContentMaterializationStage;
    readonly order: number;
    readonly changes: readonly ContentPatchChangeProvenance[];
}
export interface MaterializedContentDefinition {
    readonly id: string;
    readonly kind: "action" | "actionProcedure" | "item" | "support";
    readonly visibility: "exported" | "support";
    readonly extensionPolicy: ContentExtensionPolicy;
    readonly semantic: unknown;
    readonly presentation: ContentPresentation | null;
    readonly references: readonly string[];
    readonly provenance: ContentDefinitionProvenance;
    readonly fingerprint: string;
}
export interface PreparedPlayBundle {
    readonly schema: {
        readonly identity: "asha.rpg.play-bundle.prepared";
        readonly major: 1;
    };
    readonly playBundleIdentity: PlayBundleIdentity;
    readonly ruleset: Ruleset;
    readonly contentPacks: readonly ResolvedContentPack[];
    readonly dependencyLock: readonly ContentPackDependencyLockEntry[];
    readonly contentRequirements: ContentPackRequirements;
    readonly exportedRoots: readonly string[];
    readonly materializedDefinitions: readonly MaterializedContentDefinition[];
    readonly compiledPolicyBindings: readonly ContentPolicyBinding[];
    readonly definitionProvenance: readonly ContentDefinitionProvenance[];
    readonly definitionCommitments: readonly ContentDefinitionCommitment[];
    readonly relationships: readonly ContentRelationshipProvenance[];
    readonly derivationProvenance: readonly ContentDerivationProvenance[];
    readonly overlayProvenance: readonly ContentOverlayProvenance[];
}
export type PreparePlayBundleResult = {
    readonly ok: true;
    readonly prepared: PreparedPlayBundle;
    readonly diagnostics: readonly [];
} | {
    readonly ok: false;
    readonly diagnostics: readonly PlayBundleCompilerDiagnostic[];
};
export interface ScenarioPosition {
    readonly x: number;
    readonly y: number;
}
export interface ScenarioBoundedValue {
    readonly current: number;
    readonly max: number;
}
export type ScenarioInitialCapability = {
    readonly owner: "vitality";
    readonly value: ScenarioBoundedValue;
} | {
    readonly owner: "stat";
    readonly id: string;
    readonly value: number;
} | {
    readonly owner: "defense";
    readonly id: string;
    readonly value: number;
} | {
    readonly owner: "resource";
    readonly id: string;
    readonly value: ScenarioBoundedValue;
} | {
    readonly owner: "modifier";
    readonly stackingGroup: string;
    readonly id: string;
    readonly value: number;
    readonly remainingTurns: number;
};
export type ScenarioCellCapabilityValue = {
    readonly kind: "traversal";
    readonly passable: boolean;
    readonly movementCost: number;
} | {
    readonly kind: "flag";
    readonly value: boolean;
} | {
    readonly kind: "integer";
    readonly value: number;
} | {
    readonly kind: "identifier";
    readonly valueId: string;
};
export interface Scenario {
    readonly schema: {
        readonly id: "asha.rpg.scenario";
        readonly version: 1;
    };
    readonly playBundleId: string;
    readonly board: {
        readonly width: number;
        readonly height: number;
        readonly cells: readonly {
            readonly id: string;
            readonly position: ScenarioPosition;
            readonly capabilities: readonly {
                readonly id: string;
                readonly version: number;
                readonly definitionId?: string;
                readonly value: ScenarioCellCapabilityValue;
            }[];
        }[];
    };
    readonly participants: readonly {
        readonly id: string;
        readonly label: string;
        readonly teamId: string;
        readonly position: ScenarioPosition;
        readonly definitionIds: readonly string[];
        readonly items?: readonly {
            readonly id: string;
            readonly definitionId: string;
        }[];
        readonly equipment?: readonly {
            readonly slotId: string;
            readonly itemInstanceId: string;
        }[];
        readonly capabilities: readonly ScenarioInitialCapability[];
    }[];
    readonly turn: {
        readonly initiativeOrder: readonly string[];
        readonly currentActorId: string;
        readonly round: number;
        readonly turn: number;
    };
    readonly randomSource: {
        readonly policyId: string;
        readonly policyVersion: number;
        readonly sourceId: string;
        readonly sourceVersion: number;
    };
}
/**
 * Immutable, artifact-independent setup data published by a content owner.
 * Hosts bind a template to the exact compiled PlayBundle artifact only when a
 * user chooses to instantiate it.
 */
export interface ScenarioTemplate {
    readonly schema: {
        readonly id: "asha.rpg.scenario-template";
        readonly version: 1;
    };
    readonly identity: {
        readonly id: string;
        readonly version: string;
    };
    readonly playBundle: {
        readonly id: string;
        readonly version: string;
    };
    readonly presentation: {
        readonly label: string;
        readonly description?: string;
    };
    readonly board: Scenario["board"];
    readonly participants: Scenario["participants"];
    readonly turn: Scenario["turn"];
    readonly randomSource: Scenario["randomSource"];
}
export {};
//# sourceMappingURL=play-bundle-types.d.ts.map