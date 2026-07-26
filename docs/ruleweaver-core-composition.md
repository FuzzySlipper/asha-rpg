# RuleWeaver-style core composition witness

Task #6250 records how the bounded core mechanics selected by campaign #6247
compose from existing ASHA RPG contracts. This is a reusable consumer witness,
not a RuleWeaver content port or compatibility claim.

The TypeScript source is
`examples/generate-ruleweaver-core-source.ts`. It declares:

- six input stats and four input defenses;
- one ordered d20 scalar profile whose natural 20 selects a critical band and
  whose natural 1 has no special rule;
- Standard, Bonus, and Reaction activation budgets with owner-turn reset;
- persistent Focus as an ordinary bounded resource, independent of activation
  reset;
- one shared typed attack procedure whose exact equipped item supplies range,
  damage dice, and damage type;
- two inert item definitions and actor feature/item/target-effect
  contributions using canonical typed and untyped stacking groups;
- cost-once multiple targeting, per-target scalar results, hit/miss/critical
  branches, and a bounded critical follow-on expressed as an ordinary sequence;
- a no-roll effect action and an unopposed healing action.

TypeScript prepares a closed PlayBundle. It does not select targets, reduce
contributions, roll dice, choose branches, mutate resources or vitality, or
execute the follow-on.

The independent Git consumer is
`consumers/minimal-game/src/bin/ruleweaver_core.rs`. Through only the public
`asha-rpg` facade it:

1. rejects tampered natural-band, duplicate-definition, dependency-lock, and
   out-of-domain contribution inputs before a session exists;
2. loads the exact prepared bundle and validates explicit scenario stats,
   defenses, Focus, class/feature selection, items, and equipment;
3. reads the two participant-local item-bound action options from Rust;
4. applies a target effect with a Bonus activation;
5. submits one Standard/Focus action against two canonical targets using the
   exact long-spear binding;
6. asserts the exact random requests, natural critical result, miss result,
   actor-feature/item/target-effect contribution decisions, bounded follow-on,
   damage, cost-once resource/budget events, and generic encounter readback;
7. proves stale, mismatched binding, unaffordable, and malformed-evidence
   rejection leave the checkpoint unchanged; and
8. replays the accepted sequence to the same state, hash, random position, and
   log.

The witness also protects the portable JSON spelling of
`RpgNaturalDieEffect::SetBand`: TypeScript emits `bandId`, and Rust decodes and
re-encodes that same generated boundary.

## Limits

Wits is supplied as an explicit setup defense whose downstream policy is the
better of Acuity and Intellect. This does not claim a general character
generation formula, derived vitality, class levels, progression, or rest-based
Focus recovery.

The follow-on is a bounded authored `sequence` under an outcome branch. It does
not claim recursive stages, arbitrary callbacks, deferred action grants, or
TypeScript continuation logic.

Line of effect, condition restrictions and save-ends, route-cost movement
allowances, forced movement, leave-adjacency reactions, and persistent spatial
sources remain owned by #6251 and its direct implementation children.
