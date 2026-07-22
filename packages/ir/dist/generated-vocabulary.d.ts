export declare const RPG_IR_IDENTITY: "asha.rpg.ir";
export declare const RPG_IR_MAJOR: 1;
export declare const RPG_OPERATION_VERSIONS: {
    readonly "operation.applyModifier": 1;
    readonly "operation.changeResource": 1;
    readonly "operation.damage": 1;
    readonly "operation.heal": 1;
    readonly "operation.move": 1;
    readonly "operation.moveToCell": 1;
    readonly "operation.openReaction": 1;
};
export type RpgOperationId = keyof typeof RPG_OPERATION_VERSIONS;
export declare const RPG_CAPABILITY_VERSIONS: {
    readonly "capability.defenses": 1;
    readonly "capability.modifiers": 1;
    readonly "capability.position": 1;
    readonly "capability.random": 1;
    readonly "capability.reactions": 1;
    readonly "capability.resources": 1;
    readonly "capability.stats": 1;
    readonly "capability.vitality": 1;
};
export type RpgCapabilityId = keyof typeof RPG_CAPABILITY_VERSIONS;
//# sourceMappingURL=generated-vocabulary.d.ts.map