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
  exiled to its owner. A stack-only virtual spell copy controlled by that
  player has no owner-zone move, so it instead ceases with one
  `SpellCopyLeftGame { copy, original, controller }` receipt. No departed
  player's object or virtual copy may later appear in a zone, on the stack, in
  combat, or in an effect.
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
  exactly one primary-creature member, zero or more duplicate-free attached
  Aura members, and one delayed return action. A source may be an Aura,
  artifact, land, or spell; its departure never erases the scheduled return.
  Each member records the exact positive incarnation it had after the ordinary
  exile move. A delayed return moves only members that are still in `Exile`
  with that exact incarnation; a card that independently changed zones is
  never revived or reattached merely because its stable `ObjectId` matches.
  The primary creature returns first; linked Auras follow in stable object-id
  order and may attach only through a new ordinary legality-checked attachment
  to that returned incarnation.
- A source-linked hand-exile group is keyed by one positive source identity
  and exact source incarnation and contains a nonempty duplicate-free set of
  cards that remain in `Exile` at their captured exact incarnations. It is
  source-relative rather than controller-relative: a later control change
  neither loses prior members nor changes their owner-indexed hand return.
  The controller of the return trigger receives its following draw, while each
  returned card uses its ordinary owner-hand zone transition. The group remains
  live only while that source incarnation is on the battlefield or an
  already-stacked return instruction still names it. A hand card leaving its
  captured exile incarnation is removed from the group; source departure
  without such a pending return emits `LinkedHandExileExpired` and cannot
  leave unreachable private state. `HandExiledWithSource`,
  `LinkedHandExileReturned`, and expiry receipts name nonempty, unique public
  object identities while ordinary zone/incarnation receipts remain the
  authoritative transition sequence.
- A delayed linked-exile action has a nonzero unique action identity, typed
  `EndStep` timing, and a due turn no earlier than the current turn. It is
  consumed exactly once with its group, removing both from suspended state.
  `DelayedActionScheduled` names the exact exiled members, while
  `DelayedActionConsumed` records only the duplicate-free subset that was
  still eligible to return. Those receipts are replay-audited against their
  schedule; ordinary `CardMoved` and incarnation receipts remain zone truth.
- An attachment binding has one typed source kind (`Aura` or `Equipment`),
  one permanent-only target restriction, and a duplicate-free set of optional
  linked continuous changes. Aura bindings require an Enchantment source and
  matching permanent-spell attachment effect; they may have no linked change
  when their represented behavior is source-relative. Equipment bindings
  require a nonempty set, an Artifact source, and matching activated or
  target-bearing triggered attachment effect. A live Aura has
  exactly one legal attached target; if either endpoint or legality
  disappears, the ordinary SBA moves it to its graveyard and expires every
  linked effect. Equipment may be unattached; its attach activation may move
  it only between legal endpoint incarnations, expires only its prior linked
  effects, and leaves it on the battlefield when an endpoint becomes illegal.
  A typed Aura activated attachment may likewise move only between legal
  endpoint incarnations: ordinary Aura entry remains unattached-only, while a
  successfully resolving reattachment expires exactly the old linked effects
  before installing the new endpoint's effects and receipt.
  Every live attachment has exactly one permanent effect per declared change.
  `AuraAttached` and `EquipmentAttached` immediately follow the final linked
  effect receipt. A zero-change attachment instead emits
  `AttachmentEstablishedWithoutContinuousEffect`, which must not be paired
  with a synthetic layer receipt. `AttachmentDetached` names a previously
  attached Equipment, including one with no declared changes.
  An attachment binding may additionally grant a duplicate-free set of typed
  nonmana activated abilities. Such a grant is available only to the exact
  live attached permanent incarnation; its attachment immediately revokes it
  on either endpoint's departure, reattachment, or illegal-attachment SBA.
  The attached permanent remains the activation source for tap costs, target
  legality, damage, and controller checks, while `AbilityActivated.definition`
  records the immutable attachment binding that supplied the grant. Once
  activated, its complete typed ability shape is retained by the ordinary
  stack object and remains resolvable if the attachment departs in response;
  replay rejects an ambiguous or fabricated attachment-grant stack ability.
- A spell may snapshot its resolving controller's current creature permanents
  and install an exact typed activated-ability grant on each recipient as a
  timestamped layer-six end-of-turn continuous effect. Every recipient retains
  its own source identity, controller, tap cost, and target validation; the
  effect source is receipt provenance only. A later creature is not a
  recipient, and a recipient's zone change immediately expires its grant by
  exact incarnation. An activation made while the grant is live remains an
  ordinary stack object after that expiry or its source's departure; historical
  stack validation matches the immutable grant shape and rejects ambiguous or
  fabricated provider bindings.
- A timestamped layer-seven attachment change may scale power and toughness by
  the number of other creatures controlled by its target's current controller.
  The target itself is excluded, opposing creatures never contribute, and the
  value is derived whenever characteristics are read; ordinary creature
  departure or control change therefore recalculates it without a stale
  counter or synthetic layer receipt.
  A suppression change rejects only nonmana activated abilities before any
  cost or receipt; mana abilities remain legal.
- A source-relative Aura instruction reads only the permanent currently
  attached to its exact live source incarnation at resolution. If that source
  has departed, returned, or become unattached, the instruction is a no-op;
  it must never follow a stable object ID into a later incarnation. Returning
  a live attached permanent uses the ordinary owner-hand zone transition,
  after which SBA performs the now-unattached Aura's normal graveyard cleanup.
- A non-token object has exactly one catalog definition; a token has exactly
  one token specification and exists only on the battlefield. An object cannot
  be both, and no nonpermanent card can occupy the battlefield.
- A token's mechanically relevant creature subtypes are typed separately from
  its display name. A token with any such subtype must be a creature; the
  public `Characteristics` view preserves that type-line information and the
  token's static keywords through stack resolution and zone placement.
- A `CreateTokenForTargetOpponent` ETB trigger records one synthetic stack
  object and requires its controller to choose exactly one living opponent
  before any priority window. Its public generic decision records the
  decision-maker but only that controller receives the actionable target-choice
  view; a foreign submission is atomic. On resolution, every `TokenCreated`
  recipient and typed token specification must match the selected target; it
  cannot target its controller, fan out to every opponent, or substitute a
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
- `ReturnSourceToOwnersHand` is a source-relative resolution instruction, not
  an activation cost. It moves the source only when that exact source
  incarnation is still on the battlefield; an already departed source leaves
  the instruction as a no-op and a later incarnation with the same stable
  object ID can never be returned by the old stack object.
- `MoveSourceToOwnersLibraryAndShuffle` has the same exact-live-incarnation
  boundary. It performs the ordinary owner-indexed `Library` move first, then
  deterministically shuffles that immutable owner's library and records one
  `LibraryShuffled` receipt with its post-move card count. The ability
  controller is never a substitute for the owner after control changes; if a
  response removes or re-creates the source, the historical instruction is a
  no-op with no library move or shuffle receipt.
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
- A target-side prevention shield created by a typed activated ability has one
  exact player or live permanent target and a strictly positive amount. The
  source's ordinary tap-cost receipt/stack lifecycle is authoritative for its
  creation, while the shield itself follows the ordinary target lifecycle: a
  later source tap state or source departure cannot retarget, duplicate, or
  silently revoke that already-created shield.
- A permanent static source may grant
  `PreventDamageFromControlledSources` to each current creature its controller
  controls. The quality is evaluated for every prospective permanent-damage
  packet against the damage source's current controller, so it prevents only
  friendly-source damage and leaves an opposing controller's source unchanged.
  It is derived characteristic state, never a standalone mutable shield; a
  static source's departure immediately revokes the quality through the normal
  battlefield/characteristics query boundary.
- A spell effect may instead create that same shield for its controller from
  its retained chosen-X value. The cast boundary requires explicit X and pays
  it as generic mana; a positive X emits `DamageShieldCreated` with exactly
  that amount before the spell's ordinary terminal zone move. X equal to zero
  is legal, draws or resolves the spell's other instructions normally, and
  creates no zero-valued shield or synthetic prevention receipt.
- A target-bearing Radiance prevention instruction snapshots the target plus
  every other current battlefield creature sharing at least one of that
  target's colors, even across controllers. It emits one independent positive
  `DamageShieldCreated` receipt per selected creature; an off-color creature
  never receives one, and a colorless target selects only itself. Each receipt
  is governed by the ordinary target-side shield lifecycle above, rather than
  a shared mutable batch record.
- A source-side combat-damage prevention record has a unique positive id, a
  current-turn expiry, a retained creating-source identity, and one exact live
  battlefield creature incarnation. It is created only while its target still
  satisfies the typed attacking-or-blocking target requirement, emits
  `CombatDamagePreventionCreated`, prevents every positive combat packet from
  that exact creature through `CombatDamagePrevented`, and cannot follow a
  leave-and-return incarnation. Cleanup or target departure removes it with
  `CombatDamagePreventionExpired`. `DamageCannotBePrevented` bypasses this
  prevention replacement but not unrelated redirection.
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
  target-shields, permanent shields/protection, and bounded redirections
  before it records any damage. Two or more candidates open the same public,
  id-bearing `DecisionKind::Replacement` boundary used by quantity
  replacement; its options are `ReplacementChoice::Damage` values visible
  only to the affected player. The submitted identity must still be live, is
  applied once, and applicability is recomputed. The stack spell remains live
  while this decision is pending. The legacy `ChooseDamageReplacement` action
  is only a checked compatibility shim over that exact current `DecisionId`;
  new policies use `SubmitDecision`. Its public `DamageReplacementApplied`
  receipt precedes the authoritative `DamagePrevented`, `DamageRedirected`, or
  committed `DamageDealt*` receipt, and only committed positive damage queues
  damage triggers. The generic decision closes after that causal damage batch
  and before the suspended spell's terminal lifecycle.
- A registered damage-amount replacement has a catalogued permanent source and
  is fixed before the game begins. Every live source applies once to a
  prospective player or permanent packet and retains its source incarnation in
  both the `DamageReplacementApplied` identity and the immediately following
  `DamageAmountReplaced` arithmetic receipt. The receipt has a positive input,
  the bound reduced output, and must name the same source/target as its
  preceding replacement identity. Direct and combat paths use the same source
  discovery; the initial registered operation is integer halving, so stable
  application of multiple identical sources cannot change its result.
- A registered source-bound combat replacement may replace that source's
  positive combat-damage packet to a player with immediate non-damage
  consequences. `CombatDamageReplacedWithMillAndCounters` records the exact
  source incarnation, damaged player, and replaced positive amount; it must
  be followed by the matching source `+1/+1` counter placement and no
  intervening ordinary player-damage receipt from that source to that player.
  Any resulting library moves retain their ordinary `CardMoved` and
  incarnation receipts. This currently supplies a deterministic
  single-replacement compatibility path only. If it competes with another
  applicable combat-damage replacement, the affected-player ordering choice
  is deliberately not fabricated; that broader replacement-order boundary is
  tracked as an open engine weakness.
- `Keyword::DamageCannotBePrevented` excludes prevention only. It bypasses
  target shields, permanent shields, protection, and color-based prevention,
  but does not bypass a non-prevention damage redirection. A redirected event
  receives a new target and is then reconsidered against that recipient's
  applicable replacements; one source-bound replacement identity cannot
  apply twice to the same prospective event.
- The current decision continuation is intentionally narrow: it supports one
  targeted direct-damage instant/sorcery and bounded redirections that may
  split the pending event. A partial redirect commits its new-recipient packet
  first and retains the protected remainder as a deterministic deferred packet
  inside the same no-priority stack continuation. Every deferred packet has a
  positive amount, a live matching target incarnation, a valid affected
  player, and unique prior replacement identities before it can be resumed;
  replacements are re-evaluated at each recipient. Multi-instruction stack
  continuations, optional replacements, and a fully general replacement-event
  algebra remain explicit engine gaps rather than deterministic claims.
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
  `CardView` preserves the derived current identity, and a typed intrinsic
  activation cannot produce a different color. A timestamped layer-four
  `ReplaceBasicLandType` effect may replace a live land's type through the
  current turn; each affected land then has exactly the chosen type's
  intrinsic one-color mana ability while retaining its other represented
  abilities. Nonlands cannot receive this change, an opponent land is not
  silently selected by a controller-scoped effect, and expiration restores
  the definition-bound type. A cast-payment basic-land request must name a
  currently typed land and its current intrinsic color; it cannot use an
  untyped land or produce a different color while paying a spell cost.
- A registered static entry restriction names a permanent source, is immutable
  before the game begins, and is rechecked from the battlefield at each
  ordinary entry. The represented rules apply either to opponents' artifacts,
  creatures, and lands or to the source's own artifact/creature/land entry;
  neither alters setup injection, an ally's entry, or an
  enchantment/instant/sorcery entry. A changed non-token entry records `CardMoved(Battlefield) →
  ObjectIncarnationAdvanced → PermanentEnteredTapped` with the exact source
  incarnation; a changed token entry records `PermanentEnteredTapped →
  TokenCreated` with incarnation one. The audit rejects absent transition
  provenance, invalid source/controller identities, an unsupported source
  binding, or a fabricated token incarnation. A source-relative entry receipt
  has identical permanent and source identities and must name the dedicated
  source-entry binding; all other tapped-entry receipts must name an opposing
  live restriction source. Source departure immediately
  revokes the restriction without a delayed cleanup effect.
- A definition-bound mana ability may require a registered source sacrifice as
  a physical cost. The policy submits every selected output color bundle
  explicitly; it must contain only offered colored mana and exactly the
  declared positive total. The engine preflights the entire bundle and mana
  cost before mutation, then records `SacrificedAsManaAbilityCost` followed
  immediately by the ordinary graveyard transition and incarnation receipt
  before producing mana. The source's normal leaves-the-battlefield triggers
  are then queued and resolve through the ordinary stack/priority lifecycle;
  source sacrifice never turns a mana ability itself into a stack object.
- A stack instruction that draws for each controlled registered basic-land type
  snapshots the resolving controller's live count once at that instruction's
  resolution boundary. Only the exact registered type contributes; lands an
  opponent controls, untyped lands, display-name coincidences, and later zone
  changes do not. The resulting draw attempts use the ordinary spell-effect
  draw lifecycle and retain its empty-library handling.
- A registered land-entry behavior names exactly one land definition and
  represents either a mandatory tapped entry or one positive optional life
  payment that permits an untapped entry. The latter choice is explicitly
  submitted with the land play; an unqualified legacy land-play action cannot
  silently choose it. A paid choice requires current life, records
  `LandEntryLifePaid`, and that receipt must be immediately followed by the
  exact card's ordinary battlefield move under a matching immutable binding.
  A decline marks the land tapped. Neither branch creates a stack item or a
  priority boundary. Normal trigger placement, target legality, priority, and
  resolution receipts remain required after entry. Controller-relative trigger
  selection must use controller-relative legality, so a `ControlledLand`
  target can select the newly entered land itself but can never select an
  opponent's land.
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
- A stack object has a unique, positive, monotonic `StackObjectId` independent
  of its source card and a valid controller. Thus two activations of the same
  permanent remain distinguishable public stack objects. Resolving or
  countering a spell removes it from the stack before it receives its resulting
  zone move.
- `ActivatedAbilityWithSingleTarget` names one lower, currently live activated
  stack item by `StackObjectId`, never a source `ObjectId`. Its target must
  have exactly one target occurrence. The public activated-stack view exposes
  that identity and target occurrence to policies without exposing hidden-zone
  information. A retarget decision holds the resolving spell at the stack top
  with no priority, captures the lower ability's controller, source colors,
  target requirement, and original target, and accepts only a different target
  that remains legal for that lower ability at completion. It then emits one
  `ActivatedAbilityTargetChanged` receipt before the enclosing spell continues
  to its remaining effects; if no different legal target exists, that one
  instruction is a no-op.
- A `PhysicalSpell` target names a lower, non-ability, non-copy stack card.
  It is intentionally unavailable to stack-only virtual copies, whose
  identity has no physical object or terminal zone. A
  `CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent`
  instruction snapshots that target's controller and printed mana value while
  it is still a physical stack object; it records `SpellCountered`, performs
  the target's ordinary terminal zone move, and only then may move up to that
  many cards from the captured controller's library. The mill gate reads only
  the resolving source's captured explicit mana-payment receipt, never the
  mutable mana pool or the departed target. A nonmatching receipt still
  counters and terminally moves the target but emits no mill moves.
- A bound triggered ability has a synthetic stack identity that is not a card
  object or zone member. Its private metadata and public stack item must agree
  on source, controller, and ability, carry no spell targets/effects/payment,
  and be removed together when it resolves or its controller leaves. Trigger
  placement follows the source permanent's `CardMoved { to: Battlefield }`
  receipt; resolution emits the draw/effect receipts before its terminal
  `TriggeredAbilityResolved` receipt.
- A target-bearing trigger cannot select a stable-order fixture target. It
  remains outside the stack in a public, id-bearing
  `DecisionKind::TriggeredAbilityTargets` continuation, whose captured source
  incarnation, source colors, controller, registered ability identity, and
  ordered target requirements are audited before it can complete. The
  controller submits `DecisionSelection::Targets` with one legal target per
  occurrence; non-`DistinctCreature` occurrences may name the same legal
  target more than once, while `DistinctCreature` occurrences may not. The
  compatibility `ChooseTriggeredAbilityTargets` action is only a checked shim
  over the live exact `DecisionId`. `DecisionCompleted` precedes
  `TriggeredAbilityStacked`; rejected stale, wrong-controller, wrong-source,
  wrong-ability, wrong-cardinality, illegal, or distinctness-violating answers
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
- `DecisionKind::CounterUnlessPaysMana` keeps its counterspell at the stack
  top and captures that stack object's source/incarnation/controller together
  with the lower target spell's identity/incarnation and controller. Only the
  lower spell's living controller may submit either an explicit decline with
  no mana data or `DecisionSelection::CounterUnlessPaysMana { pay: true, .. }`
  with every selected generic/hybrid color and every intervening mana ability
  listed in order. Each listed ability and the final selected spend are one
  atomic no-priority payment transaction: failure leaves mana, tap state,
  stack, pending decision, and receipts unchanged. A successful payment emits
  `CounterUnlessPaysManaPaid` immediately before the matching
  `DecisionCompleted`; it has one color per declared mana symbol. Declining
  emits no payment receipt and counters only the captured lower spell. Neither
  path permits an automatic payment, an automatic decline, a self-target, an
  ability-stack target, or a target at or above the counterspell.
- `DecisionKind::TargetPlayerManaColor` retains exactly one live stack item
  with one `Player` target and one
  `AddOneManaOfTargetPlayersChosenColor` instruction. The decision belongs to
  that captured recipient rather than the resolving controller, is public,
  requires exactly one answer, and offers precisely the represented five card
  colors that can still fit in the recipient's bounded mana pool; it never
  offers `Colorless`. The source, source incarnation, controller, target, and
  current option set are revalidated before completion, so a foreign, stale,
  malformed, noncolored, or capacity-overflow answer is atomic. The chosen
  output is materialized only while the ordinary resolver resumes, emits one
  `ManaAdded` receipt for the target player after `DecisionCompleted`, and
  cannot appear in a catalog definition or permanent ability binding.
- This first unified-decision migration covers policy-submitted one-card
  library searches, trigger target selection and triggered discard/sacrifice
  object choices, multi-block combat order, spell-copy targets, concurrent
  token/counter quantity replacement ordering, and bounded direct-damage
  prevention/redirection, plus private top-library hand/top/bottom
  partitions. Library search, discard, and top-library partition options are
  private: only the deciding player's `GameView` contains their candidate
  identities, while a public sacrifice option is projected safely to its
  deciding controller. `DecisionContinuation` holds only typed cloned data,
  never a resolver closure. Optional-cost, color, partial-redirection, and
  arbitrary replacement-event composition remain separate bounded decision
  families until migrated to it.
- `DecisionKind::ConditionalPrivateDiscard` retains one non-ability spell at
  the stack top with exactly one live `Player` target and the one matching
  `DrawTargetPlayerThenConditionalPrivateDiscard` instruction. That targeted
  recipient, rather than the spell controller, receives three ordinary
  spell-effect draws before the private decision opens. Its candidate set is
  exactly that recipient's current owned hand, is visible only through that
  recipient's `GameView`, and accepts only one current land card or two
  distinct current hand cards. Foreign, stale, duplicate, non-land singleton,
  wrong-zone, wrong-owner, and malformed answers are atomic. Completion emits
  the ordinary discard and graveyard transitions before the source spell's
  terminal resolution/zone receipts; candidate identities never enter the
  public decision receipts. If the recipient leaves the game during its
  mandatory draws, no decision opens: the still-live source completes its
  terminal stack/zone lifecycle before the terminal game receipt, while a
  source already removed by player-departure cleanup retains only its ordinary
  `ObjectLeftGame` terminal provenance.
- Every represented trigger condition captures one source/controller/payload
  event and reaches a common active-player-first placement pipeline after its
  enclosing action. A target-bearing event stays outside the stack in its
  generic target decision until its controller chooses every legal target;
  later events cannot overtake that pending placement. Dynamic damage-trigger
  instructions use the captured positive amount, not a later damage
  accumulator. A targetless optional trigger opens the same accept/decline
  boundary even with a zero mana cost. The represented one-effect all-player-
  discard and controller-creature-sacrifice triggers keep their stack object
  live while the relevant chooser submits a legal current hand or battlefield
  object; no deterministic fixture selection may move a card or permanent.
  A controller-creature-sacrifice candidate and its final payment both query
  `controller_of`, never the owner-bound base controller, so a player may
  choose a creature they currently control through a layer-two effect; its
  normal owner-indexed graveyard move then ends the temporary control effect.
- A `ControlledAuraEntersBattlefield` observer queues only when a live
  Aura-like permanent enters under that observer's current controller. The
  ordinary trigger stack item retains the live observer source/incarnation and
  controller; an opponent's Aura cannot queue it. Its target-free optional
  decision belongs only to that controller: acceptance creates exactly the
  declared typed token and the matching `TokenCreated`/`AbilityResolved`
  receipts, while decline creates neither a token nor a fabricated effect
  receipt. Zone departure or control change after placement is handled by the
  ordinary trigger-source and stack lifecycle audits.
- A `ControlledNonartifactPermanentEntersBattlefield` observer snapshots the
  entering permanent's positive incarnation and nonempty current card-type
  set at entry. Its `ReturnAnotherControlledPermanentSharing...` stack effect
  can resolve only through a public zero-or-one object decision: the chooser
  may decline, but a selected object must be a different current battlefield
  permanent they control and share at least one captured type. The entering
  permanent may subsequently leave or change types without changing the
  captured event. A stale, foreign, departed, same-object, or non-overlapping
  choice is atomic; a successful choice uses the ordinary owner-hand zone
  transition before the trigger's `AbilityResolved` receipt.
- A `Blocks` trigger is observed only for a creature that was successfully
  committed as a legal blocker. It waits until the defender's complete
  declaration and every required attacker damage-order decision have finished,
  then stacks before ordinary post-block priority. Replay requires its source
  in the immediately preceding `BlockersDeclared` assignment and rejects a
  trigger for an attacker, unrelated creature, rejected attempted block, or a
  prior combat step. Its source-relative effect retains the ordinary exact
  source-incarnation boundary, so a response that moves the blocker cannot
  cause an old trigger to move a later object sharing its stable ID.
- A target-free token-producing ETB trigger from an Aura is not an inline
  spell effect: its source must first enter the battlefield, establish every
  legal attachment-linked continuous change, and emit the corresponding
  attachment receipt. Only then may `TriggeredAbilityStacked` open its normal
  priority window; no `TokenCreated` receipt may precede that trigger's own
  terminal `AbilityResolved` receipt.
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
- `FirstNoncreatureSpellCastEachTurn` separately records every player's first
  noncreature spell for the current turn; it is not a global first-spell flag
  and a countered first spell still consumes only that player's slot. The
  per-player state resets only on the next Untap turn transition. When a live
  binding observes such a cast, `FirstNoncreatureSpellCastThisTurn` names the
  exact turn, caster, and spell immediately after that spell's cast-incarnation
  receipts and before its trigger is stacked. Replay rejects duplicate
  player/turn receipts, a creature spell, or nonadjacent cast provenance.
- A stack instruction that depends on colors spent to cast its spell requires
  a nonempty `mana_spent` receipt on that exact stack object. When the visible
  event log contains its `SpellCast`, the immediately preceding
  `SpellManaPaid` receipt must name the same controller, card, and ordered
  colors. Floating mana added or spent after casting cannot alter this
  resolution-time provenance.
- A spell instruction that requires one chosen card color rejects an omitted
  choice and rejects `Colorless` before any payment, zone move, stack entry,
  or accepted policy receipt. `PolicyAction::CastWithColorChoice` retains the
  exact five-color value on the spell stack object and records one matching
  `SpellColorChosen` receipt before `SpellCast`. The choice is neither inferred
  from mana spent nor silently defaulted. An activated or triggered ability
  cannot fabricate this spell-only provenance; a virtual copy instead keeps
  the original spell's retained color. Resolution uses only the retained value
  to install controller-team temporary protection or a target's temporary
  replacement color, so later policy state or mana-pool changes cannot alter
  it.
- A spent-mana global modifier uses that same stack-owned receipt. When its
  named color is present, it snapshots every current battlefield creature only
  after preceding instructions in that spell have resolved, installs one
  temporary layer-seven effect per snapshot member, then reaches the normal
  post-resolution SBA boundary. When the color is absent, it creates no
  continuous-effect receipt or hidden modifier.
- Stack controller, effects, and target-slot count must match the represented
  card definition. Every executable occurrence of a target requirement owns
  one ordered stack slot; each slot also retains the target's captured object
  incarnation (`None` for players and activated-stack-item targets), so a
  target that leaves and re-enters is illegal for the original stack object.
  The same object may
  occupy multiple slots when the source has multiple independent target
  occurrences. Tokens and lands cannot occupy the stack. A target may later
  become illegal, but it cannot be absent, fabricated, or change enum kind
  after cast time. `Target::Spell` additionally retains and validates the
  immutable card definition after CR 800.4a removes that object from the live
  game. Its typed boundary remains exact: `InstantOrSorcerySpell` accepts only
  an instant or sorcery, `NoncreatureSpell` excludes creature cards, and
  `Spell` accepts any represented physical spell card (including permanent
  spells), never an activated or triggered ability.
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
- `PutTargetCreatureOnOwnersLibraryTop` accepts only a live creature target
  whose exact incarnation still matches the occurrence captured when its spell
  or triggered ability entered the stack. Its zone transition uses the ordinary
  owner-indexed `Library` move, so the card becomes the final library element
  (the draw top) and receives normal zone-departure cleanup and an incarnation
  receipt. If a response removes or re-enters the target, the original
  occurrence is illegal; a sole-target trigger receives
  `AbilityCounteredByRules` and must neither move nor affect the later object.
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
- `AddSourceKeywordUntilEndOfTurn` is a target-free source-relative stack
  instruction. When its exact source incarnation remains on the battlefield,
  it creates exactly one layer-six `AddKeyword` continuous effect whose source
  and target are that source; the effect expires at the resolving turn's
  cleanup. If a response moved or re-created the source, the ability still
  receives its ordinary terminal receipt but must create neither a continuous
  effect nor a keyword on the later incarnation.
- A `CreatureCardInControllerGraveyard` target names a creature catalog card
  currently in the resolving controller's graveyard. An ETB ability with an
  intervening "another creature card" condition is stacked only if that
  graveyard contains the target plus at least one other creature card, and
  rechecks the same count while resolving. If only its target remains, the
  ability resolves without a zone move rather than returning an ineligible
  card or fabricating a new target.
  A legal owner-library-top instruction instead moves that same exact target
  through the ordinary owner-indexed library transition; it cannot target an
  opponent-owned graveyard card, a noncreature card, or a later incarnation,
  and its `CardMoved { to: Library }` receipt precedes terminal resolution.
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
  players or nonbattlefield objects. Typed counter-removal activation costs are
  explicitly selected and paid through the generalized activated-cost boundary;
  ordinary typed add/remove effects remain distinct resolving instructions.
- A source-counter-derived life-loss instruction reads only the resolving
  source's exact live battlefield incarnation after every preceding stack
  instruction has completed. It emits no loss when that source has departed or
  has no represented counter total. A positive materialization records a valid
  `SourceCounterLifeLoss` provenance receipt immediately followed by the equal
  ordinary `LifeLost` mutation; an orphaned, invalid-kind, nonpositive, or
  mismatched pair fails the invariant audit.
- A source-counter mana-value sweep is materialized before its activation costs
  mutate state. Its source counter value must be a nonnegative, exact-incarnation
  `SourceCounterValueMaterialized` receipt immediately before the matching
  sacrifice-source activation; the live stack item then contains only that
  numeric value, never a template that could reread a departed or re-entered
  source. Resolution snapshots every nonland permanent with that current mana
  value before destroying any of them, including zero-mana tokens, and an
  orphaned receipt, a mismatched stack value, or an unmaterialized template
  fails the invariant audit.
- A target-free beginning-of-upkeep source-counter creature sweep is distinct
  from that sacrifice-cost materialization: after an exact
  `TriggeredAbilityOrder` decision it reads only the source's still-live,
  exact battlefield incarnation at resolution. The counter kind must be valid,
  the sampled quantity nonnegative, and the complete creature recipient set is
  snapshotted before any destruction. A departed or re-entered source is an
  auditable no-op in this bounded substrate; it may never donate a later
  incarnation's counters to an old stack object. Exact last-known counter
  values for departed sources remain an explicit fidelity gap.
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
  strictly positive count. When that target has cards, its one-effect stack
  item suspends at a private `ConditionalPrivateDiscard` decision controlled
  by the target, never the caster. The continuation retains the exact
  stack-item id, source/incarnation/controller, ability identity when present,
  target, requested count, and ordered recipient-owned hand
  object/incarnation snapshot. A submitted selection has the exact required
  cardinality (`min(requested, snapshot length)`), is duplicate-free, and is
  revalidated against the unchanged live hand snapshot before any mutation.
  Wrong-player, stale-id, foreign, duplicate, wrong-zone, or reincarnated-card
  submissions are atomic. `DecisionOpened` and `DecisionCompleted` expose only
  safe metadata; `CardDiscarded` immediately precedes each corresponding
  graveyard move, and no public receipt discloses the private candidates.
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
- A target-player library-top may-choice is a one-effect, targeted activated
  ability suspension. Its controller alone sees precisely the current top
  object of the one live target player's library through `GameView`; neither
  the public decision receipts nor the target player's view disclose that
  identity. The continuation captures the source, source incarnation,
  controller, ability, target, and exact top object; all must still agree with
  the live stack and library when the controller submits either no object or
  that one exact object. The selected object moves to its owner's graveyard
  only after `DecisionCompleted`, while the declined object remains the target
  library top. `PrivateTargetPlayerLibraryTopChoiceOpened` immediately follows
  its matching private `DecisionOpened`, appears once per decision, and is
  closed by the same controller's matching `DecisionCompleted` before the
  ability's terminal receipt.
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
- `GameView` never exposes an opponent's hand or ordinary hidden library, nor
  does it expose a departed player as an opponent life-total or battlefield
  target. A live registered static top-library reveal source is the narrow
  exception: every seated policy view receives exactly one `CardView` for the
  current final element of each nonempty owner-indexed library, with no second
  card or cached prior top. The projection is recomputed from live battlefield
  source definitions and ordinary library zones, so draws, shuffles, zone
  changes, and source departure immediately change or revoke it without a
  synthetic visibility receipt. It otherwise projects only the
  controller-owned, mana-value-matching library cards for each transmute card
  in that controller's hand, allowing an honest search decision without
  granting general hidden-library access. A suspended typed library search
  similarly projects only its resolving controller's matching candidate
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
- A non-Instant card on the stack has valid timing only if it was cast in its
  controller's main phase with an empty stack, has an explicit current casting
  permission, or its printed characteristics include `Flash`. A pending
  resolution-time decision retains its designated decision controller as the
  priority holder; resolving a spell or ability must not overwrite that
  identity with ordinary active-player priority until no decision remains.
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
  a token is created (and by the invariant audit), while additive and
  replacement layer-five color changes are rejected before installation. A
  typed nonbasic land may produce it through the same bound mana-ability
  receipts as colored mana.
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
- A stack-using activated ability captures its controller at activation in the
  immutable stack object and matching `AbilityActivated` receipt. A later
  layer-two control change to its live battlefield source does not change or
  invalidate that historical controller: effects resolve for the captured
  activator, while ordinary source zone and target-incarnation checks continue
  to govern legality independently. In particular, a valid response that
  steals the source must not make the response transition fail its invariant
  audit or retarget the already-activated ability.
- Every activated-ability sacrifice cost is represented by explicit, distinct
  policy-selected controlled battlefield permanents in binding order: source
  sacrifices first, then the configured number of creatures, then lands. The
  engine validates cardinality, control, current zone, and each required type
  before mana, zone, tap, stack, or receipt changes. Each successful selection
  records `SacrificedAsAbilityCost` immediately followed by its graveyard move
  (or token-ceases receipt), before `AbilityActivated`; a rejected selection is
  an atomic no-op with no mana debit or cost receipt.
- A generalized activation-cost profile may further require that every bound
  land sacrifice currently has one exact `BasicLandType`. Registration rejects
  such a profile when its ability has no land-sacrifice slot; activation rejects
  a wrong typed land before mana payment or any event. Replay checks each
  matching historical `SacrificedAsAbilityCost` receipt against the immutable
  profile, so a fabricated or substituted non-Forest land cannot become a
  valid payment after its graveyard move.
- An activated ability that requires additional creature taps receives exactly
  that many explicit, distinct, controlled, untapped non-source creature
  selections from the policy. These are cost taps rather than tap-symbol
  activations of the selected creatures, so their summoning sickness is not a
  restriction. All selections are validated before mana, zones, taps, stack,
  or event history change. Every successful selected tap emits
  `AdditionalCreatureTappedAsAbilityCost` immediately before its matching
  `AbilityActivated`; the invariant audit rejects orphaned, duplicate,
  wrong-source, or wrong-cardinality receipts.
- A bound activated ability may require exactly one policy-submitted
  `BasicLandType` choice. That semantic choice is neither a permanent target
  nor a source of object-incarnation provenance: it is validated before any
  mana payment, materialized into the immutable stack-effect payload, and
  cannot be omitted, duplicated, or supplied to an ability that does not
  declare it. A rejected choice leaves the mana pool, stack, and event log
  unchanged.
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
- Source-, land-, and permanent-untap effects are stack instructions, not
  hidden step transitions. The source variant is a no-op unless its original
  source is still a tapped battlefield permanent; target variants recheck that
  their selected target remains a battlefield land or, for the broader typed
  form, any battlefield permanent. Each records exactly one
  `PermanentUntapped` receipt only for an actual tapped-to-untapped change;
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
  trigger. `ControlledNontokenCreatureDies` observes only a non-token
  creature whose live controller equals the observing source's live controller
  immediately before the battlefield-to-graveyard transition; token and
  opposing-creature deaths queue no such event. Dies triggers retain the historical source object after a graveyard move and
  materialize every declared target slot before stacking; a legal selected
  target cannot be dropped or replaced by an empty target vector.
  A `LifeGained` trigger is captured only from a source controlled by the
  player named by the positive `LifeGained` receipt while that source is on
  the battlefield, then stacked only after the enclosing
  spell or ability reaches its terminal receipt. Its optional mana cost is
  paid at trigger resolution, not while the trigger is stacked; an unpaid
  optional cost resolves with no damage, while a paid trigger selects a legal
  creature-or-player target at resolution and deals its fixed amount.
  ETB triggers with targets require a controller-submitted legal target for
  each declared occurrence; a targeted opponent trigger cannot select its
  controller or silently fan out to every opponent. A two-target redirection activation must resolve both target
  instructions before installing its replacement shield, and an incomplete
  or countered activation cannot leak a pending half-effect. If the protected
  target becomes illegal before resolution, the paired destination instruction
  is a no-op rather than a resolver error or leaked final-pass transition.
  Every materialized dynamic effect is checked against its binding before it
  can resolve, and no pending attack, damage, life-gain, or dies trigger may survive its
  enclosing transition.
  A recipient-damage trigger that derives the damage source's controller must
  capture that controller at the damage receipt, materialize a target-free
  player instruction, and never open a policy target decision. Its positive
  amount and captured player must survive source/recipient zone changes and
  match the binding before resolution.
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
- `PolicyAction::CastWithPayment` is the policy-facing dispatch for that same
  cast transaction. It supplies exactly one explicit generic/hybrid allocation
  and an optional chosen-X value, then delegates to the ordinary selected-spend
  or chosen-X path; it never has a second cost-calculation or payment rule.
  Consequently, the normal `SpellManaPaid → SpellCast` receipt pair and the
  stack object's chosen-X, mana-spent, Convoke, and reduction provenance remain
  authoritative. `PolicyMoveSubmitted { kind: Cast }` is written only after a
  successful complete action. A malformed allocation, missing required X, or
  spurious X therefore leaves mana sources, mana pool, zones, stack, pass
  sequence, and event log unchanged—including no accepted policy-move receipt.
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
- A nonempty Convoke-contributor provenance group is keyed by one live,
  physical non-ability spell and that spell's exact stack incarnation. Its
  captured contributors are nonzero, duplicate-free `(ObjectId, incarnation)`
  pairs and its length exactly matches the spell's recorded Convoke symbols.
  The group is created only after a nonempty Convoke payment succeeds; it is
  consumed on permanent entry and removed on countering, terminal spell
  movement, or player-departure cleanup. A Root-Kin-style ETB marker
  materializes into that exact captured set—or a valid empty set for a spell
  cast with no Convoke payment—and resolution counters only contributors that
  are still live creature permanents with the captured incarnation. Thus a
  later zone change or a later cast of the same physical card cannot inherit a
  prior Convoke payment.
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
  move (and structural incarnation receipt). A hand-to-library cost has exact
  ordered arity, names distinct cards owned by the activating player while
  they are in hand, cannot overlap an activation discard, and each
  `HandCardPutOnLibraryTopAsAbilityCost` receipt is immediately followed by
  its ordinary library move; selections are committed bottom-to-top. A
  positive life payment may not
  exceed the controller's current life and records `AbilityLifePaid`. The
  complete mana/life/counter/return/X block is validated against exactly one
  subsequent `AbilityActivated` receipt; missing, fabricated, duplicated, or
  mismatched components fail the replay audit. A later invalid component rolls
  back every earlier debit, zone move, stack mutation, and receipt.
  `GeneralizedAbilityActivation` may additionally carry the ordinary typed
  `ManaPaymentSelection` for every generic and hybrid symbol in its calculated
  cost. That selection is preflighted in the same transaction, recorded in an
  `ActivatedAbilityCostCalculated` context bound to the exact source
  incarnation, and retained as the live stack ability's ordered `mana_spent`
  colors. The context must lead to the same player/source/incarnation/ability
  activation; a malformed hybrid color, stale context, or live stack object
  with different spent colors fails the audit. Effects that inspect mana
  colors reject deterministic ability payment, just as their spell equivalents
  do.
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
  A `reveal_selected: true` instruction records exactly one public
  `CardRevealed` receipt for the chosen current library object immediately
  before that object's normal `CardMoved` receipt; the player, object, and
  definition must agree. A `reveal_selected: false` generic search creates no
  synthetic selected-card reveal (Transmute has its separately specified
  reveal lifecycle). Completion then records the normal `CardMoved` entry to
  its exact declared destination (`Battlefield`, `BattlefieldTapped`, or
  `Hand`) before `LibrarySearchResolved`, immediately follows the latter with
  that controller's `LibraryShuffled` receipt, and records neither a selected
  card movement nor a selected-card reveal when `found` is absent. Public
  receipts cannot disclose candidate identities before an actual selected-card
  reveal. Either battlefield destination may queue ordinary entry-trigger work
  only after the search source reaches its own terminal stack lifecycle.
- A source-bound Aura search is the same private, id-bearing zero-or-one
  library decision, but its candidates are additionally restricted to Auras
  that can establish a registered typed Aura attachment to the resolving
  source's exact current battlefield incarnation. The candidate set becomes
  empty if that source has left or returned before resolution. A selected Aura
  must still be in its owner's library and remain legal for that exact source
  when submitted; the engine completes `DecisionCompleted`, performs the
  Aura's ordinary battlefield/incarnation and attachment receipts, then
  records `LibrarySearchResolved` and `LibraryShuffled` before the parent
  ability's terminal receipt. An explicit decline or empty candidate set
  creates no Aura zone move, but still records the result/shuffle pair. The
  decision is private to the source controller and its candidates are never
  exposed in another `GameView` or the canonical event log.
- A policy-submitted search-and-cast continuation is an exact one-effect
  activated-ability stack boundary. It opens only for the resolving controller,
  exposes only current matching instant candidates, and accepts one selected
  card plus that spell's normal typed targets (or a legal failure to find).
  The engine validates the source and source incarnation, candidate snapshot,
  selected card, and eventual spell targets before it emits a one-shot,
  exact-card, exact-incarnation `Library` cast permission. Its receipt order is
  `DecisionCompleted → LibrarySearchResolved → SpellCastFromPermission →
  SpellCast → LibraryShuffled → AbilityResolved`; opponents receive priority
  only after the parent ability has completed and the selected spell is the
  live top stack object. A source-Equipment detach generalized cost is legal
  only for a currently attached Equipment and atomically expires its linked
  attachment effects before `AbilityActivated`; a rejected payment mutates no
  mana, attachments, stack, or receipts.
- A policy-submitted batch library search uses an expansion-neutral typed
  predicate and either `ZeroOrMore { maximum }`,
  `ZeroOrMoreDistinctNames { maximum }`, or `Exactly(count)` selection
  cardinality. Its candidate identities project only to the resolving
  controller, include only current controller-owned library cards matching the
  typed predicate, and reject stale, duplicate, oversized, or foreign answers
  atomically. A distinct-name cardinality additionally rejects two selected
  cards sharing one printed name even when their definition ids differ. The
  typed `Aura` predicate accepts only Enchantments carrying one Aura
  attachment effect, never a non-Aura Enchantment. An exact hidden-zone search
  with too few candidates is a legal zero-card failure-to-find boundary; one
  that permits failure to find may submit fewer than the requested count. Each
  selected card moves through the ordinary destination transition (and is
  revealed first only when the effect requires it); `LibrarySearchBatchResolved`
  names the ordered selected set and is immediately followed by exactly one
  controller `LibraryShuffled` receipt. The receipt auditor preserves each
  selected card's own `CardRevealed → CardMoved → ObjectIncarnationAdvanced`
  block, so one selected card's reveal cannot invalidate the prior selected
  card's move in a batch replay.
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
- `LookAtTopCardsPutOneInHandOneOnTopRestOnBottom` is a one-effect,
  spell-only private decision boundary. It snapshots the current top up to
  its positive requested count in top-to-bottom order, retains the exact live
  stack spell and controller, and exposes those identities only to that
  controller's `PendingDecisionView`. A submitted
  `LibraryTopPartition` must use every snapshot card exactly once: one moves
  to hand, one of the remainder becomes the new top when present, and the
  rest are ordered bottom-to-top. A one-card library therefore requires no
  top card. Wrong-controller, stale-id, duplicate, foreign, or malformed
  partitions reject atomically. `DecisionOpened`, `DecisionCompleted`, and
  `PrivateLibraryTopPartitionResolved { inspected }` retain only safe
  metadata; no look, reveal, candidate, selected-top, or selected-bottom
  identity appears in the public receipt stream. The normal public hand move
  and terminal spell lifecycle follow only after the private decision closes.
- A `Permanent` target is a current battlefield object, never a player or a
  card in another zone. A permanent-bounce instruction snapshots the target's
  controller before its owner-hand zone move; its `CardMoved { to: Hand }`
  receipt therefore precedes the corresponding positive source-aware
  `LifeLost` receipt and the spell's terminal resolution lifecycle. An
  illegal target rejects atomically before costs, stack placement, or receipts.
- A `GraveyardCard` target is a current public card in any player's graveyard,
  captured with its current incarnation at activation. It is rechecked when
  the stack item resolves; a legal
  `PutTargetGraveyardCardOnOwnersLibraryBottom` transition uses the target's
  immutable owner rather than its activator or current controller, emits the
  ordinary `CardMoved { to: Library }` and incarnation-advance receipts, and
  inserts that card below every card already in that owner's library before
  the terminal ability receipt. A departed or re-entered target is illegal
  rather than a cross-zone no-op.
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
  declaration, so later combat bookkeeping preserves the exact declared pair
  without dereferencing a vanished token. Before blockers are declared, every
  blocker map, block-history record, ordered-damage group, departed-blocker
  marker, and evasion-qualified blocker set must be empty. An ordinary
  battlefield departure removes that permanent from *live* combat membership
  before its zone incarnation advances; historical block records remain only
  for delayed effects and must never make the priority transition roll back.
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
- Static and temporary landwalk are declaration provenance: every recorded
  attacker is in the unique declared-attacker set and retains a nonempty set
  of typed basic land types. A submitted blocker is rejected exactly when the
  fixed defender controls at least one recorded land type for that attacker.
  The provenance is cleared when that creature leaves combat or the combat
  declaration resets, so a later keyword or land change cannot rewrite the
  legality already recorded for that combat; a rejected block emits no
  `BlockersDeclared` receipt or partial combat mutation.
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
- A target-free all-combat-damage prevention record is valid only when it has
  a unique nonzero identity, an allocated provenance source identity, and an
  unexpired turn-bound lifetime. It is source-independent after the stack
  object resolves: the creating permanent may leave the battlefield or cease
  to exist without revoking prevention. Creation receipts must trace to a
  bound activated ability with the typed effect; every expiry receipt accounts
  for one prior creation, and creation counts equal live-plus-expired records.
  Each prospective combat-damage packet first respects the source's
  `DamageCannotBePrevented` status, then may emit exactly one
  `CombatDamagePrevented` receipt naming a live matching target-specific or
  global prevention source. Cleanup removes every current-turn global record
  and records one matching `GlobalCombatDamagePreventionExpired` receipt, so
  the prevention cannot leak into a later turn.
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
- A layer-five `ReplaceColorsWith` effect clears the currently derived color
  set and installs exactly one colored card color at its timestamp. It is not
  an additive grant, so a multicolor target is exactly the selected color
  until expiry; any later layer-five change still applies in timestamp order.
  The effect is target-incarnation-bound and therefore cannot affect a new
  creature object after the original target changes zones.
- Every continuous effect names extant source and target objects, has a unique
  positive monotonic timestamp, captures both endpoint incarnations, and has a
  valid duration. A permanent-duration effect cannot outlive either matching
  battlefield endpoint; an end-of-turn effect belongs to the current turn
  only and may retain its historical source after a spell has left the stack.
  In either duration, it cannot apply to a target that has left and returned.
- An end-of-turn effect created by a resolved instruction is independent of a
  former battlefield source. Source departure preserves it through that
  turn's cleanup; target departure still expires it immediately. In contrast,
  a permanent-duration effect remains source-dependent and expires when either
  matching battlefield endpoint leaves.
- `ShareControllerCreatureKeywordsUntilEndOfTurn` snapshots every controlled
  creature's derived keywords once before installing any layer-six effect. For
  each recipient, evidence comes only from a distinct controlled creature, so
  the recipient cannot copy its own ability and no newly granted keyword can
  cascade during the same resolution. Its declared typed family list is
  nonempty and duplicate-free; only those families are copied, preserving the
  exact `Protection(color)` and `Landwalk(type)` value. Every created grant is
  recipient-incarnation-bound and expires at that turn's cleanup.
- A layer-two control effect names one live battlefield permanent and is
  either seat-bound (`ChangeController`) or source-relative
  (`ChangeControllerToSourceController`). Active effects apply in timestamp
  order, so the most recent applicable effect determines `Game::controller_of`;
  a source-relative effect follows that source's live derived controller rather
  than baking in its caster. Its dependency graph must be acyclic, and every
  endpoint incarnation prevents a previous object from retaining control after
  a zone change. Installation records `ContinuousEffectCreated` then, when the
  derived controller actually changes, `ControllerChanged`. A battlefield
  departure snapshots the affected targets before source incarnation advances,
  then records expiration before every audited controller-reversion receipt.
  The base controller is never mutated.
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
- A target-bearing Equipment enter-the-battlefield trigger uses the same
  registered attachment binding, controller-relative target restriction,
  source/target incarnation checks, linked effect creation, and
  `EquipmentAttached` receipt as Equip. If there is no legal target when that
  trigger would be created, it is not placed on the stack and the Equipment
  remains a legal unattached permanent. It must never attach through a
  card-name exception or to an opponent's creature merely because one exists.
- The bounded linked-exile resolver intentionally stores no closures. Its
  typed group and delayed-action records remain invariant-valid while a sole
  target creature, or that creature plus its linked Auras, is suspended in
  exile, and the consuming end-step transition reaches the normal SBA boundary
  after returns. It is a substrate only: individual card promotion, arbitrary
  simultaneous delayed action ordering, and broader blink/zone-replacement
  interactions remain separate coverage work.
- Static continuous bindings are immutable expansion data, never timestamped
  runtime effects. Each registered binding names one supported static change
  and a creature or other compatible permanent definition. It applies only
  while an object with that definition is on the battlefield, creates no
  synthetic event receipt, and is reevaluated from live battlefield state
  whenever characteristics are read.
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
- Static top-library reveal bindings are immutable pregame data that name only
  permanent definitions. Duplicate, unknown, nonpermanent, or live-game
  registration is rejected atomically. A binding supplies public information
  only while at least one permanent using its effective definition is actually
  on the battlefield; copies therefore use their copied definition, while a
  departed source leaves no persistent or mutable reveal record behind. The
  binding scope is explicit: `EveryPlayer` projects every nonempty library
  top, while `SourceController` projects only the current controller's
  nonempty library top and follows live control changes. Both views are
  derived directly from owner-indexed library order; no historical top or
  hidden opponent identity may be cached.
- A controller-top creature color static layer reads that same controller's
  live current library top only when evaluating characteristics. It grants its
  layer-seven modifier only to controlled creatures that share at least one
  color with that top card, and only if the top card is a creature card. An
  empty library, a noncreature or colorless top card, a source departure, a
  control change, or any nonoverlapping color set immediately removes the
  contribution without an expiry receipt or an opponent-library probe.
- `PutTopCardOfControllerLibraryOnBottom` is a reorder of one live
  owner-indexed library, not a zone change: it takes the vector top and inserts
  that exact object at its bottom without changing its incarnation. A nonempty
  resolution records `LibraryTopMovedToBottom { player, card }`; it must name
  that player's current or subsequently `ObjectLeftGame`-proven card and be
  followed by its enclosing spell or ability resolution before another
  priority, step, or game-end boundary. An empty library is a legal no-op and
  produces no receipt.
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
  rules-counter records `SpellCopyCounteredByRules`. A resolving spell or
  ability that counters it instead records `SpellCopyCountered` with the
  physical or virtual counter source, again without a zone move. If its
  controller leaves a continuing multiplayer game, it is removed before that
  player's physical objects and records `SpellCopyLeftGame`; these are the
  only valid terminal lifecycle outcomes.
- A copy receipt has one unique nonzero identity and one immutable original /
  controller pair. Its receipt history must therefore show exactly one of a
  live virtual stack object, `SpellCopyResolved`, `SpellCopyCounteredByRules`,
  `SpellCopyCountered`, or `SpellCopyLeftGame`; no copy may be live after any
  terminal receipt or have two terminal receipts. A counter receipt cannot
  name the copy itself as its countering source. A scenario event-log reset is
  rejected while a virtual copy is live, preserving the opening `SpellCopied`
  provenance that the lifecycle audit requires.
- A copy whose target set may be changed suspends through the same monotonic
  `DecisionId` state machine as other public choices. Its options are public
  legal target values, and its cardinality is zero through the original
  spell's target-slot count. Zero selections is the explicit optional-decline
  branch: the virtual copy retains the original target values and captured
  target incarnations. A nonempty selection must replace every target slot.
  Its continuation can complete only while the copying spell is still on top
  of the stack with the same original-source provenance. A stale decision, an
  altered source, a partial retarget, or an illegal target rejects atomically.
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
must remain present. A reset is receipt-quiescent: while a typed
`PendingDecision`, activated/triggered stack ability, permission-timed spell,
linked-exile delayed action, attached Equipment, or terminal lifecycle remains
live, `clear_event_log()` is a deliberate no-op. It must never erase the
opening/creation receipt needed to validate or complete that live transition.

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
