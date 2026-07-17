/** Immutable normalized declarations accepted by the Rust compiler boundary. */
export type RpgIdentifier = string;

export interface RpgRuleModuleReference {
  readonly id: RpgIdentifier;
  readonly version: string;
}

export interface RpgRulesetDeclaration {
  readonly id: RpgIdentifier;
  readonly version: string;
  readonly modules: readonly RpgRuleModuleReference[];
}
