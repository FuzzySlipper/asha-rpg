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
- fixed and target-turn-end save-ends effect tenure, actor/target effect
  contribution lanes, and closed action/movement condition restrictions;
- one fixed-diamond spatial-source definition whose closed deterministic
  procedure is bound independently to enter, start-turn, end-turn, and exit;
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
8. proves canonical multiple save-ends failure and success, fixed expiry,
   restriction removal, stale option identity, duplicate active-effect
   rejection, and condition/tenure artifact tamper rejection; and
9. creates, overlaps, restores, ages, and expires fixed spatial sources,
   proves canonical boundary evidence over intermediary movement cells,
   verifies application-revision suppression, and rejects shape, trigger, and
   random-procedure tampering; and
10. replays the accepted sequences to the same state, hash, random position,
   and log.

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

The condition and movement witness is deliberately closed. It proves weighted
owner-turn movement allowance, cardinal push stop policy, bounded slide choice,
and one registered voluntary-leave-adjacency response through the same
TypeScript-authored/public-Rust consumer boundary. It does not claim a full
condition catalog, domination or control transfer, stealth/hearing,
concentration, permanent or rest-based tenure, or arbitrary condition
language. The spatial witness is limited to fixed Manhattan diamonds, four
closed boundaries, deterministic procedures, typed target filters, bounded
tenure, and source-aware stacking. Pull, teleport, diagonal/hex/flying
movement, moving zones, source collisions, summons, arbitrary polygons,
recursive triggers, general callbacks, and ready/delay remain explicit
non-claims.
