import type {
  RpgRuleModuleReference,
  RpgRulesetDeclaration,
} from '@asha-rpg/ir';

/** Pure construction helper. It does not execute or mutate RPG state. */
export function defineRuleset(
  id: string,
  version: string,
  modules: readonly RpgRuleModuleReference[],
): RpgRulesetDeclaration {
  return { id, version, modules: [...modules] };
}
