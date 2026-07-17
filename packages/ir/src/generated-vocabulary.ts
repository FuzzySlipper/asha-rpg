// Generated from the Rust semantic registry. Do not edit by hand.
export const RPG_IR_IDENTITY = "asha.rpg.ir" as const;
export const RPG_IR_MAJOR = 1 as const;

export const RPG_OPERATION_VERSIONS = {
  "operation.applyModifier": 1,
  "operation.changeResource": 1,
  "operation.damage": 1,
  "operation.heal": 1,
  "operation.move": 1,
} as const;
export type RpgOperationId = keyof typeof RPG_OPERATION_VERSIONS;

export const RPG_CAPABILITY_VERSIONS = {
  "capability.defenses": 1,
  "capability.modifiers": 1,
  "capability.position": 1,
  "capability.random": 1,
  "capability.resources": 1,
  "capability.stats": 1,
  "capability.vitality": 1,
} as const;
export type RpgCapabilityId = keyof typeof RPG_CAPABILITY_VERSIONS;
