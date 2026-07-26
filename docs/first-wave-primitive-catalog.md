# First-wave neutral RPG primitive catalog

## Status and boundary

This document is the architecture brief produced by Den task `#6179`. It
selects the smallest coherent semantic expansion needed for three independently
authored representative kits. It is a versioned implementation map. `F0@1` and
`F1@1` are implemented by tasks `#6180` and `#6197`; `F2` through `F6` remain
planned and are not support claims. Current support remains exactly what `design.md`,
`non-claims.md`, and the checked-in code say.

The catalog is intentionally not a Foundry adapter, a source-system
compatibility target, a content transcription, or an import format. The source
studies supplied bounded architecture evidence. Public ASHA names, data
structures, witnesses, and kit content are independently defined here.

TypeScript continues to construct immutable intent. Rust independently
validates, compiles, evaluates, mutates, emits events and trace, and owns
checkpoint/replay compatibility. No accepted primitive permits a TypeScript
semantic evaluator, callback, mutable gameplay context, source-system record,
or UI workflow at the portable boundary.

## Evidence receipts

All three dossiers were produced by `/home/dev/rpg-primitive-survey` at
`4fc9d28c7fe35d0d9e5a6010b886d72f13852d3e`. The generated dossiers and source
checkouts remain ignored under `.work`; none are copied into this repository.
Their machine receipts bind every report file by SHA-256.

| Study | Pinned source | Bounded included evidence | Candidate result |
| --- | --- | --- | --- |
| roll-over fantasy core | `foundryvtt/dnd5e` `6.0.x` at `65ee4f748f1d6d8d8cc00f2f7a81e67426927d5a` | 2,153 source-whitelisted documents, 1,488 structural groups | 16 candidates: 1 supported, 3 composable, 12 gaps |
| degrees-and-context core | `foundryvtt/pf2e` `v14-dev` at `91e5c792eeae4ee56610ff58fce28e65953ccbf9` | 3,161 documents from two explicit core source identities, 1,075 structural groups | 9 candidates: 1 composable, 8 gaps |
| multi-axis pool implementation | `StarWarsFoundryVTT/StarWarsFFG` `main` at `f989bf4fa8590ef83dd55d09bf0d15bf59690d18` | one system-owned schema witness plus pinned implementation pointers | 9 candidates: 1 composable, 8 gaps |

None of the studies launched a Foundry runtime. Stored and implementation
evidence is therefore used to identify plausible semantic requirements, not to
claim exact behavior. The third study's one schema witness is especially weak:
its accepted pool requirements are justified by direct pinned implementation
pointers and neutral worked hypotheses, not by source record frequency.

The ASHA comparison revision for all three dossiers is
`ed9eb6a5ba21e346cbe34279eb05c1709f2b6bf7`, which is also the base revision for
this synthesis.

## Selection rules

A requirement enters the first wave only when it is needed by at least one
representative kit and either recurs across studies or adds a strategically
different mechanic family. Frequency inside a source corpus is not a priority
signal.

The synthesis applies these rules:

1. Prefer a small authority operation or value model over a high-level
   source-system convenience.
2. Generalize an existing ASHA seam when its ownership, phase, and atomicity
   stay coherent.
3. Keep content nouns, labels, catalogs, balance, and presentation downstream.
4. Reject source package, sheet, chat, migration, derived-document, and
   probability-preview mechanics.
5. Defer a valid RPG concern when none of the three bounded kits needs it.
6. Fail closed on unknown versions, selectors, axes, definitions, facts,
   transforms, timing anchors, or capability owners.

## Cross-study semantic map

The source candidate ids below are provenance handles only. Coding tasks use
the neutral families `F0` through `F6`.

| Neutral concern | Source candidates | Decision |
| --- | --- | --- |
| typed action envelope and reusable equipment binding | `P01`, `P13` | `S0`: already supported |
| participant/cell target, team, range, cardinality, and check/DC relation | `P02` subset, `P04` subset, `PFX-D05`, `FFG-C10` | `S1` plus implemented `F1`: supported through composition and generic scalar profiles |
| sequenced damage/healing, fixed resource costs, and multiple bounded resource tracks | `P06` subset, `P08` subset, `FFG-R06` subset | `S2`: supported through composition; per-part interaction moves to `F4` |
| selected-cell movement, one before-damage reaction, and turn-bounded modifier aging | `P10` subset, `P12` subset, `P14` subset | `S3`: already supported |
| sealed class/feature selection and attack contribution provenance | `P15` subset, `P16` subset, `PFX-P09` subset | `S4`: already supported; generalized through implemented `F0` |
| typed contextual contributions and deterministic suppression | `P16`, `PFX-M03`, `PFX-C04`, `FFG-M05` | `F0@1`: implemented by `#6180` |
| generic scalar tests, ordered outcome bands, critical policy, and band adjustment | `P02`, `P03`, `PFX-O02`, `PFX-D05`, `FFG-E07` outcome subset, `FFG-C10` | `F1`: implemented |
| variable activation budgets | `P12` economy subset, `PFX-A01` | `F2`: needs generalization |
| named effect instances with bounded authority-relative expiry | `P10`, `P11`, `PFX-E07`, `FFG-E07` effect subset | `F3`: requires a new primitive |
| typed damage packets and qualified responses | `P06`, `P07`, `PFX-I06`, `P16` damage trace subset | `F4`: needs generalization |
| bounded area selection and spatial legality | `P05`, `PFX-S08` | `F5`: requires a new primitive |
| heterogeneous random pools and vector outcomes | `FFG-A01`, `FFG-A02`, `FFG-A03`, `FFG-A04`, `FFG-M05` | `F6`: requires a new primitive |

No interpreted candidate is accepted as a TypeScript-only content policy:
catalog nouns and balance stay downstream, but every selected meaning above
changes Rust legality, evaluation, timing, mutation, or readback. The studies'
ambiguous opposed/assisted semantics did not become candidate records and
remain deferred research rather than an invented first-wave contract.

### Exhaustive candidate disposition

Every interpreted dossier candidate has an explicit destination:

| Candidate | Current/first-wave destination | Excluded remainder |
| --- | --- | --- |
| `P01` | `S0` action/program composition | source activity-family labels |
| `P02` | `S1` existing attack/save/no-roll; `F1` generic scalar test | contested paired rolls |
| `P03` | `F1` outcome bands; consequence branches compose with `F4` | source-specific critical damage transforms |
| `P04` | `S1` participant/cell, team, range, cardinality | object/willing/mixed subjects and distinct self selector |
| `P05` | `F5` bounded grid areas | descriptive special ranges, sight, cover, arbitrary templates |
| `P06` | `S2` ordered operations; `F4` packet/part identity | temporary vitality and healing responses |
| `P07` | `F4` qualified damage responses | broad condition immunity |
| `P08` | `S2` fixed resource costs/changes | linked item depletion and heterogeneous recovery |
| `P09` | deferred | rest/day/initiative recovery scheduler |
| `P10` | `S3` turn modifiers; `F3` bounded effect expiry | wall clock, permanence, maintenance, destruction/dispel triggers |
| `P11` | `F3` named effect application | arbitrary property mutation and condition ontology |
| `P12` | `S3` existing before-damage reaction; `F2` economy | general trigger/window/priority authoring |
| `P13` | `S0` exact item/procedure binding | inventory economy and runtime equip operations |
| `P14` | `S3` selected-cell movement | summon, teleport, enchant, transform, delegation |
| `P15` | `S4` sealed class/feature setup | runtime levels, grants, prerequisites, and choices |
| `P16` | `F0` contribution ledger; `F4` damage response trace | reroll/keep and other unselected roll mechanics |
| `PFX-A01` | `F2` activation budgets | system-specific action labels |
| `PFX-O02` | `F1` outcome/natural/contextual band adjustments | unproven source adjustment ordering |
| `PFX-M03` | `F0` typed groups and suppression | source selector/rule-element vocabulary |
| `PFX-C04` | `F0` bounded typed facts | arbitrary option bags and mutable toggles |
| `PFX-D05` | `S1` defense relation; `F1` generic scalar test | source-specific derived statistic paths |
| `PFX-I06` | `F4` per-part responses | recurring damage timing |
| `PFX-E07` | `F3` bounded effect state/expiry | ephemeral hooks and persistent triggers |
| `PFX-S08` | `F5` bounded grid areas | movement modes, elevation, cover, sight |
| `PFX-P09` | `S4` sealed item/class/feature selection | runtime progression and derived-data preparation |
| `FFG-A01` | `F6` heterogeneous pool | none inside the bounded candidate |
| `FFG-A02` | `F6` pairwise axis cancellation | unobserved negative/zero-crossing source edge cases |
| `FFG-A03` | `F6` explicit die replacement/fallback | source UI controls |
| `FFG-A04` | `F6` face vectors | source face tables, glyphs, and distributions |
| `FFG-M05` | `F0` provenance plus `F6` typed pool effects | unproven source duplicate/ordering policy |
| `FFG-R06` | `S2` multiple bounded resources | derived threshold lifecycle and vehicle-specific roles |
| `FFG-E07` | `F1` coupled outcome bands plus `F3` short effects | critical tables and unobserved duration scheduling |
| `FFG-T08` | deferred | result-derived initiative and role substitution |
| `FFG-C10` | `S1` difficulty formula plus `F1` generic test | opposed/assisted semantics not evidenced |

### Existing composition that must be proven, not rebuilt

`S0` through `S4` are requirements on implementation witnesses and kit
acceptance; they are not permission to add parallel abstractions.

- `S0`: `RpgIrAction`, bounded programs, owner-bound action procedures, inert
  item attributes, Scenario item instances, and exact equipment bindings
  already provide the neutral action/equipment seam.
- `S1`: attack, saving-throw, and no-roll checks already compare formula values
  to named defenses or difficulties. Existing participant/cell, team, range,
  and cardinality selectors remain valid alongside the new surfaces.
- `S2`: sequences already preserve ordered typed damage events, healing,
  resource costs, resource changes, and atomicity. A participant may already
  own several independent bounded resources; the first wave does not need a
  second resource store.
- `S3`: selected-destination pathfinding, one typed before-damage reaction, and
  modifier aging already have authority, event, checkpoint, replay, and
  readback coverage. The kits reuse those exact paths.
- `S4`: Scenario-bound classes/features and source-labelled scalar
  contributions are authoritative. `F0` and `F1` replace the attack-only
  restriction without creating another character-feature runtime.

## Common extension contract

Every `F*` implementation follows the repository's normal extension path:

- A Ruleset provides an exact, versioned Rust-bound model and any Ruleset-owned
  named contracts. A Content Pack declares an exact requirement and references
  only exported definitions and provided names.
- `@asha-rpg/authoring` exposes immutable builders and structural diagnostics.
  It may normalize and sort but may not evaluate applicability, rolls,
  outcomes, geometry, timing, stacking, or state.
- Normalized IR and prepared/compiled PlayBundle schemas use strict tagged
  unions, deny unknown fields, and version independently. Rust reconstructs
  private plans from the retained declarations.
- Rust validates reference closure, ownership, numeric domains, graph depth,
  cardinality, expansion, and semantic compatibility before mutable state
  exists whenever possible.
- Runtime reads staged authority state, evaluates legality and meaning, and
  commits all capability writes and events atomically. Rejection consumes no
  random evidence and preserves state, log, turn, pending phase, and replay
  boundary.
- Every authority-relevant declaration participates in source/semantic
  fingerprints, PlayBundle identity, Scenario binding, checkpoint, portable
  state hash, replay compatibility, and typed readback as applicable.
- Events and readbacks retain the authored source identity and intermediate
  authority decisions needed by a human UI. A client never reconstructs a
  total, target set, outcome, suppression decision, expiry, or damage response.
- Bounds are public contract, not test-only constants. An implementation task
  may choose lower limits than the ceilings below, but may not silently raise a
  ceiling or accept an unbounded form.

## F0 — Contextual contribution ledger

Implementation status: supported as `F0@1` by task `#6180`. The checked-in
authoring, prepared/compiled artifact, Rust execution, event/trace, and replay
contracts are the implementation truth for this section.

### Semantic and version boundary

`F0@1` generalizes the current attack-feature contribution path into one
authority-owned ledger for named scalar calculation selectors. A contribution
has a stable id, source definition, source label, Ruleset-owned selector id,
signed value expression, Ruleset-owned stacking group, and a bounded predicate.
The selected Ruleset declares each selector's numeric domain and each stacking
group's deterministic reduction policy.

Initial group policies are:

- `sum`: retain every applicable contribution;
- `greatest`: retain the numerically greatest contribution;
- `least`: retain the numerically least contribution;
- `signed-extremes`: retain at most the greatest positive and least negative
  contribution.

Ties use canonical source-definition id then contribution id. Contributions
whose predicates are false or which lose group reduction remain in the ledger
as `inapplicable` or `suppressed`; they do not disappear.

`F0@1` applies scalar integer effects only. Pool-dimension and category
transform effects arrive in `F6` as a new contribution-effect version, not as
untyped data smuggled into `F0`.

### Authoring and normalized representation

Ruleset authoring adds closed named calculation selectors and stacking-group
contracts. Content feature, item, and effect schemas reference them nominally.
The normalized declaration retains owner ids, source ids, selector, group,
formula, predicate, and version. The compiler canonicalizes declaration order
without using export order as a semantic tie-breaker.

The first predicate vocabulary is bounded Boolean composition over typed
authority facts already available at resolution: actor/target identity and
team relation, living state, actor/target named values, distance, current
flanking/surrounding facts, exact bound item identity/tags, selected action
tags, and current cell capabilities. `F3` extends the vocabulary with active
effect facts. There is no arbitrary string option set or generic predicate VM.

### Validation and authority

The compiler limits one source to 32 contributions, one selector evaluation to
256 candidates, predicate depth to 16, predicate nodes to 128, and formula
nodes to existing formula bounds. It rejects duplicate ids, unknown owners,
selectors, groups, facts, item tags, catalogs, and out-of-domain possible
results.

Rust gathers candidates after targets and exact item binding are known but
before the selected calculation is resolved. It evaluates predicates against
one staged state revision, reduces each group canonically, performs checked
integer arithmetic, and supplies the resulting value to the owning resolution
phase. `F0` reads stats, defenses, resources, positions, vitality, selected
definitions, item binding, and cell capabilities; it owns no mutable
capability.

### Randomness, timing, events, and persistence

`F0` uses no randomness and cannot open a timing window. Its evaluation is
repeated from immutable declarations and staged state on every proposal.
Resolution readback contains every candidate in canonical order with source,
selector, group, declared value, applicability, suppression reason, and applied
value, followed by the checked final value. Accepted events retain the same
ledger. Checkpoint/replay persist the source definitions and resulting events,
not a mutable contribution cache.

### Witnesses and non-claims

- Two same-group bonuses and two penalties prove `signed-extremes`, tie
  ordering, suppressed-source visibility, and export-order independence.
- Actor/target position, a bound item tag, and a cell capability independently
  flip applicability without changing content declarations.
- Invalid selectors, predicates, overflows, and duplicated ids fail in both
  TypeScript preparation and independent Rust loading where applicable.

`F0@1` does not claim arbitrary roll options, mutable toggles, user-authored
predicates, probability preview, generic script evaluation, or pool
construction.

## F1 — Generic scalar tests and ordered outcomes

### Semantic and version boundary

`F1@1` replaces fixed hit/miss and saved/failed assumptions with a Ruleset-owned
scalar test profile. A test declares who supplies the rolled value, one primary
numeric die, how its base and difficulty are computed, which `F0` selectors
contribute, and an ordered closed set of outcome-band ids.

The profile first maps the checked total-versus-difficulty margin through a
complete, disjoint threshold table into a base band. It may then apply
disjoint natural-die rules which set a named band or shift by a bounded number
of bands. Finally, applicable contextual band shifts move one at a time in
canonical source order through the declared ordering and clamp after each
shift at its ends. An action branches on exact band ids. A profile
may map old attack/save vocabulary to bands, but the normalized contract does
not reserve source-system outcome names.

Natural-die policy, margin thresholds, and band shifts are separate typed
rules. A flat modifier cannot silently act as a band shift, and a band shift
cannot alter the recorded roll total.

### Authoring and normalized representation

Ruleset authoring adds scalar-test profiles with stable ids, one primary die
whose sides are `2..=100`, base formula, difficulty source, ordered bands,
margin thresholds, natural-die rules, and named `F0` selectors. Actions add a
generic scalar-test check and an outcome-branch map. Existing attack/save
builders may be re-expressed through supplied profiles during the schema
migration, while no-roll remains an evidence-free program path; Rust sees one
authoritative check model.

Classification rules are data for a closed Rust evaluator, not an expression
language. A profile has at most 16 bands, 32 margin thresholds, and 8 disjoint
natural-die rules. An action must provide only known branches and may define a
typed default; unreachable or duplicate bands fail compilation.

### Validation and authority

Rust verifies the profile owner, numeric domains, exactly one primary die,
complete/disjoint margin coverage, disjoint natural values, bounded shifts,
band closure, selector compatibility, and complete branch references. Generic
tests support actor statistic versus explicit difficulty or target named
defense. Contested actor-versus-actor roll pairs remain deferred.

At runtime Rust gathers `F0` contributions, requests the exact scalar evidence,
computes base/difficulty/total/margin with checked arithmetic, selects the base
band, applies the matching natural rule, then applies canonical contextual
band-shift contributions, selects one program branch, and stages its mutations
in the existing atomic root. Legacy attack and save checks use the same ordered
scalar resolver with their existing two-band projections and events.

### Randomness, timing, events, and persistence

The initial profile uses existing homogeneous numeric die requests. The request
records count, sides, subject, target order, and semantic path; replay consumes
the exact recorded evidence. `F6` adds heterogeneous requests without changing
the band/readback contract.

The accepted event reports profile, primary-die evidence, base expression
values, contribution ledgers, difficulty, total, margin, base band,
natural-die rule/result, each contextual band shift, final band, and selected
branch. These values enter trace and replay comparison. Profile definitions and
versions participate in artifact identity and compatibility.

### Witnesses and non-claims

- The committed public-facade witnesses exercise a four-band profile, natural
  set/shift rules, and the legacy two-band attack/save projections.
- Positive and negative shifts prove ordering, clamping, source visibility,
  and independence from numeric modifiers.
- A malformed overlapping profile, unknown band, overflow, extra branch, or
  substituted evidence fails closed without mutation.

`F1@1` does not claim opposed or assisted checks, hidden-information tests,
reroll/keep mechanics, heterogeneous dice, or source-specific critical damage
rules. Damage consequences are ordinary branch programs and `F4` packets.

## F2 — Variable activation budgets

### Semantic and version boundary

`F2@1` generalizes one-action turns into Ruleset-owned named activation budgets.
An action activation declares a timing kind and zero or more exact
budget/amount costs. Turn initialization and accepted turn transitions reset
the configured turn-relative budgets. Reaction budgets remain distinct and may
be consumed only in a Rust-owned reaction phase.

Zero-cost activations are legal but the model includes a per-turn accepted
activation ceiling so a free-action loop cannot create unbounded state or log
growth. A passive definition is not an activation and cannot be proposed.

### Authoring and normalized representation

The Ruleset declares at most 8 budget ids, each with a numeric domain, reset
boundary, and initial amount. Actions reference known budgets and timing kinds.
Scenario does not submit current budget values; Rust derives them from the
Ruleset and turn state. Content cannot define a new economy model by naming a
budget.

### Validation and authority

The compiler rejects duplicate costs, negative amounts, unknown budgets,
incompatible timing, unreachable action costs, and a per-turn activation
ceiling above 64. Legality projection omits actions that cannot pay all costs.

Rust reads and stages the turn-budget owner before action consequences. Costs,
consequences, reaction decisions, events, and budget state commit together.
Rejection or a declined/failed pending transaction restores the same budget.
Turn-control authority resets only the budgets whose declared boundary
matches.

### Randomness, timing, events, and persistence

Budget payment is not random. Accepted action events identify each spent
budget and before/after values. Turn events identify resets. Encounter readback
shows remaining budgets and why an otherwise selected action is illegal.
Checkpoint, state hash, replay, and pending-reaction state retain the budget
values and accepted activation count.

### Witnesses and non-claims

- A three-unit turn executes a two-unit and then one-unit action, while a later
  two-unit proposal is absent or atomically rejected.
- A zero-cost action hits the accepted-activation ceiling.
- A pending reaction proves that both normal and reaction budgets restore on
  invalid evidence and commit once on acceptance.

`F2@1` does not claim real-time actions, rest/day recovery, initiative-derived
budgets, action borrowing, delay/ready rules, or a general scheduler.

## F3 — Named effects with bounded expiry

### Semantic and version boundary

`F3@1` adds exported semantic effect definitions and authority-owned effect
instances. A definition contains a stable schema version, bounded rank domain,
`F0` contributions, stacking identity, and one authority-relative duration.
Applying an effect creates or deterministically replaces/refreshes an instance;
removal and expiry are explicit authority transitions.

Initial durations are positive counts anchored to one of: accepted global turn
transition, source actor turn start, target actor turn start, or round
transition. There is no wall-clock duration. Initial stacking policies are
`independent-by-source`, `replace`, and `refresh`; ties use source and instance
identity, never export order.

- `independent-by-source` allows one instance per target, stacking identity,
  and exact source. Reapplication from that source preserves the instance id,
  replaces rank/source fields with the new evaluated values, resets
  `remainingCount`, and updates the application revision; a distinct source
  creates a distinct instance.
- `replace` removes every target instance with the stacking identity in
  canonical instance order, emits their removal events, and creates one new
  instance.
- `refresh` maintains at most one target instance for the stacking identity.
  Reapplication preserves its instance id, replaces its source/rank with the
  new evaluated values, resets `remainingCount`, and updates the application
  revision; absence creates one new instance.

`remainingCount` is the number of eligible matching future boundaries before
the instance expires. An instance is active exactly while that value is
positive. A matching boundary decrements it once; `1 -> 0` removes the instance
at that boundary. Each instance has exactly one anchor, so one transition can
age it at most once:

- `globalTurnTransition` matches every accepted transition to a new turn;
- `roundTransition` matches only a transition which increments the round;
- `sourceTurnStart` matches when the new current actor is the stored source;
- `targetTurnStart` matches when the new current actor is the stored target.

### Authoring and normalized representation

Content Packs author closed effect definitions and actions use
`applyEffect`/`removeEffect` operations referencing exported definitions.
Normalized IR retains source, target subject, rank formula, duration count and
anchor, and exact definition version. An effect may contribute only through
typed `F0` declarations or other registered effect fields; it cannot contain
arbitrary property paths.

### Validation and authority

The compiler limits 128 effect definitions per package, 32 contributions per
effect, 64 active instances per participant, duration counts to `1..=1000`,
and rank to the definition's numeric domain. It rejects unknown definitions,
cycles, invalid stacking identities, zero duration, unsupported anchors, and
effect predicates that read undeclared facts.

Rust owns a private effect-instance capability. Application evaluates rank and
legality against staged state, creates a canonical instance identity, and makes
its contributions visible only after the operation's defined commit point.
One action cannot observe a partially applied effect.

Every accepted transition uses this single staging and event order:

1. finish the outgoing action/control operations, including effect
   apply/refresh/replace/remove events;
2. determine and stage the new round, turn, and current actor, then append the
   round/turn transition events;
3. age matching effects in anchor order `globalTurnTransition`,
   `roundTransition`, `sourceTurnStart`, `targetTurnStart`, then by target id,
   definition id, source id, and instance id;
4. age existing turn-bounded modifiers;
5. reset matching `F2` activation budgets;
6. calculate encounter outcome and project the next legal interaction view.

All six steps commit atomically and no proposal or reaction can interleave.
An instance applied or refreshed in the same accepted transaction is marked
with that transaction revision and is ineligible for every boundary in that
transaction; its first possible decrement is a matching boundary in a later
accepted transaction. Expiry at step 3 removes its contributions before
modifier aging, budget reset, outcome calculation, and next-turn action
projection.

### Randomness, timing, events, and persistence

Effect lifecycle itself is deterministic. For `n > 1`, a matching boundary
emits one aged event with `beforeCount=n` and `afterCount=n-1`. For `n = 1`, it
emits one expired event with `beforeCount=1` and `afterCount=0`; it does not
also emit an aged event. Events report definition, source, target, anchor, and
the transaction/boundary identity. Readback exposes only still-active
instances and their currently applicable/suppressed `F0` contributions.

Checkpoint and replay represent only stable states before or after the complete
transition sequence; there is no serializable transition-in-progress phase.
The portable state, state hash, and pending transaction retain exact instances,
remaining counts, application revision, and anchors. Restoring a pre-boundary
checkpoint makes the next accepted transition age once; restoring a
post-boundary checkpoint never ages the same boundary again. A pending reaction
cannot advance a turn.

### Witnesses and non-claims

- A short effect contributes to a check, expires at its exact target-turn
  boundary, and no longer appears as applied on the next resolution.
- Two sources prove independent stacking; repeated same-source application
  proves replace versus refresh.
- Apply and refresh during a transaction which also advances the matching
  anchor prove that the same-transaction boundary is skipped.
- Restore/replay at one transition before expiry produces the same events,
  ledger, and hash.
- Adversarial pre-boundary, post-boundary, and invented transition-in-progress
  checkpoints prove exactly-once aging and atomic restore.

`F3@1` does not claim permanent effects, concentration/maintenance, wall-clock
units, recurring damage, before/after-roll hooks, arbitrary triggers, a
condition ontology, or document-property mutation.

## F4 — Typed damage packets and qualified responses

### Semantic and version boundary

`F4@1` applies one bounded packet containing independently identified damage
parts. Each part has an amount formula, Content-Pack-owned damage type, and
bounded tags. The target's definitions and active `F3` effects may provide
typed response records matched by damage type, tags, and explicit bypass tags.

Initial response effects are `immune`, signed flat adjustment, and positive
rational scale. Every response has a stable id unique within its source
definition or effect instance. Its runtime identity is
`(sourceDefinitionId, sourceInstanceId-or-empty, responseId)`. Duplicate exact
runtime identities are corrupt state and reject the command; responses from
distinct source instances remain distinct.

Rust evaluates every part in lexicographic part-id order and uses this exact
pipeline:

1. collect candidates and order them by response phase (`immune`, `flat`,
   `scale`) then runtime identity;
2. classify filter mismatches as `inapplicable` and explicit bypass matches as
   `suppressed:bypassed`;
3. if any eligible immunity remains, mark every eligible immunity `applied`,
   set the part to zero, and mark every eligible flat/scale
   `suppressed:immunity`;
4. otherwise checked-sum every eligible signed flat value without intermediate
   clamping, add that one sum to the original amount, and clamp once at zero;
5. apply eligible scales in runtime-identity order, checking
   `amount * numerator` and using mathematical floor division by denominator
   after each scale;
6. clamp the final packet sum to the existing vitality mutation domain.

Canonical scale order is semantic because per-step floor can make scale order
observable. Enumeration or export order never participates.

Every original part remains visible even when its final amount is zero. Packet
aggregation is presentation/readback convenience; vitality changes once for
the checked final sum.

### Authoring and normalized representation

Content damage types remain inert named catalog values. Response definitions
and packet operations are registered semantic schemas with exact versions.
Normalized IR retains part ids, formulas, type/tag references, response ids and
source references, and bypass facts. A packet has at most 16 parts; a
participant may contribute at most 64 candidate responses; each rational
numerator and denominator is positive and no greater than 1000. Duplicate
response ids inside one definition fail compilation.

### Validation and authority

Rust validates catalog ownership, unique part ids, nonnegative possible
amounts, tags, response closure, rational bounds, and worst-case checked
arithmetic. Runtime gathers responses from selected definitions and active
effects, evaluates each part against one staged revision, and stages one
vitality mutation plus its events. Invalid, overflowing, or stale inputs reject
the entire packet and any surrounding action.

`F4` reads definitions, effects, and vitality and writes only through the
existing vitality owner. It does not make damage types executable content.

### Randomness, timing, events, and persistence

Part formulas use the existing exact random-evidence path. The packet's random
requests retain part and target identity. The damage event contains every
original part, every candidate in canonical order with runtime identity and
`inapplicable`/`suppressed`/`applied` reason, the flat sum and single clamp,
each scale's before/after value and floor, final part amounts, original packet
sum, adjusted packet sum, actual bounded vitality delta, and before/after
vitality. The requested and applied amounts can therefore differ without a
misleading event.

Packet semantics and response definitions bind artifact/checkpoint/replay
compatibility. Replay compares the structured per-part trace and final state.

### Witnesses and non-claims

- A two-part packet meets a response for only one type.
- An explicit bypass tag suppresses one matching response.
- Immunity, flat adjustment, scale, rounding, vitality-floor clamping, and
  requested-versus-applied event values are separately witnessed.
- Permutations of two flats and two non-commuting scales prove identical
  canonical response order and result; the trace proves one flat clamp and
  per-scale floors.
- Duplicate runtime identity, tampered response ordering, and missing
  definitions fail independently in Rust.

`F4@1` does not claim temporary vitality, healing responses, armor wear,
recurring damage, critical tables, body locations, or user-selected allocation.

## F5 — Bounded area selection and spatial legality

### Semantic and version boundary

`F5@1` adds authority-selected participant sets derived from a submitted cell
anchor. The first square-grid shapes are:

- `diamond`: cells whose orthogonal Manhattan distance from the anchor is no
  greater than the declared radius;
- `orthogonal-line`: cells from the origin through an aligned selected anchor,
  up to the declared length.

The action declares origin kind, shape, range to anchor, team constraint,
living-state requirement, and minimum/maximum affected participants. The
caller selects one projected legal anchor; Rust derives and canonically orders
the cells and participants. A caller cannot omit an eligible target or add an
ineligible one. An empty set is legal only when the declared minimum is zero.

### Authoring and normalized representation

Target selectors gain a versioned area variant with closed shape records and
integer bounds. An action program uses the existing bounded per-target body
over the authority-derived set. The normalized form does not contain canvas
coordinates, templates, pixels, line-of-sight callbacks, or client-computed
participants.

### Validation and authority

The compiler limits radius/length/range to the board contract, affected cells
to 256, and participants to the existing target ceiling. It rejects impossible
shape parameters, a minimum greater than the maximum, cell targets combined
with participant-only operations, and program maximums smaller than the
selector maximum.

Encounter projection lists each legal anchor with ordered affected cell and
participant ids. Every option binds the exact session binding id, artifact id,
Scenario fingerprint, state revision, round/turn/current actor, action id,
exact item binding, and anchor cell. Submission carries the session binding id,
state revision, ordinary action/item identity, and anchor; it never submits the
derived cells or participants.

Any accepted command, reaction, or turn control which changes the state
revision invalidates every older area option, even if its geometry would happen
to be unchanged. Activation, checkpoint restore, replay restoration, or
authority-session replacement changes the opaque session binding id, so an
equal numeric revision cannot resurrect an old option. Refreshing readback
without authority change does not change either identity.

Under the session's exclusive authority lock, Rust first requires the submitted
session binding and revision to equal the current values. A session-binding
mismatch rejects with `RPG_AREA_OPTION_STALE` at
`$.proposal.sessionBindingId`; a revision mismatch uses the same code at
`$.proposal.authorityRevision`. Either consumes no random evidence,
mutates/logs nothing, and returns the current interaction readback. Rust never
reinterprets a stale anchor. If identities match, it recomputes cells and
participants once against that same revision, revalidates minimum/maximum
bounds, then requests evidence and executes the resulting per-target programs
as one atomic command. A same-revision anchor which is not projectable rejects
as `RPG_AREA_OPTION_INVALID` at `$.proposal.anchorCellId`; it cannot fall back
to a different current target set.

### Randomness, timing, events, and persistence

Target ordering fixes per-target random request order. Events and readback
retain shape, origin, anchor, included cells, included participants, and every
filtered participant with reason. Accepted events and replay entries also
retain the accepted session-independent proposal revision, normal post-command
revision, and exact derived set; the opaque live session binding is not a
portable replay field. Scenario, checkpoint, replay, and state hash already
bind board/positions; schema versions add the shape contract and exact
selection trace.

### Witnesses and non-claims

- Moving a participant, accepting an unrelated state change, restoring a
  checkpoint with the same numeric revision, or replacing the active authority
  between projection and submission all reject the old proposal as stale.
- Equal-distance cells and multiple occupants use canonical cell then
  participant ordering.
- An area action proves per-target rolls, shared rolls, explicitly allowed and
  disallowed empty areas, team filtering, and minimum/maximum-bound rejection.

`F5@1` does not claim cones, arbitrary polygons, elevation, cover, line of
effect, diagonal movement, hex grids, footprints, aura persistence, collision
volumes, or client-rendered template authority.

## F6 — Heterogeneous random pools and vector outcomes

### Semantic and version boundary

`F6@1` adds a Ruleset-owned pool model whose die types have independent side
counts and face-to-vector tables over Ruleset-owned result axes. A pool
contains canonical die-type counts plus automatic axis contributions. Rust
draws every die, maps each face to its vector, performs checked aggregation,
then applies declared pairwise cancellation and bounded vector-outcome rules.
It reuses `F1`'s versioned outcome-band identities, branch envelope, and
readback shape, not its scalar margin classifier.

Face vectors may contribute to several axes at once, so a special face can
couple a primary and secondary result without hard-coded source vocabulary.
Cancellation pairs are directional axis contracts, not string conventions.
The final result retains raw totals, automatic contributions, cancellation
deltas, net axes, and derived `F1` outcome bands.

Pool contribution effects extend `F0` with exact tagged forms:
`add-dice`, `add-axis`, and `replace-or-add-die`. A replacement names `from`,
`to`, count, and an explicit fallback die type when no source die remains.
Flat scalar contributions cannot be interpreted as pool changes.

Every pool contribution has runtime identity
`(sourceDefinitionId, sourceInstanceId-or-empty, contributionId)`. Duplicate
exact identities reject the resolution. After `F0` predicate and stacking-group
reduction, surviving contributions are sorted by that identity inside their
semantic phase; export, package, participant, and input enumeration order never
participate.

Pool construction and reduction use these exact phases:

1. materialize the action's base die counts and base automatic axis values;
2. group all `add-dice` contributions by die type, checked-sum their signed
   deltas, add each sum once, and reject any negative or over-limit result;
3. execute `replace-or-add-die` contributions sequentially by runtime identity;
4. freeze the resulting pool and request exactly that heterogeneous evidence;
5. map faces and checked-sum their vectors, then add base and `add-axis`
   automatic values grouped by axis;
6. cancel each declared disjoint axis pair and evaluate vector outcome bands.

A replacement executes `count` positive unit steps. On each step, if one
`from` die currently exists, Rust removes it and adds one `to` die; otherwise
it adds one `fallback` die. The next unit observes that updated pool. Therefore:

- replacements may consume dice introduced by the phase-2 additions;
- a later replacement may consume `to` or fallback dice produced by an earlier
  replacement;
- when `fallback == from`, a later unit of the same replacement may consume the
  newly added fallback;
- a replacement never revisits an earlier contribution, so type-reference
  cycles do not cause recursion and are permitted;
- `from == to`, zero count, unknown ids, overflow, and a pool exceeding the
  total-die bound reject before any random request.

The trace records, for every unit, whether it replaced `from` or selected the
fallback and the before/after counts. This sequential cascade is part of
`F6@1`; an implementation may not substitute a snapshot/non-cascading pass.

### Authoring and normalized representation

Ruleset authoring declares at most 64 die types, 32 axes, 32 cancellation
pairs, and 16 derived outcome bands. Die sides are `2..=100`; every face has an
explicit bounded vector. Actions select a pool profile and declare bounded base
terms and difficulty terms. Actor, item, feature, and active-effect sources add
typed `F0` pool contributions.

Normalized IR retains the complete face tables, axis contracts, contribution
forms, canonical reduction order, and profile version in the compiled
PlayBundle identity. Presentation glyphs and source-system symbol names remain
downstream.

### Validation and authority

Rust rejects incomplete/duplicate face tables, unknown axes/dice, duplicate
cancellation-axis ownership, negative final die counts, invalid self
replacement, zero/out-of-range replacement counts, more than 64 pool terms,
more than 256 total dice, more than 128 reduction operations, or any possible
checked-integer overflow. Paired cancellation axes must be nonnegative before
cancellation; their face vectors and automatic contributions therefore cannot
declare negative values.

At runtime Rust gathers `F0` sources and completes phases 1 through 3 before
requesting any randomness. It then evaluates phases 4 through 6, chooses
programs, and commits all consequences atomically. TypeScript and UI receive
the constructed pool only as authority readback.

### Randomness, timing, events, and persistence

The random-source contract gains a heterogeneous request containing an ordered
list of die-type/count/sides entries and a total draw ceiling. Evidence retains
each die type, ordinal, sides, and value. A source must answer exactly the
request; missing, extra, wrong-sided, or out-of-range evidence rejects without
advancing the accepted random position.

Accepted events and trace expose base pool, every applied/inapplicable/
suppressed contribution with runtime identity, grouped die deltas, each
replacement unit and fallback decision, frozen pool, raw evidence, per-face
vectors, grouped automatic axes, raw axes, cancellation, net axes, derived
bands, chosen branches, costs, and mutations. Checkpoint and replay persist and
compare the exact heterogeneous request/evidence and all model versions.

### Witnesses and non-claims

- Two die types with different side counts and original face tables produce
  primary success plus an independent secondary benefit or complication.
- Separate opposing axis pairs cancel to zero independently while an uncoupled
  axis remains.
- Permutations of several source contributions produce the same phase/key
  order, pool, request, and trace.
- An added die is consumed by a replacement; one replacement's result feeds a
  later replacement; and repeated units with `fallback == from` prove the
  declared cascade.
- Contention for one source die proves that the first canonical replacement
  consumes it and the next takes fallback. A non-cascading expected result is
  explicitly rejected by the witness.
- Tape exhaustion, wrong die type/order, missing/extra evidence, overflow, and
  tampered face tables/transforms fail closed.

`F6@1` does not claim branded dice, proprietary face distributions, a
probability calculator, user-edited authority pools, opposed/assisted check
workflows, exploding dice, rerolls, keep/drop, or arbitrary reduction scripts.

## Representative kit briefs

The kit tasks must use these exact semantic subsets. They may add original
content nouns and presentation but may not expand engine acceptance.

### K0 — Roll-over tactical kit

Required existing surfaces: `S0` through `S4`.

Required first-wave surfaces: `F0`, `F1`, `F3`, `F4`, and `F5`.

The kit contains one item-bound attack, one defense test, one fixed resource
cost, one selected-cell movement action, one existing before-damage reaction,
one two-part damage packet with a qualified response, one short effect that
changes a later contribution, and one area action. It proves ordinary and
critical-style bands through an original scalar profile. It does not contain
rest recovery, summons, runtime progression, broad reaction authoring, or
temporary vitality.

### K1 — Degrees-and-context tactical kit

Required existing surfaces: `S0` through `S4`.

Required first-wave surfaces: `F0` through `F5`.

The kit uses a multi-unit activation budget, a four-band scalar profile,
bounded band shifts, same-group contribution suppression, actor/target/item/
cell predicates, one expiring effect, per-part damage responses, and one area
selector. The event log must show applicable, inapplicable, and suppressed
sources plus the base and final outcome band. It does not reproduce a
source-system selector vocabulary, progression tree, persistent-damage timing,
or derived-document preparation.

### K2 — Multi-axis pool kit

Required existing surfaces: `S0`, `S2`, `S3`, and exact item binding from `S4`.

Required first-wave surfaces: `F0`, `F1`, `F3`, and `F6`.

The kit defines original die names, face vectors, distributions, axes, and
cancellation pairs. Actor and item sources add dice and automatic axes; one
source performs a die replacement with an explicit fallback. A roll produces
one primary band plus an independent secondary benefit or complication and
drives a short effect or resource consequence. Two existing resource tracks
remain independent. It does not use source branding, glyphs, distributions,
initiative rules, a pool-builder UI, or probability preview.

## Implementation order

The dependency order is part of the brief:

```text
#6180 F0 contribution ledger
    |
    +--> #6197 F1 scalar tests/outcomes --> #6202 F5 area selection
    |             |
    |             +---------------> #6200 F6 heterogeneous pools
    |
    +--> #6198 F2 activation budgets --> #6199 F3 effect instances
                                            |
                                            +--> #6201 F4 damage packets/responses
```

`#6180` owns only `F0`. Each `F1` through `F6` family is a separate child under
`#6181`, with the dependencies shown above. A child may update schema versions
needed by its family, but may not absorb a sibling for convenience. The parent
`#6181` closes only after all six children have exact-SHA approval and the
support map is refreshed.

If a batch proves it needs a generic `rusty-engine` facility, it creates or
links an upstream task and waits for a public pinned boundary. Asha RPG must not
copy a private substitute into its own crates.

## Deferred valid RPG concerns

These observations remain plausible later work but are unnecessary for the
bounded first-wave kits:

- contested/opposed and assisted multi-actor resolution;
- reroll, keep/drop, exploding dice, advantage/disadvantage, and user-edited
  roll pools;
- object/willing/mixed-subject targets, self as a distinct selector, cones,
  arbitrary templates, elevation, cover, and line of effect;
- temporary vitality, healing responses, recurring/persistent damage, critical
  tables, and user-selected damage allocation;
- rest/day/dawn/initiative recovery schedules (`P09`) and linked item-use
  depletion;
- permanent, wall-clock, maintenance/concentration, destruction-bound,
  before/after-roll, and arbitrary triggered effects;
- general reaction-window authoring, priority, ready/delay, and action
  borrowing;
- entity creation, summons, teleport, enchantment, transformation, and
  delegated actions;
- runtime levels, prerequisites, grants, boosts/flaws, feature choices, and
  derived progression preparation;
- result-derived initiative and vehicle-role substitution (`FFG-T08`), plus
  campaign persistence.

Deferral means no first-wave child and no implementation claim. A later survey
or kit must select a bounded contract before any of these enter ASHA.

## Rejected source/workflow accidents

The following do not become ASHA RPG primitives:

- Foundry document, pack, UUID, rule-element, activity-family, embedded-item,
  sheet, chat-card, target-set, migration, tour, localization, and package
  structures;
- source-specific derived-data preparation and arbitrary property-path
  mutation;
- browser pool editors, modifier dialogs, probability simulation, visual
  templates, and presentation glyphs;
- economic item price fields without authority action-cost evidence;
- source labels, branded terms, distinctive content, exact distributions,
  catalogs, prose, or stat blocks.

Coding agents should use this document and the resulting Den task contracts.
They do not need to inspect, clone, or cite the Foundry repositories or
generated dossiers.

## Completion accounting

Every selected kit requirement maps to `S0`–`S4`, `#6180/F0`, or exactly one
`#6181` child:

| Requirement | Owner |
| --- | --- |
| immutable actions/procedures, items/equipment, fixed costs/resources, movement/reaction, sealed class/features | existing `S0`–`S4` |
| contextual source ledger and suppression | `#6180` / `F0` |
| scalar tests, ordered bands, shifts, generic difficulty | `#6197` / `F1` |
| multi-unit/free/reaction budgets and reset | `#6198` / `F2` |
| named effect instances and bounded expiry | `#6199` / `F3` |
| damage parts, responses, rounding, requested/applied trace | `#6201` / `F4` |
| area anchors, authority-derived participants, stable roll order | `#6202` / `F5` |
| heterogeneous evidence, face vectors, cancellation, pool transforms | `#6200` / `F6` |

The first wave is complete only when the mapping reflects implemented,
reviewed revisions and the three kit tasks can work from public ASHA contracts
without reading any source dossier.
