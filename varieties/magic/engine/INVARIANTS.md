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
- Every player zone, including the battlefield vector, is ownership-indexed.
  A card in a library, hand, battlefield, graveyard, or exile is therefore in
  its owner's vector; `Game::controller_of` derives battlefield control from
  active layer-two effects. A control effect never masquerades as a zone move,
  and every non-token definition is present in the game catalog.
- A targeted exile instruction may move only a currently legal battlefield
  creature, records the ordinary `CardMoved { to: Exile }` receipt, and runs
  zone-departure cleanup for the object and any continuous effects involving
  it. It is not lethal damage and cannot be silently substituted with a
  graveyard move.
- Every pending linked-exile group has one positive, unique group identity,
  exactly one primary-creature member, duplicate-free attached-Aura members,
  and one delayed return action. Each member records the exact positive
  incarnation it had after the ordinary exile move. A delayed return moves
  only members that are still in `Exile` with that exact incarnation; a card
  that independently changed zones is never revived or reattached merely
  because its stable `ObjectId` matches. The primary creature returns first;
  linked Auras follow in stable object-id order and may attach only through a
  new ordinary legality-checked attachment to that returned incarnation.
- A delayed linked-exile action has a nonzero unique action identity, typed
  `EndStep` timing, and a due turn no earlier than the current turn. It is
  consumed exactly once with its group, removing both from suspended state.
  `DelayedActionScheduled` names the exact exiled members, while
  `DelayedActionConsumed` records only the duplicate-free subset that was
  still eligible to return. Those receipts are replay-audited against their
  schedule; ordinary `CardMoved` and incarnation receipts remain zone truth.
- An attachment binding has one typed source kind (`Aura` or `Equipment`),
  one permanent-only target restriction, and a nonempty duplicate-free set of
  linked continuous changes. Aura bindings require an Enchantment source and
  matching permanent-spell attachment effect; Equipment bindings require an
  Artifact source and matching activated attachment effect. A live Aura has
  exactly one legal attached target; if either endpoint or legality
  disappears, the ordinary SBA moves it to its graveyard and expires every
  linked effect. Equipment may be unattached; its attach activation may move
  it only between legal endpoint incarnations, expires only its prior linked
  effects, and leaves it on the battlefield when an endpoint becomes illegal.
  Every live attachment has exactly one permanent effect per declared change.
  `AuraAttached` and `EquipmentAttached` immediately follow the final linked
  effect receipt; `AttachmentDetached` names a previously attached Equipment.
  A suppression change rejects only nonmana activated abilities before any
  cost or receipt; mana abilities remain legal.
- A non-token object has exactly one catalog definition; a token has exactly
  one token specification and exists only on the battlefield. An object cannot
  be both, and no nonpermanent card can occupy the battlefield.
- A token's mechanically relevant creature subtypes are typed separately from
  its display name. A token with any such subtype must be a creature; the
  public `Characteristics` view preserves that type-line information and the
  token's static keywords through stack resolution and zone placement.
- A target-opponent token ETB trigger records one synthetic stack object and
  selects exactly one legal player target before its priority window. On
  resolution, every `TokenCreated` recipient and typed token specification
  must match that object; it cannot fan out to every opponent or substitute a
  source-controller token after the original target remains legal.
- Tokens may attack and deal combat or effect damage without a catalog
  definition. Definition-bound attack and damage triggers therefore dispatch
  only for non-token sources; a token's legal declaration and damage batch must
  not fail while attempting a definition lookup.
- Object IDs never alias or regress. Every card object has a positive,
  monotonic incarnation that advances on every actual zone transition,
  including library-to-hand draw, hand-to-stack cast, stack-to-terminal-zone,
  and battlefield departure/return. A physical card retaining its public
  `ObjectId` after leaving and re-entering is a new rules object. Every
  advance has exactly one public `ObjectIncarnationAdvanced` receipt directly
  after the associated `CardMoved` or `SpellCast` receipt; receipt values are
  strictly increasing per object. Object turn metadata cannot be from a
  future turn, marked damage cannot be negative, and a card object's base
  controller always remains its owner. Only `Game::controller_of` may expose a
  different live battlefield controller.
- A regeneration shield is private, source-identified replacement state for a
  current battlefield creature. Shield creation records
  `RegenerationShieldCreated { source, target }`; the next modeled destroy or
  lethal-damage event may consume exactly one shield, record
  `RegenerationShieldUsed`, tap that target, clear its marked damage, and
  remove it from combat instead of moving it. Shields never prevent a
  zero-toughness action or a sacrifice and clear when their target leaves the
  battlefield.
- A targeted damage-prevention shield is private replacement state with a
  positive remaining amount, a seated player or live creature target, a
  current-turn expiry, and a retained source identity. Creation is a stack
  effect and emits `DamageShieldCreated`; the source may have left the
  battlefield as an activation cost without invalidating the shield. Damage
  consumes only the represented amount and emits `DamagePrevented`; no damage
  trigger is queued for the prevented portion. A target departure or cleanup
  transition removes the shield and emits `DamageShieldExpired`, and an
  exhausted shield cannot remain in game state.
- `Keyword::Protection(color)` is source-aware permanent protection: a source
  with that color cannot target the protected permanent at cast, activation,
  trigger-selection, or resolution-time revalidation. A creature with
  protection from an attacker's color cannot block it, and an attacker with
  protection from a blocker's color cannot be blocked by it. Damage from a
  matching-color source is prevented through the same `DamagePrevented`
  receipt path unless the source has `DamageCannotBePrevented`; non-targeted
  damage batches therefore still honor protection even when no target slot
  exists. A stack object retains the source colors of its originating
  incarnation, so a source that leaves the battlefield and loses a temporary
  color cannot evade protection during target revalidation or damage
  prevention. Nonmatching-color and colorless sources remain unaffected. A
  matching-color Aura becomes illegally attached when protection is gained,
  then the ordinary SBA moves it to its owner's graveyard and expires every
  attachment-linked continuous effect.
- A bounded prospective damage event carries source incarnation, target
  incarnation (when the target is a permanent), affected player, remaining
  amount, and the exact replacement identities already used. For the initial
  one-effect targeted instant/sorcery slice, the engine gathers live
  target-shields, permanent shields/protection, and full-event redirections
  before it records any damage. Two or more candidates open a no-priority
  decision visible only to the affected player; the submitted identity must
  still be live, is applied once, and applicability is recomputed. The stack
  spell remains live while this decision is pending. Its public
  `DamageReplacementApplied` receipt precedes the authoritative
  `DamagePrevented`, `DamageRedirected`, or committed `DamageDealt*` receipt,
  and only committed positive damage queues damage triggers.
- `Keyword::DamageCannotBePrevented` excludes prevention only. It bypasses
  target shields, permanent shields, protection, and color-based prevention,
  but does not bypass a non-prevention damage redirection. A redirected event
  receives a new target and is then reconsidered against that recipient's
  applicable replacements; one source-bound replacement identity cannot
  apply twice to the same prospective event.
- The current decision continuation is intentionally narrow: it supports one
  targeted direct-damage instant/sorcery and redirections large enough to
  replace the entire remaining event. Partial redirection, multi-instruction
  stack continuations, optional replacements, and arbitrary replacement
  ordering remain explicit engine gaps rather than deterministic claims.
- A dynamic creature-count life-gain effect snapshots all current battlefield
  creatures at resolution, including tokens and opposing creatures, converts
  the count into a bounded receipt, and queues life-gain triggers only for the
  committed `LifeGained` amount. It does not use a stale cast-time count or
  inspect creatures that have already left the battlefield.
- Every catalog definition has identity and a type. A basic land is a land;
  only lands have intrinsic mana colors; creatures have both power and
  toughness; and dredge values are positive.
- A registered basic-land type line names exactly one basic-land definition and
  its singleton intrinsic mana color agrees with the typed land identity.
  `CardView` preserves that identity, and a typed intrinsic activation cannot
  produce a different color. A cast-payment basic-land request must name one
  such typed land and its intrinsic color; it cannot use an untyped land or
  produce a different color while paying a spell cost.
- A registered land-entry behavior names exactly one land definition and
  currently represents only a mandatory tapped entry. Playing that land marks
  it tapped before state-based actions and before any resulting ETB ability is
  placed on the stack. It does not fold an ETB instruction into the land-play
  transition: normal trigger placement, target legality, priority, and
  resolution receipts remain required. Controller-relative trigger selection
  must use controller-relative legality, so a `ControlledLand` target can
  select the newly entered land itself but can never select an opponent's land.
- A `LandEntersBattlefield` or `ControlledLandEntersBattlefield` trigger binding
  is permitted only on a permanent source and uses the same checked
  effect/target shape as every other trigger. Every represented land entry
  first makes the land live, reaches its ordinary state-based-action boundary,
  queues that land's own ETB triggers, then scans every live non-token
  permanent. The first condition observes every entry; the second only stacks
  for a source controlled by that entering land's controller. An entry that
  occurs while another spell or ability resolves retains its entering
  controller in a deferred batch until that enclosing stack object has emitted
  its terminal receipt and changed zones; it never creates a nested stack
  object mid-resolution.
  Each observer receives its own ordinary synthetic stack object and
  `TriggeredAbilityStacked` receipt; the active player keeps the normal
  post-resolution priority window and a trigger never performs its effect
  inline.
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
- A target-bearing attack trigger cannot select a stable-order fixture target.
  It remains outside the stack at a no-priority policy boundary, exposes one
  ordered legal option set per target occurrence only to its controller, and
  emits `TriggeredAbilityStacked` only after that controller submits the exact
  pending source, ability, and a legal target for every slot. Rejected choices
  leave the pending decision, stack, zones, mana, and event log unchanged.
- Every migrated no-priority choice occupies the one typed, clonable
  `PendingDecision` state slot. Its positive `DecisionId` is strictly less
  than the next monotonic id, is never reused after completion, names one
  living deciding player, and has distinct typed options with a valid
  inclusive min/max cardinality. `DecisionOpened` and `DecisionCompleted`
  form one unique, matching public receipt lifecycle per id; neither receipt
  contains hidden candidate or selection identities.
- `PolicyAction::SubmitDecision` must supply the currently live exact id and a
  selection whose type, cardinality, uniqueness, and options match the typed
  continuation. A stale id, different player, duplicate, out-of-range count,
  or illegal option rejects atomically. While the decision remains open,
  priority cannot pass or interleave, the suspended stack item remains stable,
  and no other pending decision family may coexist. Compatibility actions for
  older policies dispatch through the same continuation but new policies must
  use the id-bearing generic action.
- This first unified-decision migration covers policy-submitted one-card
  library searches, triggered discard/sacrifice object choices, multi-block
  combat order, spell-copy targets, and concurrent token/counter quantity
  replacement ordering. Library
  search and discard options are private: only the deciding player's
  `GameView` contains their candidate identities, while a public sacrifice
  option is projected safely to its deciding controller. `DecisionContinuation`
  holds only typed cloned data, never a resolver closure. Future target,
  optional-cost, color, and the legacy damage prevention/redirection
  replacement choice remain separate bounded decision families until migrated
  to it.
- Every represented trigger condition captures one source/controller/payload
  event and reaches a common active-player-first placement pipeline after its
  enclosing action. A target-bearing event stays outside the stack until its
  controller chooses every legal target; later events cannot overtake that
  pending placement. Dynamic damage-trigger instructions use the captured
  positive amount, not a later damage accumulator. A targetless optional
  trigger opens the same accept/decline boundary even with a zero mana cost.
  The represented one-effect all-player-discard and controller-creature-
  sacrifice triggers keep their stack object live while the relevant chooser
  submits a legal current hand or battlefield object; no deterministic fixture
  selection may move a card or permanent.
- Every simultaneous controller group with two or more represented triggers
  opens one public `TriggeredAbilityOrder` decision before any member of that
  group reaches the stack. Its options and submitted permutation are exact,
  duplicate-free `(source, source-incarnation, ability)` identities; stale,
  partial, foreign, duplicate, or wrong-player answers are atomic rejections.
  Groups are processed active player first and then in living turn order, so
  the active player's submitted group is lower on the stack than each
  nonactive group. `DecisionCompleted` immediately precedes the public
  `TriggeredAbilityOrderChosen` receipt, which precedes the next member's
  placement (or its requisite no-priority target choice); no priority action
  or later APNAP group may interleave with an unfinished group.
- A `DealsCombatDamageToCreature` trigger is queued only after a positive
  `DamageDealtToPermanent` combat receipt reaches a creature. Its materialized
  effect carries the recipient's exact battlefield incarnation and has no
  target slot: prevention or redirection creates no trigger for the original
  recipient, while a later zone change makes the captured instruction a
  harmless no-op rather than affecting a new object with the same stable id.
- An `OpponentCardPutIntoGraveyard` trigger observes every ordinary,
  incarnation-advancing non-token transition into a player's graveyard. It
  compares that owner with each live observer's controller, so a discard,
  mill, destroyed permanent, countered spell, or paid cost can qualify while
  a card entering its own controller's graveyard cannot. The normal
  `CardMoved` and `ObjectIncarnationAdvanced` receipts remain the public event
  provenance; trigger placement remains deferred until the enclosing action
  completes.
- After every surviving player passes on a trigger with an optional mana cost,
  the trigger remains the top stack object and opens a no-priority decision for
  its controller. The view exposes the exact cost, current affordability, and
  legal conditional targets. Declining requires no target and spends nothing;
  accepting requires a payable pool and the exact target shape. Only acceptance
  emits `AbilityManaPaid`, and rejected submissions are atomic no-ops.
- A registered generic-cost reducer has a catalogued permanent source, a
  strictly positive amount, and is registered before the game starts. It
  contributes only while a source with that definition is live on the casting
  player's battlefield, changes generic symbols only, and is applied after an
  explicit chosen-X value but before mana or Convoke payment. A
  `CastsNoncreatureSpell` trigger retains the exact triggering spell in its
  one `NoncreatureSpell` target slot, stacks above that spell after `SpellCast`,
  and either sacrifices one creature controlled by its resolving controller
  or emits `SpellCountered` followed by the target's ordinary terminal move.
  Until a policy submits that sacrifice choice, stable battlefield order is an
  explicitly bounded fixture-selection rule rather than full choice fidelity.
- A stack instruction that depends on colors spent to cast its spell requires
  a nonempty `mana_spent` receipt on that exact stack object. When the visible
  event log contains its `SpellCast`, the immediately preceding
  `SpellManaPaid` receipt must name the same controller, card, and ordered
  colors. Floating mana added or spent after casting cannot alter this
  resolution-time provenance.
- A spent-mana global modifier uses that same stack-owned receipt. When its
  named color is present, it snapshots every current battlefield creature only
  after preceding instructions in that spell have resolved, installs one
  temporary layer-seven effect per snapshot member, then reaches the normal
  post-resolution SBA boundary. When the color is absent, it creates no
  continuous-effect receipt or hidden modifier.
- Stack controller, effects, and target-slot count must match the represented
  card definition. Every executable occurrence of a target requirement owns
  one ordered stack slot; each slot also retains the target's captured object
  incarnation (`None` only for player targets), so a target that leaves and
  re-enters is illegal for the original stack object. The same object may
  occupy multiple slots when the source has multiple independent target
  occurrences. Tokens and lands cannot occupy the stack. A target may later
  become illegal, but it cannot be absent, fabricated, or change enum kind
  after cast time. `Target::Spell`
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
- Every stack object also captures its source incarnation. Spell objects must
  name the source's current incarnation while they remain on the stack;
  activated and triggered abilities may intentionally retain a historical
  incarnation after their source leaves (notably a dies trigger). The
  `AbilityActivated`, `TriggeredAbilityStacked`, `AbilityResolved`, and
  `AbilityCounteredByRules` receipts carry that same value, and the ability
  lifecycle audit keys open/terminal receipts by `(source, incarnation,
  ability)` rather than the stable object ID alone. Source-relative effects
  must do nothing if their captured source is not the same live battlefield
  incarnation.
- A `CreatureCardInControllerGraveyard` target names a creature catalog card
  currently in the resolving controller's graveyard. An ETB ability with an
  intervening "another creature card" condition is stacked only if that
  graveyard contains the target plus at least one other creature card, and
  rechecks the same count while resolving. If only its target remains, the
  ability resolves without a zone move rather than returning an ineligible
  card or fabricating a new target.
- An `EnchantmentCardInControllerGraveyard` target names only an enchantment
  card owned by the resolving controller and currently in that player's
  graveyard. Target validation occurs before an activation spends mana or pays
  its tap cost and is rechecked at resolution; a legal return records the
  ordinary owner-hand zone transition before its terminal ability receipt.
- A target-free all-player graveyard-return instruction snapshots at most one
  creature catalog card from each living player's own graveyard before it
  moves any card. Each selected card remains owner-preserving and may move
  only from that graveyard to that owner's hand; a player with no eligible
  card contributes no fabricated selection or zone receipt.
- A target-free controller-graveyard land-return instruction snapshots no
  more than three land cards owned by the resolving controller before any of
  them move. It may not select an opponent's card or a nonland, and each
  selected card follows the ordinary graveyard-to-hand transition. Until a
  policy supplies the public-zone choice, the stable zone order is the
  explicitly bounded deterministic selection rule rather than full fidelity.
- A persistent typed `CounterKind` belongs only to a live battlefield
  permanent, has a valid nonalias name and a strictly positive quantity, and
  clears on every departure from the battlefield, so a later incarnation can
  never retain counter state. `+1/+1` and `-1/-1` counters each contribute
  once to derived creature power and toughness after supported timestamped
  layer-seven effects; every other named kind is real permanent state with no
  implied characteristic rule. Every `CounterPlaced` and `CounterRemoved`
  receipt names a valid kind and a positive quantity. Placement applies the
  prospective quantity-replacement pipeline before mutation; removal never
  does. Removal preflights the live counter balance and, if insufficient,
  atomically restores the resolving stack object, event log, and counter map.
  Generic target-counter instructions require a live permanent at casting and
  resolution. This bounded substrate does not yet represent counters on
  players or nonbattlefield objects, nor a counter-removal *activation cost*;
  it represents typed add/remove resolving effects.
- A registered quantity replacement has a catalogued permanent source, a
  multiplier of at least two, and is fixed before the game begins. When tokens
  are created or a represented counter is placed, only live battlefield
  sources controlled by the affected player apply. Every candidate and
  `ReplacementEffectApplied` receipt records the source's exact positive
  incarnation; the same source incarnation/effect cannot apply twice to one
  prospective event. One-effect stack token/counter instructions with two or
  more live candidates open a public `DecisionKind::Replacement` boundary for
  the affected player. The submitted option is revalidated, applied once, and
  candidates are recomputed; a fresh monotonic decision id opens only while
  two or more choices remain, while one remaining candidate applies without a
  prompt. `DecisionCompleted`/`DecisionOpened` and a submitted policy receipt
  may appear between causal replacement receipts, but the replay audit still
  requires a finite same-event chain ending in exactly the resulting
  `TokenCreated` batch or `CounterPlaced` receipt. Direct non-suspended helper
  paths use the same live candidate/application logic in stable order.
- A `DistinctCreature` target slot must name a creature permanent and may not
  reuse any other distinct-creature occurrence in the same spell. The cast
  validator and the stack-provenance audit both reject a duplicate before any
  cost, zone, or event mutation. Once the spell is on the stack each distinct
  occurrence remains its own resolution slot, so a later illegal target is
  skipped without collapsing the remaining legal instructions.
- A targeted-discard instruction owns one player target slot and requests a
  strictly positive count. At resolution it may move only cards still in that
  target's hand, emits `CardDiscarded` before the corresponding graveyard move
  for each card, and cannot discard from an unrelated player. Until a policy
  submits hidden-hand choices, the implemented selection is the target
  player's oldest current hand entry rather than a cast-time snapshot.
- A dynamic opponent-creature-count life-loss instruction is target-free but
  evaluates every living opponent independently at ability resolution. It
  excludes the resolving controller, reads only that opponent's current
  battlefield creatures, emits no zero-amount receipt, and records each
  positive change as the ordinary source-aware `LifeLost` event before the
  ability's terminal resolution receipt.
- A top-library reveal instruction reads the current top object only at
  resolution, records `CardRevealed` before moving that exact object to hand,
  and only then records its source-aware `LifeLost` using the catalog mana
  value. An empty library creates neither a reveal nor a draw-loss event.
- A private-library resolution choice is a one-effect, spell-only suspension
  boundary with a positive inspected-card count and positive per-card life
  payment. Its stack spell remains live and is the current top item while the
  controller alone sees the exact current top-card snapshot through
  `GameView`; opponents see no candidate identities. The snapshot must still
  be exactly the controller-owned current library top sequence, priority must
  stay with that controller with zero passes, and no draw replacement may
  coexist. A submitted selection is unique and a subset of that snapshot,
  checks the controller's life before mutation, then atomically records
  `LifePaid`, moves each selected card to hand and each other inspected card
  to graveyard, and only then records the spell's terminal resolution and
  source-zone receipts. No priority action or pass can interleave.
- A private opponent-library exile choice is a one-effect, targeted
  activated-ability suspension with a positive inspection count. Its live top
  stack object must retain the activating source, ability identity, controller,
  and one living opponent target; it never exposes candidate identities through
  the public event log or the target opponent's `GameView`. The controller-only
  candidate list must equal the current top-first sequence in that opponent's
  library, must remain stable while priority is blocked, and requires exactly
  one selected candidate when nonempty (or no selection when the library has
  no candidates). Resolution atomically moves only that selected card to exile
  before `AbilityResolved`; the unchosen cards preserve their original library
  order.
- Its receipt-order audit applies only to the activated-ability identities it
  has observed. A triggered ability's ordinary `AbilityResolved` receipt is
  not evidence of a private-choice lifecycle and remains valid without an
  `AbilityActivated` receipt.
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
- A `FlyingCreature` target requirement names only a current battlefield
  creature whose resolved characteristics include `Flying`. Target validation
  happens before any spell or activated-ability cost mutates mana, zones, or
  receipts, and the same requirement is rechecked at resolution. Its ordinary
  destruction path therefore preserves regeneration replacement and the
  standard `CardDestroyed` followed by zone-departure lifecycle.
- A `NonblackCreature` target requirement names only a current battlefield
  creature whose resolved characteristics do not include Black. It is checked
  before casting costs are paid and again at resolution; the legal destruction
  path remains regenerable and records ordinary `CardDestroyed` then
  zone-departure receipts. A black target is rejected atomically rather than
  becoming a silently accepted no-op.
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
  without granting general hidden-library access. A suspended typed library
  search similarly projects only its resolving controller's matching candidate
  identities; opponents receive no candidate list or selected-card identity
  before the ordinary zone-move receipt.
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
  positive. A fixed mana bundle has positive entries and `amount == 0`; a paid
  bundle additionally has a positive explicit mana cost, while a free bundle
  must never fabricate a zero-cost payment receipt. Activation requires the
  source on the battlefield and controlled by the priority holder. A tap-cost
  creature ability additionally observes summoning sickness unless its current
  characteristics include Haste. Fixed outputs reject a supplied color choice
  and selectable outputs require one listed color. A successful bound mana
  ability never enters the stack, emits its exact single-color, paid-bundle, or
  free-bundle receipt followed by every required life-payment and `ManaAdded`
  receipt, resets consecutive passes, and leaves priority with its activator.
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
- Source- and land-untap effects are stack instructions, not hidden step
  transitions. The source variant is a no-op unless its original source is
  still a tapped battlefield permanent; the land variant rechecks that its
  selected target remains a battlefield land. Either variant records exactly
  one `PermanentUntapped` receipt only for an actual tapped-to-untapped change;
  automatic Untap steps continue to use only their grouped turn receipt.
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
- A spell instruction that depends on a chosen `X` cannot use ordinary
  `cast_spell`: the caller must submit one explicit nonnegative value and an
  explicit payment allocation for every remaining symbol after reductions and
  Convoke. The value, actual Convoke-symbol count, and applied generic
  reduction are retained as cast-time stack provenance—not inferred from a
  later battlefield query—and the mana receipt must contain exactly the
  remaining payment-symbol count. A stack-using activated ability may carry a
  chosen-X value only when its immutable generalized cost profile requires it;
  its `AbilityXCostChosen` receipt occurs during payment and its actual generic
  amount is added before live increases/reductions. Both cast-time and
  resolution-time checks read their respective retained values, so a
  fabricated or undersized receipt fails the invariant audit before it is
  treated as a legal state transition.
- A registered activated-cost modifier has a catalogued permanent source, a
  positive generic amount, one unique declarative modifier per source
  definition, and is fixed before the game starts. The initial RAV-sufficient
  boundary admits only modifiers scoped to nonmana activated abilities; a mana
  ability is never taxed. Every live source applies once to any player's
  eligible activation, so multiple sources stack in stable object-id order and
  ordinary source departure immediately removes its contribution. The typed
  cost context retains acting player, activating source/incarnation, ability,
  base mana cost, increases, reductions, all represented nonmana costs,
  optional selected mana allocation, and final effective cost. Increases apply
  before reductions; only generic symbols change, and colored/hybrid symbols
  remain identical. A modified activation emits
  `ActivatedAbilityCostCalculated` immediately before a matching
  `AbilityManaPaid` for its effective cost. The full cost is preflighted before
  any tap, sacrifice, discard, stack, priority, zone, or receipt mutation; an
  insufficient or malformed payment is therefore an atomic no-op. The replay
  audit rejects a fabricated source/incarnation, unsupported scope, duplicate
  binding, invalid adjustment, changed colored requirement, incorrect effective
  total, missing effective-payment receipt, or malformed explicit selection.
- An expansion may register one nonempty immutable
  `GeneralizedActivatedAbilityCost` profile for an already-bound stack-using
  activated ability before the game begins. Its concrete choices arrive only
  in the normal priority action `GeneralizedAbilityActivation`; this is not a
  resolution-time pending decision and therefore cannot interleave with a
  stack continuation. Counter-source selection has exact ordered arity and
  names either the source required by its profile or a live controlled
  permanent. Counter totals are aggregated by `(permanent, CounterKind)`
  before any mutation, so repeated declared removals cannot overspend the
  same counters. Every `CounterRemovedAsAbilityCost` receipt is immediately
  followed by an equal ordinary `CounterRemoved` receipt. A return-cost
  selection has exact ordered arity, requires the source first when required,
  otherwise names distinct controlled battlefield permanents, and every
  `ReturnedAsAbilityCost` receipt is immediately followed by its owner-hand
  move (and structural incarnation receipt). A positive life payment may not
  exceed the controller's current life and records `AbilityLifePaid`. The
  complete mana/life/counter/return/X block is validated against exactly one
  subsequent `AbilityActivated` receipt; missing, fabricated, duplicated, or
  mismatched components fail the replay audit. A later invalid component rolls
  back every earlier debit, zone move, stack mutation, and receipt. The
  initial public entry point deliberately does not yet combine explicit
  generic/hybrid mana-color selection with generalized-cost payment; that
  compositional API remains an explicit follow-up rather than silently using
  deterministic spending.
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
- A definition-bound activation marked `sorcery_speed` is accepted only while
  its controller is the active player in a main phase and the stack is empty.
  This check runs before target validation, cost payment, stack placement, or
  receipts, so a rejected timing attempt leaves the mana pool, zones, stack,
  and event log unchanged. Target-player draw remains a stack instruction and
  therefore moves the selected player's current top card only at resolution.
- Transmute is a source-incarnation-backed synthetic activated ability, not an
  immediate hidden-zone operation. At legal sorcery speed it pays its printed
  mana cost, emits `AbilityManaPaid`, discards its source with
  `DiscardedAsAbilityCost`, then places exactly one targetless
  `transmute` ability on the stack before `AbilityActivated`. Its one effect is
  a policy-submitted controller-library search for exactly the discarded
  card's mana value. The activating player retains priority; no card is
  revealed, moved from a library, or shuffled until every surviving player has
  passed and the private search decision is completed. A successful selection
  emits `CardRevealed` before its hand move, followed by
  `LibrarySearchResolved`, `LibraryShuffled`, `Transmuted`, and
  `AbilityResolved`; a legal failure to find has no reveal/move but retains the
  terminal search, shuffle, and Transmuted receipts. The private decision is
  visible only to the ability controller and blocks priority submissions.
- A resolving turn-scoped library-search-prevention effect records one
  `LibrarySearchesPrevented { source, until_turn }` receipt for the current
  nonzero turn. The legacy immediate `Game::transmute` compatibility helper
  rejects before costs or zone changes; the policy-stack Transmute path and
  other stack-resolving search effects instead resolve with `found: None` and
  retain their required shuffle. The marker may equal only the current turn and
  is cleared as the next turn begins; a stale marker is an invariant failure.
- A stack-based typed library search examines only the resolving controller's
  library and selects no more than one card satisfying its expansion-neutral
  predicate. A deterministic selector remains an explicitly bounded
  compatibility mode. A policy-submitted selector instead suspends its
  one-effect resolving stack item with zero passes in a private
  `PendingDecision`; only that controller sees the ordered matching candidates.
  Their `DecisionId`-bearing submitted object must still be in that snapshot
  and match the typed predicate (including a spell's retained chosen X).
  `None` is legal only when the search permits failure to find or has no
  candidates. Priority and every unrelated choice reject atomically while the
  boundary is open.
  Completion records a normal `CardMoved` entry to its exact declared
  destination (`Battlefield`, `BattlefieldTapped`, or `Hand`) before
  `LibrarySearchResolved`, immediately follows the latter with that
  controller's `LibraryShuffled` receipt, and records no selected-card
  movement when `found` is absent. Either battlefield destination may queue
  ordinary entry-trigger work only after the search source reaches its own
  terminal stack lifecycle.
- A policy-submitted batch library search uses an expansion-neutral typed
  predicate and either `ZeroOrMore { maximum }` or `Exactly(count)` selection
  cardinality. Its candidate identities project only to the resolving
  controller, include only current controller-owned library cards matching the
  typed predicate, and reject stale, duplicate, oversized, or foreign answers
  atomically. An exact hidden-zone search with too few candidates is a legal
  zero-card failure-to-find boundary; one that permits failure to find may
  submit fewer than the requested count. Each selected card moves through the
  ordinary destination transition (and is revealed first only when the effect
  requires it); `LibrarySearchBatchResolved` names the ordered selected set
  and is immediately followed by exactly one controller `LibraryShuffled`
  receipt.
- `RevealTopLibraryCardsAndReorder` snapshots at most its positive requested
  top-card count in current top-to-bottom order, emits one public
  `CardRevealed` receipt for each snapshot member, then opens one public
  `LibraryReorder` pending decision. Every player sees the same revealed
  option set, but only its controller may submit the exact stale-safe
  `DecisionId`; the submitted selection must be a complete duplicate-free
  permutation. `LibraryReordered { top_to_bottom }` records that permutation
  before the suspended spell or ability reaches its terminal receipt. No zone
  transition, shuffle, or unrelated priority action may interleave with the
  captured ordering boundary.
- A `Permanent` target is a current battlefield object, never a player or a
  card in another zone. A permanent-bounce instruction snapshots the target's
  controller before its owner-hand zone move; its `CardMoved { to: Hand }`
  receipt therefore precedes the corresponding positive source-aware
  `LifeLost` receipt and the spell's terminal resolution lifecycle. An
  illegal target rejects atomically before costs, stack placement, or receipts.
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
- A Radiance enchantment-destruction spell has one live enchantment target at
  cast and resolution. At resolution it snapshots that target plus every other
  live enchantment sharing at least one target color before any zone change.
  An all-illegal target set is countered by rules with no destruction or token
  instruction; a legal target never broadens the batch to artifacts,
  creatures, colorless non-target enchantments, or nonsharing enchantments.
- A target-free global nontoken-creature destruction effect snapshots every
  current battlefield creature whose object is not a token before the first
  destroy instruction. Tokens and noncreatures receive no destruction
  instruction; every selected creature follows the ordinary regenerable
  destruction and zone-departure lifecycle. The complete batch of resulting
  receipts precedes the enclosing spell terminal receipt and one subsequent
  state-based-action fixed point.
- Landwalk is likewise declaration provenance: every recorded attacker is in
  the unique declared-attacker set and retains a nonempty set of typed basic
  land types. A submitted blocker is rejected exactly when the fixed defender
  controls at least one recorded land type for that attacker. The provenance is
  cleared when that creature leaves combat or the combat declaration resets,
  so a later keyword or land change cannot rewrite the legality already
  recorded for that combat.
- A target constrained to an attacking-or-blocking creature has the normal
  permanent target shape, but must additionally identify a battlefield
  creature present in the active combat's attacker or blocker provenance at
  cast and resolution time. A noncombat creature is rejected atomically rather
  than being silently accepted by a generic creature-target effect.
- Combat damage occurs only after attacker and blocker declarations. It is
  recorded as player/permanent damage events, then state-based actions run.
  A creature with zero or negative power assigns no combat damage and emits no
  damage event; negative power can never increase life or remove marked damage.
  Every nonempty blocker group begins in defender declaration order and every
  blocker is globally unique across the combat. Before priority, the attacking
  player receives one public `CombatDamageOrder` decision for each group with
  more than one blocker. Its exact option set is a fixed-cardinality
  permutation; `DecisionOpened → DecisionCompleted → CombatDamageOrderChosen`
  records the no-priority lifecycle and then the public result. A stale,
  duplicate, partial, foreign-player, or out-of-group selection is atomic. The
  ordered-attacker provenance is a subset of declared multi-block groups and
  all such groups must be ordered before either combat-damage step. Each live
  eligible blocker assigns its complete positive power. A blocked nontrample
  attacker with one live blocker assigns its complete positive power to that
  blocker; with several blockers, assignment first gives each remaining lethal
  damage in the submitted order, then assigns any nontrampling remainder to
  the first ordered blocker so damage is never silently dropped.
  Alternative combat restrictions and other unsupported combat rules must be
  reported as capability gaps rather than approximated.
- `trampling_attackers` is declaration provenance only: it is a subset of the
  uniquely declared attackers and cannot exist before their declaration. At
  combat damage, Trample is evaluated from the attacker's live characteristics,
  so a later supported characteristic change can affect assignment without
  corrupting the historical declaration. The bounded substrate retains every
  attacker-submitted blocker order. A positive-power live Trample attacker assigns
  each live blocker's remaining lethal damage (after marked damage) in that
  order, then its positive excess exactly once to the fixed defender; if all
  blockers have left combat, all its positive assignment goes to that defender.
  For a source with `Deathtouch`, one positive assigned point is lethal to
  each creature regardless of toughness. Every committed positive
  deathtouch-damage packet sets source-quality provenance on the target
  incarnation; SBA treats that marker as lethal, even if the source later loses
  Deathtouch, and clears it together with marked damage at regeneration,
  cleanup, and battlefield re-entry. Combat prevention/replacement ordering and
  arbitrary player-selected assignment amounts remain explicit capability gaps
  rather than approximated damage assignment.
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

- A permanent copy is a layer-one snapshot applied before every implemented
  continuous-effect layer (4--7) and before counter-derived layer-seven
  modifiers. Its `CopiableValues` are either one catalog card definition or
  one validated token specification. They never include marked damage,
  counters, tapped/controller/attachment state, or any static or timestamped
  derived effect on the copy source.
- A live copied snapshot belongs only to one battlefield incarnation, names a
  distinct source with a positive source incarnation and globally unique,
  positive timestamp, and names an extant catalog definition or valid token
  specification. Source departure does not revoke that snapshot: it is a
  copied value, not a source-dependent continuous effect. When the target
  leaves the battlefield, the snapshot clears before its new incarnation can
  be observed and the canonical trace emits one `PermanentCopyExpired` record
  after the ordinary zone/incarnation receipts.
- `PermanentCopied` carries the exact source and target incarnations plus the
  layer-one timestamp. Copying a card makes its definition-bound activated
  abilities and static bindings use that captured definition; copying a token
  changes a card's characteristics without changing the card into a token.
  `AbilityActivated` records its effective definition at activation, so a
  later source or target zone change cannot make a historical copied ability
  fail replay validation against its resumed printed definition.
- Continuous effects are applied in the implemented layer order (1, 2, 4--7), then
  timestamp order within a layer. End-of-turn effects expire during cleanup;
  marked damage clears there. An effect removed because its source or target
  leaves the battlefield emits an explicit expiration lifecycle receipt.
- Every continuous effect names extant source and target objects, has a unique
  positive monotonic timestamp, captures both endpoint incarnations, and has a
  valid duration. A permanent-duration effect cannot outlive either matching
  battlefield endpoint; an end-of-turn effect belongs to the current turn
  only and may retain its historical source after a spell has left the stack.
  In either duration, it cannot apply to a target that has left and returned.
- A layer-two `ChangeController` effect names one living player and one live
  battlefield permanent. Active effects apply in timestamp order, so the most
  recent applicable effect determines `Game::controller_of`; its source and
  target incarnations prevent a previous object from retaining control after a
  zone change. Installation records `ContinuousEffectCreated` then, when the
  derived controller actually changes, `ControllerChanged`. Expiration or a
  source departure records the ordinary expiration receipt before an audited
  controller-reversion receipt. The base controller is never mutated.
- A permanent records the turn of its most recent controller change. Attacking
  and tap-symbol ability checks use that provenance—not merely battlefield
  entry—so a creature stolen this turn is summoning sick for its new controller
  unless it has Haste. Controller-relative target checks, views, untap,
  combat, triggers, cost reductions, replacement effects, and attachment
  legality query the same derived controller. Leaving the battlefield always
  uses the owner destination, even when another player controlled it.
- Aura spells and no-cast Aura entry share the same typed target, protection,
  controller-relative restriction, endpoint-incarnation, linked-effect, and
  receipt rules. A no-cast entry validates before moving the Aura, so an
  illegal target leaves both zones and the event log unchanged. Equipment
  attachment is stack-backed through an ordinary activated ability; source
  departure expires every linked effect, while target departure or changed
  legality produces an unattached Equipment and an `AttachmentDetached`
  receipt. No attachment may follow a stable object ID across a zone change or
  silently modify a returned incarnation.
- The bounded linked-exile resolver intentionally stores no closures. Its
  typed group and delayed-action records remain invariant-valid while the
  creature and every linked Aura are suspended in exile, and the consuming
  end-step transition reaches the normal SBA boundary after returns. It is a
  substrate only: individual card promotion, arbitrary simultaneous delayed
  action ordering, and broader blink/zone-replacement interactions remain
  separate coverage work.
- Static continuous bindings are immutable expansion data, never timestamped
  runtime effects. Each registered binding names one creature definition and
  one supported static change. It applies only while an object with that
  definition is on the battlefield, creates no synthetic event receipt, and
  is reevaluated from live battlefield state whenever characteristics are read.
  A controller-scoped `other creature` binding applies only to a creature with
  the same controller that is distinct from its source; it therefore neither
  buffs the source nor leaks to an opponent's battlefield. Multiple legal
  sources combine in layer order and a source departure removes its static
  contribution without a synthetic expiry receipt. An Aura-conditioned
  controller-creature binding instead requires at least one live Aura-like
  permanent whose attachment points at its source; it includes that source and
  its other controlled creatures, never an opponent's creatures. Normal Aura
  departure immediately revokes that derived contribution. A static change
  cannot be inserted into the timestamped continuous-effect list.
- Static attack-restriction bindings are likewise immutable expansion data and
  can be registered only before game start. Each names a permanent definition,
  remains active only while a matching source is on the battlefield under the
  defending player's control, and is checked before a nonempty attacker
  declaration mutates any tapped state, combat provenance, or event log. A
  rejected declaration is therefore atomic; normal source departure revokes
  the restriction without a synthetic event or stale combat marker.
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

## Virtual spell copies and effect-created casting permissions

- A represented spell copy is a virtual, stack-only object with a fresh,
  monotonic `ObjectId`. It has no zone entry or physical `CardObject`, exactly
  one live stack object, and references one lower physical original spell.
  Its copied definition, effects, target-incarnation provenance, mana-spend
  snapshot, and source provenance must agree with that original. It leaves no
  card in a terminal zone: resolution records `SpellCopyResolved` and a
  rules-counter records `SpellCopyCounteredByRules`.
- A copy whose target set may be changed suspends through the same monotonic
  `DecisionId` state machine as other public choices. Its options are public
  legal target values, its cardinality equals the original spell's target-slot
  count, and its continuation can complete only while the copying spell is
  still on top of the stack with the same original-source provenance. A stale
  decision, an altered source, or an illegal target rejects atomically.
- Effect-created cast permissions are exact-card, exact-incarnation grants.
  They record player, source provenance, source zone, payment mode, timing
  exception, and current-turn expiry. A permission is removed when used, at
  cleanup, or on any zone transition, and a cast using one records
  `SpellCastFromPermission`. A noninstant may bypass ordinary sorcery timing
  only while its live spell stack object carries that permission's explicit
  timing exception.
- This substrate is deliberately bounded: virtual copies can resolve and
  retain or retarget the original represented targets, but copies of virtual
  copies and general copy-modification/replacement choices are not yet
  represented. New permission effects currently grant a card from exile;
  graveyard and other alternate-casting sources require their own typed
  effects rather than an implicit zone mutation.

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
