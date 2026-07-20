# Encounter session contract

## Purpose

`RpgEncounterSetup` is the versioned, portable input for creating one
artifact-bound authority session. A consumer compiles or loads a
`CompiledRulesetBundle`, constructs setup-only encounter data, and calls
`RpgAuthoritySession::from_setup`. Rust validates the complete setup before it
creates mutable authority state.

The setup schema is `asha.rpg.encounter.setup@1`. Its `artifactId` must exactly
match the compiled artifact. Checkpoint schema
`asha.rpg.session.checkpoint@2` stores the complete setup and its
`fnv1a64.rpg-encounter-setup.v1` fingerprint. Replay entry schema version 2
binds every before/after boundary to that setup fingerprint, the exact random
source binding, current turn, state revision, and state hash.

## Setup-only data

A setup contains:

- a rectangular board extent and optional stable cell records;
- typed, versioned cell capabilities, with optional artifact definition
  references;
- stable participants, open `RpgTeamId` values, starting positions, artifact
  definition references, and typed initial capability-owner values;
- an explicit initiative order, current actor, round, and turn;
- a random policy/source identity and version binding.

Participant capability entries are tagged by their Rust owner (`vitality`,
`stat`, `defense`, `resource`, or `modifier`). An entry whose owner is not
declared by the artifact fails before session construction. Every participant
requires exactly one bounded vitality value and at least one referenced action
from the materialized artifact.

Setup is not an execution script. It has no action order, target order,
reaction decisions, random results, expected events, or expected outcomes.
Strict decoding rejects additional fields, so those concepts cannot silently
be smuggled into setup version 1.

## Authority and readbacks

An accepted action advances capability state, appends an accepted-event log
entry, and advances to the next living initiative participant in the same
session transaction. A reaction keeps that transaction suspended; the turn
does not advance until the resumed action is accepted. Rejections preserve
state, log, turn, pending reaction, and accepted-random position.

`RpgAuthoritySession::encounter_view` is the renderer-facing structured
readback. It provides:

- board and cell capabilities;
- participant state and artifact definition identities;
- current round, turn, actor, and initiative order;
- only the current participant's referenced actions and legal participant
  target candidates;
- typed extension slots for cell and area choices (empty in the initial entity
  target profile);
- a pending reaction with its exact options;
- accepted DomainEvents in the encounter log;
- in-progress or completed outcome data.

The view is descriptive output. A host still sends an `RpgActionProposal` or
`RpgReactionProposal` tied to the expected state revision. Rust repeats all
legality checks and is the only state/turn mutation authority.

## Random evidence

Normal interactive calls use
`submit_with_random_source_recorded` and
`react_with_random_source_recorded`. The `RpgRandomSource` trait exposes only
an exact source binding and `draw` for the request Rust currently issued. The
session probes its own ordinary authority path to discover each request, checks
the source binding, validates count and die range, and records one terminal raw
command containing the consumed evidence. Hosts do not inspect random plans or
choose branches.

The evidence-bearing `RpgAuthorityCommand` and `RpgReactionCommand` remain
serialized replay vocabulary, but their direct execution methods are crate
internal. Public interactive callers use the proposal-plus-source methods, so
precomputed random arrays are not a second host authority path.

`RpgRollTapeSource` is the portable bounded source for consumers and tests. Its
entries pair an exact `RpgRandomRequest` with values. It diagnoses exhaustion,
request-order mismatch, out-of-range values, excess values within an entry,
and unconsumed entries through `require_exhausted`. Invalid entries are not
removed from the tape. No seeded generator or unspecified default RNG is a
portability claim in version 1.

Replay executes the recorded commands through the ordinary session paths. It
does not call a random source or regenerate evidence.

## Compatibility

Setup, encounter view, checkpoint, replay entry, event, operation, capability,
random policy, and source versions are independent compatibility identities.
Unknown setup/view schema versions, an artifact mismatch, a source binding
mismatch, or a changed setup fingerprint fail closed. Adding a new setup field
requires a new setup schema version because version 1 uses strict decode.

The initial board authority enforces extent, unique occupancy, and impassable
destination cells. It does not claim terrain pathfinding, area target
semantics, movement-cost evaluation, campaign persistence, scripted runners,
AI control, or Rulebench product protocols.
