# Magic engine invariants

This document is the public integrity contract for the Rust Magic engine. It
describes facts that `Game::validate_invariants()` must reject when violated and
the rules boundaries that scenario and policy runners rely on. It is a
development contract for the implemented card slice, not a statement of full
Oracle Magic rules coverage.

## State ownership and zones

- A game has at least two seated players; `PlayerId(n)` is the player at seat
  `n`, and both active player and priority holder name existing seats.
- Every object exists in exactly one player zone or exactly once as the card of
  a stack object. An object cannot be in two zones, or on both the stack and in
  a zone.
- A card in a library, hand, graveyard, or exile is in its owner's zone. A
  permanent is in its controller's battlefield. Every non-token definition is
  present in the game catalog.
- A stack object has a unique card and a valid controller. Resolving or
  countering it removes it from the stack before it receives its resulting zone
  move.

## Priority, stack, and turns

- Only the current priority holder can cast, play a land, activate mana, pass,
  or submit an engine-weakness report. A rejected proposal has no accepted
  `PolicyMoveSubmitted` event.
- A legal non-pass action resets the pass sequence. Once every seated player
  has passed in sequence, the top stack object resolves; if the stack is empty,
  the game advances exactly one step. Stack resolution is last-in, first-out.
- Mana pools clear on each step change. Land plays are limited to one per
  player turn and only occur during that player's main phase with an empty
  stack. The active player's land-play count resets at that player's untap
  step.
- The engine follows the fixed `Step` order from untap through cleanup. At a
  new untap step, the active player rotates and the turn increments. The first
  player skips the first-turn draw, as implemented by this two-or-more-player
  substrate.

## Combat

- Combat state exists only during declare attackers, declare blockers, combat
  damage, or end of combat. It is initialized at declare attackers and removed
  at end of combat.
- Only the active player declares attackers. Each attacker is unique, on that
  player's battlefield, an eligible untapped creature, and is tapped when it
  attacks.
- Only the next seated defending player declares blockers. Each blocker is a
  unique untapped creature they control; every assigned attacker was declared;
  and the current substrate allows at most one blocker per attacker.
- Combat damage occurs only after attacker and blocker declarations. It is
  recorded as player/permanent damage events, then state-based actions run.
  Multi-block assignment, alternative combat restrictions, and other
  unsupported combat rules must be reported as capability gaps rather than
  approximated.

## Effects and state-based actions

- Continuous effects are applied in the implemented layer order (4--7), then
  timestamp order within a layer. End-of-turn effects expire during cleanup;
  marked damage clears there.
- State-based actions run to a fixed point after relevant changes. The current
  slice moves creatures with zero-or-less toughness or lethal marked damage,
  and marks players with zero-or-less life as lost. Each action emits an
  auditable event.
- Card and mechanic implementations may only claim the semantic fragments
  listed in their `supported_rules`. Unsupported text is not silently inferred.

## Checking and reporting a weakness

Runners should call `Game::validate_invariants()` after setup and after every
accepted policy action. An invariant failure is an engine defect or corrupted
test setup: preserve the canonical event log and fail the run rather than
continue from an ambiguous state.

`PolicyAction::ReportEngineWeakness { code, detail }` is the deliberate path
for an interaction that cannot be represented by the implemented rules slice.
The policy must hold priority. Acceptance appends, in order:

1. `GameEvent::EngineWeaknessRevealed { player, code, detail }`;
2. `GameEvent::PolicyMoveSubmitted { kind: ReportEngineWeakness, .. }`.

The report is observational: it does not resolve a spell, change zones, pass
priority, advance a step, or make an unsupported play legal. `code` should be
a stable, machine-groupable capability identifier (for example
`combat.multiple_blockers`); `detail` should identify the attempted interaction
and the observable state needed to reproduce it, without hidden benchmark data
or card text. A run that reveals a weakness is a surfaced coverage result, not
a successful simulation of that interaction.
