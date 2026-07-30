# Magic engine invariants

This document is the public integrity contract for the Rust Magic engine. It
describes facts that `Game::validate_invariants()` must reject when violated and
the rules boundaries that scenario and policy runners rely on. It is a
development contract for the implemented card slice, not a statement of full
Oracle Magic rules coverage.

## State ownership and zones

- A game has a fixed, at-least-two seat count; `PlayerId(n)` is the player at
  seat `n`, and both active player and priority holder name existing, living
  seats while the game continues. A fixture cannot add, remove, or reorder a
  seat after game construction.
- A game ends only when zero or one players remain. Its terminal transition
  emits exactly one `GameEnded { winner }` record (where `winner` is `None`
  for a draw). In a continuing multiplayer game, an eliminated player is
  skipped by turn order and may not hold priority or submit an action.
- On a player-loss transition, objects owned by that player leave this game
  and emit `ObjectLeftGame`; a non-owned object under that player's control is
  exiled to its owner. No departed player's object may later appear in a zone,
  on the stack, in combat, or in an effect.
- Every object exists in exactly one player zone or exactly once as the card of
  a stack object. An object cannot be in two zones, or on both the stack and in
  a zone.
- A card in a library, hand, graveyard, or exile is in its owner's zone. A
  permanent is in its controller's battlefield. Every non-token definition is
  present in the game catalog.
- A non-token object has exactly one catalog definition; a token has exactly
  one token specification and exists only on the battlefield. An object cannot
  be both, and no nonpermanent card can occupy the battlefield.
- Object IDs never alias or regress. Object turn metadata cannot be from a
  future turn, marked damage cannot be negative, and a card outside the
  battlefield retains its owner's controller in the current no-control-change
  rules slice.
- Every catalog definition has identity and a type. A basic land is a land;
  only lands have intrinsic mana colors; creatures have both power and
  toughness; and dredge values are positive.
- A stack object has a unique card and a valid controller. Resolving or
  countering it removes it from the stack before it receives its resulting zone
  move.
- Stack controller, effects, and target count must match the represented card
  definition. Tokens and lands cannot occupy the stack. A target may later
  become illegal, but it cannot be absent or fabricated at cast time. At
  resolution, an all-illegal target set emits `SpellCounteredByRules`; a
  resolving counter effect emits the distinct `SpellCountered` receipt.

## Priority, stack, and turns

- Only the current priority holder can cast, transmute, play a land, activate
  mana, pass, or submit an engine-weakness report. A rejected proposal has no
  accepted `PolicyMoveSubmitted` event. A successful policy-submitted
  transmute has the same audited receipt boundary as every other policy move.
  A pending draw replacement is instead a mandatory active-player decision in
  the Draw step: it is not priority, it must resolve before any priority
  action (including a spell, mana ability, pass, or weakness report), and its
  submitted `PolicyAction::Draw` receipt follows the resulting draw or dredge
  events.
- `GameView` never exposes an opponent's hand or library. It projects only the
  controller-owned, mana-value-matching library cards for each transmute card
  in that controller's hand, allowing an honest search decision without
  granting general hidden-library access.
- While a draw replacement is pending, `GameView` projects only the deciding
  player's legal owned-graveyard dredge candidates. A policy can take the
  normal draw or choose one of those candidates; it cannot name a hidden or
  unpayable replacement.
- A legal non-pass action resets the pass sequence. Once every seated player
  still in the game has passed in sequence, the top stack object resolves; if
  the stack is empty, the game advances exactly one step. Stack resolution is
  last-in, first-out.
- Only instants in the implemented spell slice may be cast outside their
  controller's main phase or while the stack is nonempty. Sorceries and
  permanent spells observe sorcery timing.
- Casting a spell resets the pass sequence and leaves priority with its caster.
  An opponent receives a response window only after the caster passes.
- The supported atomic Transmute activation likewise resets the pass sequence
  and leaves priority with its controller; this slice does not model its
  activated ability as a separately stack-resolving object.
- Turn numbers are never zero, and the consecutive-pass counter is always
  below the number of surviving players outside its atomic resolution/step
  transition. A draw-replacement marker can exist only for the active player
  at the Draw-step decision boundary; it cannot outlive that boundary or point
  at an eliminated player.
- Once the game has ended, gameplay actions and public draw replacements are
  rejected without changing zones or emitting accepted-action events. Setup
  hooks remain deliberately separate from gameplay methods.
- Mana pools clear on each step change. Land plays are limited to one per
  player turn and only occur during that player's main phase with an empty
  stack. The active player's land-play count resets at that player's untap
  step.
- The engine follows the fixed `Step` order from untap through cleanup. At a
  new untap step, the active player rotates and the turn increments.
  `begin_game` begins a prepared deck game at turn-one Untap, automatically
  reaches Upkeep, and skips only that player's first Draw. Untap and ordinary
  Cleanup are automatic: they are never stable priority-bearing states and a
  policy action in either is rejected.
- `StepBegan` is emitted before any automatic work in that step (untapping,
  drawing, combat damage, cleanup, state-based actions, or terminal loss).
  Cleanup emits one `ContinuousEffectExpired` per expiring effect; token SBAs
  emit `TokenCeasedToExist`; and every transmute shuffle emits
  `LibraryShuffled`. These lifecycle receipts let a runner audit transitions
  instead of inferring invisible mutations from the final state.
- Opening-hand drawing preflights the requested count. It is an all-or-error
  setup transaction available only before the game begins and only into an
  empty hand: a short library cannot partially draw cards and then claim a
  larger `OpeningHandDrawn` event. Deck loading is likewise pregame-only, so
  setup events and hidden cards cannot be injected into a live turn.
- Once the game has begun, the public direct-draw primitive is legal only for
  the active player's pending Draw-step decision. It resolves that marker
  atomically; an arbitrary Upkeep, main-phase, combat, or opponent draw is
  rejected without changing a zone or canonical receipt.

## Combat

- Combat state exists exactly during declare attackers, declare blockers, and
  combat damage. It is initialized at declare attackers and removed before end
  of combat; a combat step cannot be missing its state.
- Only the active player declares attackers. Each attacker is unique, on that
  player's battlefield, an eligible untapped creature, and is tapped when it
  attacks.
- Only the next seated defending player declares blockers. Each blocker is a
  unique untapped creature they control; every assigned attacker was declared;
  and the current substrate allows at most one blocker per attacker.
- Declare-blockers cannot begin without an attacker declaration, and combat
  damage cannot begin without both declarations. A participant may leave after
  declaration, so later combat bookkeeping preserves the declaration without
  dereferencing a vanished token.
- Combat damage occurs only after attacker and blocker declarations. It is
  recorded as player/permanent damage events, then state-based actions run.
  A creature with zero or negative power assigns no combat damage and emits no
  damage event; negative power can never increase life or remove marked damage.
  Multi-block assignment, alternative combat restrictions, and other
  unsupported combat rules must be reported as capability gaps rather than
  approximated.
- A declared participant may leave the battlefield after damage. Historical
  combat bookkeeping may therefore retain a nontoken object in another zone or
  a token identifier that no longer names an object until combat ends; neither
  case corrupts zone ownership. `GameView::combat_attackers` exposes only
  currently battlefield attackers, so a historical token identifier cannot
  make policy observation fail.

## Effects and state-based actions

- Continuous effects are applied in the implemented layer order (4--7), then
  timestamp order within a layer. End-of-turn effects expire during cleanup;
  marked damage clears there. An effect removed because its source or target
  leaves the battlefield emits an explicit expiration lifecycle receipt.
- Every continuous effect names extant source and target objects, has a unique
  positive monotonic timestamp, and has a valid duration. A permanent-duration effect
  cannot outlive its battlefield source; an end-of-turn effect belongs to the
  current turn only.
- State-based actions run to a fixed point after relevant changes. The current
  slice moves creatures with zero-or-less toughness or lethal marked damage,
  and marks players with zero-or-less life as lost. Each action emits an
  auditable event.
- Card and mechanic implementations may only claim the semantic fragments
  listed in their `supported_rules`. Unsupported text is not silently inferred.
- A nonpermanent card with no supported cast effect is rejected at cast time;
  callers must submit an explicit capability report instead of receiving a
  successful no-op resolution. Transmute observes sorcery timing. Dredge is
  accepted only as the replacement selected by `draw_card` or a submitted
  `PolicyAction::Draw` for a pending draw.

## Deck construction

- Nonbasic copy limits aggregate mainboard and sideboard entries, including
  duplicate entries. Basic lands remain exempt. Mainboard minimum and sideboard
  maximum constraints are checked independently.

## Checking and reporting a weakness

Runners should call `Game::validate_invariants()` after setup and after every
accepted policy action. An invariant failure is an engine defect or corrupted
test setup: preserve the canonical event log and fail the run rather than
continue from an ambiguous state. The engine seals each event it emits; direct
append, removal, rewrite, or reorder of the public `event_log` is a corruption
and is rejected by this audit. `clear_event_log()` is the explicit authorized
reset and resets that seal alongside the public vector; it is intended before
a measured run, not after a terminal result whose required `GameEnded` receipt
must remain present.

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

## Enforcement boundary

The invariant audit validates every public engine transition and each state
that the public runners produce. Several `Game` fields remain public to permit
compact, authored fixture construction in this initial substrate. A caller can
therefore deliberately mutate those fields outside a transition and then call
the audit; the audit detects invalid *state shapes* and any event-log edit, but
cannot prove that every valid-looking zone/priority shape arose through the
transition machine. This API-encapsulation gap is tracked in the public bug
ledger and is not presented as complete protection against hostile external
mutation.
