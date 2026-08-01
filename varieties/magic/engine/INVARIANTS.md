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
- `GameEnded` is the final canonical receipt. If an effect causes a terminal
  loss while a spell or ability is resolving, the resolver first records that
  stack object's final lifecycle receipt (`AbilityResolved` or the applicable
  source-departure receipt), then emits the single `GameEnded`; no later event
  may follow it.
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
- A targeted exile instruction may move only a currently legal battlefield
  creature, records the ordinary `CardMoved { to: Exile }` receipt, and runs
  zone-departure cleanup for the object and any continuous effects involving
  it. It is not lethal damage and cannot be silently substituted with a
  graveyard move.
- A non-token object has exactly one catalog definition; a token has exactly
  one token specification and exists only on the battlefield. An object cannot
  be both, and no nonpermanent card can occupy the battlefield.
- A token's mechanically relevant creature subtypes are typed separately from
  its display name. A token with any such subtype must be a creature; the
  public `Characteristics` view preserves that type-line information and the
  token's static keywords through stack resolution and zone placement.
- Tokens may attack and deal combat or effect damage without a catalog
  definition. Definition-bound attack and damage triggers therefore dispatch
  only for non-token sources; a token's legal declaration and damage batch must
  not fail while attempting a definition lookup.
- Object IDs never alias or regress. Object turn metadata cannot be from a
  future turn, marked damage cannot be negative, and a card outside the
  battlefield retains its owner's controller in the current no-control-change
  rules slice.
- A regeneration shield is private, source-identified replacement state for a
  current battlefield creature. Shield creation records
  `RegenerationShieldCreated { source, target }`; the next modeled destroy or
  lethal-damage event may consume exactly one shield, record
  `RegenerationShieldUsed`, tap that target, clear its marked damage, and
  remove it from combat instead of moving it. Shields never prevent a
  zero-toughness action or a sacrifice and clear when their target leaves the
  battlefield.
- Every catalog definition has identity and a type. A basic land is a land;
  only lands have intrinsic mana colors; creatures have both power and
  toughness; and dredge values are positive.
- A registered basic-land type line names exactly one basic-land definition and
  its singleton intrinsic mana color agrees with the typed land identity.
  `CardView` preserves that identity, and a typed intrinsic activation cannot
  produce a different color. A cast-payment basic-land request must name one
  such typed land and its intrinsic color; it cannot use an untyped land or
  produce a different color while paying a spell cost.
- A stack object has a unique card and a valid controller. Resolving or
  countering it removes it from the stack before it receives its resulting zone
  move.
- A bound triggered ability has a synthetic stack identity that is not a card
  object or zone member. Its private metadata and public stack item must agree
  on source, controller, and ability, carry no spell targets/effects/payment,
  and be removed together when it resolves or its controller leaves. Trigger
  placement follows the source permanent's `CardMoved { to: Battlefield }`
  receipt; resolution emits the draw/effect receipts before its terminal
  `TriggeredAbilityResolved` receipt.
- A stack instruction that depends on colors spent to cast its spell requires
  a nonempty `mana_spent` receipt on that exact stack object. When the visible
  event log contains its `SpellCast`, the immediately preceding
  `SpellManaPaid` receipt must name the same controller, card, and ordered
  colors. Floating mana added or spent after casting cannot alter this
  resolution-time provenance.
- Stack controller, effects, and target-slot count must match the represented
  card definition. Every executable occurrence of a target requirement owns
  one ordered stack slot; the same object may occupy multiple slots when the
  source has multiple independent target occurrences. Tokens and lands cannot
  occupy the stack. A target may later become illegal, but it cannot be
  absent, fabricated, or change enum kind after cast time. `Target::Spell`
  additionally retains and validates the immutable instant-or-sorcery card
  definition even after CR 800.4a removes that object from the live game.
  The invariant validates that immutable target shape separately from dynamic
  target legality; in particular, every stack player target names a seated
  player, although that player may later have lost. At resolution, an all-illegal
  target set emits `SpellCounteredByRules`; if at least one target remains
  legal, the spell resolves and only instructions addressed to the now-illegal
  target slots do nothing. The engine snapshots initial legality in a
  `StackResolutionPlan` before it begins any effect to establish the
  all-targets-illegal boundary. An initially legal slot is rechecked before its
  own instruction, so an earlier instruction that removes a repeated target
  cannot make the complete resolution fail. Each skipped instruction emits its own
  `TargetInstructionSkipped { effect_index, target }` diagnostic receipt. A
  resolving counter effect emits the distinct `SpellCountered` receipt.
- A stack spell target that remains on the stack must be below its source,
  because only already-existing stack objects can be chosen while casting. A
  formerly legal target may have left the stack by resolution, which remains a
  dynamic rules-counter case rather than an invariant failure.
- Every stack controller is living. In this slice a noninstant stack object
  can only be the bottom object, cast by the active player in a main phase;
  any later stack object must be an instant. An unsupported nonpermanent card
  never becomes a successful no-op merely because public fixture state placed
  it on the stack.
- Executable direct-damage spells that use the pre-planeswalker “creature or
  player” targeting scope reject noncreature permanents at cast time and at
  resolution. The broader `Any` requirement is not used to approximate that
  narrower card rule.
- An `Artifact` target requirement names only a current battlefield permanent
  whose resolved characteristics include the `Artifact` card type. The
  `DestroyTargetArtifact` instruction records `CardDestroyed` and the normal
  graveyard/token departure before any later instruction in that spell (such
  as a draw) runs; if the sole artifact target is illegal at resolution, the
  targeted spell is countered by rules and none of its untargeted instructions
  execute.
- A global non-Flying damage effect snapshots only current battlefield
  creatures whose resolved characteristics lack `Flying`; Flying creatures
  are not accidental recipients, and all selected damage is applied before
  the enclosing state-based-action fixed point.
- Every executable damage or life-gain effect has a strictly positive amount.
  The cast preflight rejects malformed nonpositive operations before it debits
  mana, moves a card, creates a stack object, or emits an accepted-action
  receipt; negative power/toughness modifiers remain valid layer-seven data.

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
- `GameView` never exposes an opponent's hand or library, nor does it expose a
  departed player as an opponent life-total or battlefield target. It projects
  only the controller-owned, mana-value-matching library cards for each
  transmute card in that controller's hand, allowing an honest search decision
  without granting general hidden-library access.
- While a draw replacement is pending, `GameView` projects only the deciding
  player's legal owned-graveyard dredge candidates. A policy can take the
  normal draw or choose one of those candidates; it cannot name a hidden or
  unpayable replacement.
- A legal non-pass action resets the pass sequence. Once every seated player
  still in the game has passed in sequence, the top stack object resolves; if
  the stack is empty, the game advances exactly one step. Stack resolution is
  last-in, first-out. This includes the public `add_mana_from_action` fixture
  seam as well as intrinsic mana abilities: a positive mana action after an
  opponent's pass restores that opponent's response window rather than
  advancing a step. A zero-amount public mana action is rejected atomically;
  it cannot manufacture a priority-consuming non-action or a `ManaAdded(0)`
  receipt.
- The final priority pass and the resulting stack-resolution/step transition
  are one all-or-error state-machine transaction. If resolution reports an
  error, the authoritative stack order, zones, objects, effects, pass count,
  priority, turn/step fields, and canonical event log exactly match their
  state before that final pass. A successful stack object has exactly one
  terminal outcome: `SpellCounteredByRules` followed by its destination move,
  or ordered effect receipts followed by `SpellResolved` and its destination
  move; no partial outcome may survive. The invariant audit requires every
  terminal stack receipt to be immediately followed by its own destination
  move (`Graveyard` for a counter, `Battlefield` or `Graveyard` for a normal
  resolution), rejects a second terminal receipt for the same visible cast,
  and requires every visible unterminated `SpellCast` to remain on the live
  stack. An owner leaving the game is the explicit exceptional terminal path:
  `ObjectLeftGame` removes that owner's stack object under CR 800.4a.
- If the active player leaves while stack work remains, their turn continues
  without a living active player as required by CR 800.4i. The engine retains
  that departed seat only as current-turn identity, gives priority to the next
  survivor, and begins the next turn only after the stack has emptied. Event
  receipt audits must treat that departed owner's `ObjectLeftGame` as the
  terminal path for historical stack receipts: a later transition may not
  dereference an object that CR 800.4a correctly removed.
- The expansion-neutral definition-bound mana-ability catalog contains only
  permanent definitions that exist in the game catalog. Every ability identity
  is nonempty and unique within its definition; its output amount is positive;
  a selectable output has at least one color; and an optional life payment is
  positive. Activation requires the source on the battlefield and controlled by
  the priority holder. A tap-cost creature ability additionally observes
  summoning sickness unless its current characteristics include Haste. Fixed
  outputs reject a supplied color choice and selectable outputs require one
  listed color. A successful bound mana ability never enters the stack, emits
  `BoundManaAbilityActivated` then any `ManaAbilityLifePaid` and `ManaAdded`
  receipts, resets consecutive passes, and leaves priority with its activator.
  A positive optional controller-damage result is distinct from a life-payment
  cost: it is legal even when it causes loss, follows the mana receipt as a
  source-aware `DamageDealtToPlayer`, and then state-based actions determine
  elimination. Every rejection—including a full mana pool, invalid choice,
  untapped-source requirement, unaffordable life payment, or terminal game—is
  atomic.
- Attacker declaration rejects a creature that entered under its controller's
  control this turn unless it had Haste when declared. The combat state records
  that declaration-time Haste provenance; it must name only declared attackers,
  and every same-turn attacker still on the battlefield must retain that
  provenance. `CardView::can_attack` applies the same exception so policies do
  not hide a legal Haste attack. Non-Haste attack and tap-cost rejection are
  atomic: they do not tap the source, create combat state, add mana, or write
  accepted-action receipts.
- A regenerated blocker remains associated with the attacker it blocked, so a
  nontrample attacker remains blocked, but the blocker is marked
  removed-from-combat and neither assigns nor receives combat damage. That
  provenance is an invariant-checked subset of the declared blockers.
- Per-mana-kind floating amounts are bounded `u8` values, but generic-cost
  payment sums all five colors plus colorless in a widened `u16` total. The compatibility
  `ManaPool::total` policy view is saturated at `u8::MAX`; it is never used to
  decide whether a generic payment is affordable.
- Colorless is a mana kind rather than a card color: it can pay generic and
  explicit colorless costs but cannot appear in a card's color set, be chosen
  by a "choose a color" mana ability, satisfy a colored or hybrid symbol, or
  satisfy a Convoke color contribution. Token specifications are checked when
  a token is created (and by the invariant audit), while layer-five color
  changes are rejected before installation. A typed nonbasic land may produce
  it through the same bound mana-ability receipts as colored mana.
- Colored symbol repetitions are also counted in a widened `u16` requirement
  before any pool slot is debited. A cost above a bounded color slot's
  representable capacity rejects atomically; it cannot saturate into a cheaper
  payable cost.
- Each hybrid symbol is one required mana drawn from either of its two declared
  colors, never two mandatory colored symbols or an untracked generic payment.
  Payment uses capacity-aware matching rather than greedy symbol order, so a
  legal allocation cannot be rejected merely because an earlier hybrid choice
  consumed a shared color. Hybrid symbols have mana value one and are eligible
  for a matching-color convoke contribution. A failed hybrid allocation leaves
  the entire pool, card zone, stack, and event history unchanged.
- Every explicit discard cost is represented by a policy-selected,
  controller-owned hand object. The engine rejects missing, duplicated,
  wrong-zone, or opponent-owned selections before paying any other cost; a
  successful discard emits `DiscardedAsAbilityCost` immediately followed by
  that card's graveyard move and before `AbilityActivated`. The invariant audit
  rejects orphaned discard receipts, receipts for abilities without a discard
  binding, and any non-graveyard destination.
- Every activated-ability sacrifice cost is represented by explicit, distinct
  policy-selected controlled battlefield permanents in binding order: source
  sacrifices first, then the configured number of creatures, then lands. The
  engine validates cardinality, control, current zone, and each required type
  before mana, zone, tap, stack, or receipt changes. Each successful selection
  records `SacrificedAsAbilityCost` immediately followed by its graveyard move
  (or token-ceases receipt), before `AbilityActivated`; a rejected selection is
  an atomic no-op with no mana debit or cost receipt.
- An activated ability that requires additional creature taps receives exactly
  that many explicit, distinct, controlled, untapped non-source creature
  selections from the policy. These are cost taps rather than tap-symbol
  activations of the selected creatures, so their summoning sickness is not a
  restriction. All selections are validated before mana, zones, taps, stack,
  or event history change. Every successful selected tap emits
  `AdditionalCreatureTappedAsAbilityCost` immediately before its matching
  `AbilityActivated`; the invariant audit rejects orphaned, duplicate,
  wrong-source, or wrong-cardinality receipts.
- A composite one-target modifier retains exactly one creature target slot on
  its stack object even when it installs both a layer-seven power/toughness
  change and a layer-six keyword grant. Both effects use that same legal
  target, expire together at end of turn, and emit their ordinary continuous-
  effect receipts before the enclosing ability resolves.
- A conditional land-destruction instruction snapshots the target's basicness
  while the target is legal, before its destroy transition. It may untap only
  its still-battlefield source after a nonbasic target; that state change emits
  exactly one `PermanentUntapped` receipt before the enclosing stack item
  resolves. Automatic untap-step transitions never impersonate this receipt.
- A target-tap effect rechecks its creature target at stack resolution. It may
  leave an already tapped legal target unchanged, but an untapped-to-tapped
  transition emits exactly one `PermanentTapped` receipt before the stack item
  receives its terminal resolution receipt; ability-cost taps never use that
  effect receipt.
- Trigger bindings declare their condition, optional mana cost, target
  requirements, and effects as one checked shape. Every attack trigger stacks
  before any optional mana cost is evaluated, so its controller receives the
  post-declaration priority window and can activate mana abilities. An optional
  trigger cost is paid immediately before its effects resolve; the engine's
  current deterministic compatibility policy pays it when affordable, and an
  unpaid optional trigger records no effect receipts. An attack trigger retains
  its selected target through resolution; received-damage triggers capture
  positive damage before SBAs; `AnotherCreatureDies` observers are captured
  while both objects still have battlefield provenance, so a simultaneous
  creature death and a token death cannot erase a surviving observer's
  trigger. Dies triggers retain the historical source object after a graveyard move and
  materialize every declared target slot before stacking; a legal selected
  target cannot be dropped or replaced by an empty target vector.
  A `LifeGained` trigger is captured only from a source controlled by the
  player named by the positive `LifeGained` receipt while that source is on
  the battlefield, then stacked only after the enclosing
  spell or ability reaches its terminal receipt. Its optional mana cost is
  paid at trigger resolution, not while the trigger is stacked; an unpaid
  optional cost resolves with no damage, while a paid trigger selects a legal
  creature-or-player target at resolution and deals its fixed amount.
  ETB triggers with targets retain one deterministic legal target per declared
  occurrence; a targeted opponent trigger cannot silently fan out to every
  opponent. A two-target redirection activation must resolve both target
  instructions before installing its replacement shield, and an incomplete
  or countered activation cannot leak a pending half-effect. If the protected
  target becomes illegal before resolution, the paired destination instruction
  is a no-op rather than a resolver error or leaked final-pass transition.
  Every materialized dynamic effect is checked against its binding before it
  can resolve, and no pending attack, damage, life-gain, or dies trigger may survive its
  enclosing transition.
- A resolving all-player discard effect selects at most one controller-owned
  hand card per living player in deterministic hand order. Every
  `CardDiscarded` receipt is immediately followed by that exact card's
  `CardMoved { to: Graveyard }` receipt; activation discard costs remain
  separately represented by `DiscardedAsAbilityCost`.
- A resolving controller-creature sacrifice effect chooses a live controlled
  creature deterministically, preferring a creature other than its source and
  falling back to the source only when it remains a creature permanent. Its
  `SacrificedByEffect` receipt is immediately followed by either that card's
  graveyard move or a token's `TokenCeasedToExist` receipt; no legal absence of
  a creature may roll back an otherwise valid trigger resolution.
- Every public, intrinsic, or definition-bound mana producer preflights this
  bounded pool before it changes a source, pass state, pool, or event log. A
  capacity rejection is atomic and cannot emit a `ManaAdded` receipt for mana
  the pool did not receive.
- A `CastRequest` may name an ordered list of definition-bound mana abilities
  and explicitly selected typed basic-land intrinsic abilities for its own
  payment context. Such an ability is neither a priority action nor a stack
  object: it is admitted only inside that one enclosing cast transaction. The
  cast preflights spell legality, executes the listed activations in request
  order, then pays the remaining spell cost and creates exactly one spell stack
  object. Any later activation or final-payment rejection restores every
  earlier source tap, life/mana change, zone, stack, pass-state field, and
  receipt. A bound contextual activation emits
  `CastPaymentManaAbilityActivated`, immediately followed by its matching
  bound-ability receipt and its required mana-output receipts. A basic-land
  contextual activation emits
  `CastPaymentBasicLandManaAbilityActivated`, immediately followed by its
  matching intrinsic `ManaAbilityActivated` and one matching
  `ManaAdded { amount: 1 }` receipt. Mixed bound/basic payment entries remain
  in the caller's declared order, all name the same enclosing spell, and close
  with that spell's `SpellCast` before any priority pass. Paid fixed bundles
  additionally require their cost receipt, optional life-cost receipt, and
  every fixed mana-output receipt in declared order.
- `Game::cast_spell_with_mana_spend` is the explicit path for selecting colors
  for every remaining generic and hybrid symbol after Convoke. The selection
  has exact residual arity; each color must be available and each hybrid choice
  must match its symbol. The atomic transition emits `SpellManaPaid`
  immediately before `SpellCast` and copies the same ordered colors onto its
  stack object. A card that inspects paid colors rejects legacy deterministic
  `cast_spell`, so engine-selected generic draining never masquerades as the
  controller's choice.
- An expansion may bind an explicit additional spell cost to a nonland
  definition. Its `CastRequest` selection follows ordinary effect targets but
  never enters the resulting stack object's target slots. A bound controlled-
  creature sacrifice selection must name one distinct creature the caster
  controls on the battlefield. Its `SacrificedAsAdditionalSpellCost` receipt
  is immediately followed by the selected permanent's graveyard move (or a
  token-ceases receipt) and precedes the same spell's `SpellCast` before any
  priority pass. Target, selection, mana-activation, and final-payment
  rejections are one atomic cast boundary: no sacrifice zone move, stack
  object, tapped source, mana debit, or cost receipt may remain after failure.
- Player life totals use a wide signed `i64` representation, distinct from
  the `i16` effect and damage amounts. Life changes widen their amount before
  arithmetic, so an ordinary legal life-gain effect at the former `i16`
  boundary completes its ordered receipts and resolution lifecycle.
- Printed creature power/toughness and individual effect modifiers remain
  bounded `i16` values, while derived layer-seven characteristics, marked
  permanent damage, and damage-event amounts use signed `i32` values. Repeated
  representable modifiers or damage effects therefore cannot wrap or panic
  after a partial lifecycle receipt; SBAs see the complete resolved result.
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
- Once the game has ended, gameplay actions, public draw replacements, public
  continuous-effect installation, and the setup-to-live `begin_game`
  transition are rejected atomically, without changing state or emitting
  accepted-action events. Setup hooks remain deliberately separate from
  gameplay methods.
- Setup-only provenance helpers, including battlefield-entry and tapped-state
  shaping, reject a live game. They cannot erase a paid tap cost or otherwise
  rewrite gameplay state without an engine action and its canonical receipt.
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
  At `Upkeep`, every beginning-of-upkeep trigger controlled by the active
  player is stacked after that marker and before the first priority window;
  its life-loss receipts are never damage receipts and resolve only through
  the ordinary stack path.
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
  rejected without changing a zone or canonical receipt. An authored fixture
  that deliberately reaches a pending Draw step before `begin_game` likewise
  consumes that marker when it resolves its ordinary draw, rather than leaving
  a stale mandatory decision to block the next priority window.

## Combat

- Combat state exists exactly during declare attackers, declare blockers,
  first-strike combat damage when present, and normal combat damage. It is
  initialized at declare attackers and removed before end of combat; a combat
  step cannot be missing its state.
- Before the required attacker or blocker declaration, the step's recorded
  priority holder may perform only that declaration. Every ordinary priority
  action—including casts, mana abilities, passes, and weakness reports—is
  rejected until the turn-based declaration is complete.
- Only the active player declares attackers. Each attacker is unique, on that
  player's battlefield, an eligible untapped creature, and is tapped when it
  attacks.
- Combat-restriction keywords keep their scope: `CannotAttackOrBlock` rejects
  both attacker and blocker declarations, while `CannotBlock` rejects only a
  blocker declaration and never prevents that creature from attacking. A
  temporary effect must not substitute the broader keyword for a block-only
  restriction.
- A controller-wide Saproling blocker restriction is checked both against a
  submitted blocker and while determining whether a must-block attacker has a
  legal unassigned blocker. It is active only while that controller retains a
  battlefield source with the restriction keyword; a non-Saproling creature
  is never rejected by this rule.
- The next seated defending player is fixed when attackers are declared; only
  that player declares blockers. Each blocker is a unique untapped creature
  they control; every assigned attacker was declared; and each attacker may
  retain an ordered list of distinct blockers. A creature declared with
  Flying accepts only a blocker that had Flying or Reach at blocker
  declaration; a creature declared with Fear accepts only a black or artifact
  blocker; and a creature declared with the RAV black-only evasion restriction
  accepts only a black blocker. A creature declared with `Unblockable` accepts
  no blocker. The combat state records each
  declaration-time qualification so a later characteristic change cannot
  rewrite its legal history. If that defender leaves the game, the declared
  attackers are removed from combat rather than being retargeted to another
  surviving seat.
- Declare-blockers cannot begin without an attacker declaration, and combat
  damage cannot begin without both declarations. A participant may leave after
  declaration, so later combat bookkeeping preserves the declaration without
  dereferencing a vanished token.
- `unblockable_attackers` is declaration provenance only: it is a subset of
  the uniquely declared attackers and no blocker map entry may name one of
  those attackers. A rejected block writes no `BlockersDeclared` receipt.
- A target constrained to an attacking-or-blocking creature has the normal
  permanent target shape, but must additionally identify a battlefield
  creature present in the active combat's attacker or blocker provenance at
  cast and resolution time. A noncombat creature is rejected atomically rather
  than being silently accepted by a generic creature-target effect.
- Combat damage occurs only after attacker and blocker declarations. It is
  recorded as player/permanent damage events, then state-based actions run.
  A creature with zero or negative power assigns no combat damage and emits no
  damage event; negative power can never increase life or remove marked damage.
  Alternative combat restrictions and other unsupported combat rules must be
  reported as capability gaps rather than approximated.
- `trampling_attackers` is declaration provenance only: it is a subset of the
  uniquely declared attackers and cannot exist before their declaration. At
  combat damage, Trample is evaluated from the attacker's live characteristics,
  so a later supported characteristic change can affect assignment without
  corrupting the historical declaration. The bounded substrate retains every
  blocker in declaration order. A positive-power live Trample attacker assigns
  each live blocker's remaining lethal damage (after marked damage) in that
  order, then its positive excess exactly once to the fixed defender; if all
  blockers have left combat, all its positive assignment goes to that defender.
  Explicit damage-order choice, deathtouch, and prevention/replacement
  interactions remain explicit capability gaps rather than approximated damage
  assignment.
- When an attacking or blocking creature has first strike at the damage-step
  boundary, a dedicated `FirstStrikeCombatDamage` step precedes normal combat
  damage. Its recorded source set is a subset of the declared combatants and
  those sources cannot assign again in normal combat damage. State-based
  actions run after the first-strike batch, so a lethal blocker does not remain
  to assign later normal damage. If no participant has first strike, the extra
  step is absent rather than an empty priority window.
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
  auditable event. A public continuous-effect installation is such a relevant
  change: it creates its effect receipt, then reaches the SBA fixed point
  before returning; a spell that installs an effect waits until the entire
  spell has resolved before that same SBA check.
- Radiance selection is evaluated at resolution from the legal creature target:
  it includes that target and every battlefield creature sharing at least one
  of its colors, regardless of controller. Each effect declares whether it
  changes tapped state; a power/toughness-only radiance effect cannot untap a
  selected permanent as an incidental side effect. A radiance damage effect
  snapshots that selection once, emits one damage receipt per selected
  creature, and applies all of those marks before its post-resolution SBA
  check.
- A Radiance keyword effect uses the same one-time target-color snapshot and
  installs one layer-6 continuous effect per selected creature. It may grant
  Haste without changing power, toughness, or tapped state; an unrelated
  creature receives no effect receipt. The target must remain a legal creature
  through resolution, while each selected shared-color creature is independently
  checked before installation.
- A Radiance `CannotBlock` effect installs one layer-six restriction for the
  target and each shared-color creature, never for unrelated colors. The
  restriction expires at end of turn and does not alter attacker legality.
- An unconditional spell draw is performed only after all earlier spell
  instructions resolve successfully, moves exactly one library card to hand
  (or performs the normal empty-library loss), and records the ordinary card
  movement receipt before the spell itself leaves the stack.
- A target-free global creature-and-player damage effect snapshots every
  battlefield creature in stable object-id order, emits one permanent-damage
  receipt for each, then emits one player-damage receipt for each surviving
  player in seat order. Noncreatures are excluded. All marks and life changes
  complete before the post-resolution SBA fixed point, so one lethal creature
  cannot prevent another selected creature or player from receiving its
  receipt.
- A combat-count damage effect computes the resolving controller's currently
  battlefield attacking creatures at resolution. It emits no damage receipt
  when that count is zero, and its count is widened and preflighted before a
  cast can commit an unrepresentable event amount. A controller-wide temporary
  power/toughness effect snapshots every creature the controller currently
  controls, installs one layer-seven effect for each recipient, then runs SBAs
  only after the spell's complete recipient set is processed.
- A stack effect that adds mana to its controller is validated as a positive,
  capacity-checked operation. For Seismic Spike, the targeted land leaves the
  battlefield first; only then does the same resolution append one
  `ManaAdded` receipt for exactly two red mana. If the bounded mana pool cannot
  hold the addition, the enclosing transition rejects atomically rather than
  claiming mana that is not present.
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

The report is observational: it does not resolve a spell, change zones, or
make an unsupported play legal. Like every accepted non-pass action, it resets
the pass sequence and thereby preserves the other players' response windows;
it does not itself pass priority or advance a step. `code` should be a stable,
machine-groupable capability identifier (for example
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
