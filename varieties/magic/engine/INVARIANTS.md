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
- The deterministic shuffle seed is fixture-construction state. It can be set
  only before `begin_game`; after that boundary, every library permutation
  comes only from a legal rules instruction and its corresponding
  `LibraryShuffled` receipt. A rejected live seed write is atomic and writes
  no receipt, so an external caller cannot silently choose a future search or
  shuffle outcome.
- A `DamageReplacementEffectBinding` registration batch is one immutable
  setup transition. Any unknown source, invalid source kind, duplicate effect,
  or later invariant failure restores the complete pre-call configuration.
  In particular, a rejected batch cannot make a subsequent valid retry fail
  due to a hidden prefix binding.
- A `ReplacementEffectBinding` registration batch has the same transactional
  boundary for token/counter quantity replacements. A rejected batch cannot
  retain a multiplier that a later valid registration then observes as a
  duplicate.
- Direct fixture primitives that mutate effects or attachments
  (`copy_permanent`, `add_continuous_effect`, and
  `enter_attachment_without_cast`) may construct a pregame fixture, and may
  remain available while the active player has priority in an existing
  compatibility fixture. Once a started game has passed priority to another
  player, each rejects atomically: it creates no layer, zone, attachment, or
  copy receipt and preserves the exact priority/state snapshot. Rules-owned
  copies, attachments, and continuous effects use their private resolving
  paths rather than this external fixture boundary.
- A cast first places every represented cast trigger using APNAP. If that
  placement opens a mandatory trigger-order or trigger-target decision, the
  decision player remains the priority holder until the no-priority decision
  completes; the caster does not regain priority merely because their cast
  completed. Without such a decision, the caster retains priority after the
  completed cast in the ordinary way.
- A trigger-order or trigger-target decision is never itself a priority
  action. The engine retains the exact priority holder immediately before the
  first suspended APNAP placement decision, preserves it across chained order
  and target selections, and restores it only after the whole placement batch
  has either stacked every trigger or discarded triggers with no legal target.
  An external state with a trigger-placement decision but no live saved holder,
  or a saved holder outside that exact no-priority boundary, is rejected.
- Every public APNAP `TriggerOrderEntry` retains one positive source identity,
  source incarnation, ability id, and a one-based occurrence position scoped
  to that live controller group. The occurrence distinguishes two legitimate
  same-source/same-ability triggers from separate simultaneous events; it is
  meaningful only with the enclosing monotonic `DecisionId`, which continues
  to reject stale ordering submissions. A submitted order must be an exact
  duplicate-free permutation of the pending group, and each ordered occurrence
  creates one independent triggered stack object.
- A game ends only when zero or one players remain. Its terminal transition
  emits exactly one `GameEnded { winner }` record (where `winner` is `None`
  for a draw). In a continuing multiplayer game, an eliminated player is
  skipped by turn order and may not hold priority or submit an action.
- `GameEnded` is the final canonical receipt. If an effect causes a terminal
  loss while a spell or ability is resolving, the resolver first records that
  stack object's final lifecycle receipt (`AbilityResolved` or the applicable
  source-departure receipt), then emits the single `GameEnded`; no later event
  may follow it.
- A terminal SBA fixed point discards trigger observations created by that
  same loss transition before APNAP placement. Such a trigger cannot produce
  a `TriggeredAbilityStacked` receipt or a new decision after the game has
  ended; pre-existing stack provenance remains frozen only for replay audit.
- Every public activated-ability and bound-mana-ability action reaches its
  post-cost SBA fixed point before placing any trigger observed from that
  action's costs. A cost trigger is stacked only if the game continues past
  that checkpoint; a terminal life payment or other cost-driven loss leaves no
  `TriggeredAbilityStacked` or `AbilityLeftGame` receipt for that unplaceable
  trigger.
- A stack instruction that records an empty-library draw during a private
  choice suspension must close the enclosing spell lifecycle before opening
  that choice. The loss marker is consumed at the following SBA boundary;
  no decision may be exposed to the eliminated player, and the terminal
  `SpellResolved`/source-zone receipt precedes `PlayerLost` and `GameEnded`.
- Trigger-order replay validators must resolve a triggered source's immutable
  card definition from the live object or its retained `departed_card_definitions`
  provenance. A source can leave the game during its own resolving ETB draw;
  auditing an unrelated trigger class must not dereference that departed
  object or roll back the otherwise valid terminal transition.
- On a player-loss transition, objects owned by that player leave this game
  and emit `ObjectLeftGame`; a non-owned object under that player's control is
  exiled to its owner. A stack-only virtual spell copy controlled by that
  player has no owner-zone move, so it instead ceases with one
  `SpellCopyLeftGame { copy, original, controller }` receipt. No departed
  player's object or virtual copy may later appear in a zone, on the stack, in
  combat, or in an effect. An activated or triggered ability controlled by a
  survivor is distinct from its departed physical source and remains on the
  stack with exact temporary LKI; a spell or an ability controlled by the
  departed player does not. Before each physical object is removed, every
  object-keyed last-known source-provenance record, graveyard or effect-created
  cast permission is revoked with its ordinary matching expiry receipt unless
  that survivor-controlled ability still needs it. A non-owned object exiled
  rather than removed retains its LKI provenance. A
  stack-only timing exception or
  exile-on-resolution marker for that object is cleared at the same boundary.
  A battlefield creature removed with its owner is not a death or an ordinary
  zone move, but surviving `AnotherCreatureLeavesBattlefield` observers are
  captured at its last-known battlefield state and stack normally after the
  player-loss SBA fixed point.
- A creature card's current-turn battlefield-to-graveyard eligibility is live
  zone state, not a claim that survives its owner leaving the game. Player
  departure removes that candidate before CR 800.4a deletes the object. Its
  earlier `CreatureCardPutIntoGraveyardFromBattlefieldThisTurn` receipt remains
  valid historical provenance only when immutable departed-card metadata and a
  later matching `ObjectLeftGame` receipt prove the ordinary graveyard
  transition occurred first.
- A virtual spell copy has a fresh stack-only identity, an immediate original
  identity for receipt provenance, and the immutable catalog definition it
  copied when it was created. Its immediate original may itself be a live
  virtual instant or sorcery, but the resulting ancestry must be acyclic and
  definition-consistent; a virtual predecessor must have an earlier,
  nonterminal `SpellCopied` receipt. Its copied definition, source
  incarnation, source colors, and controller must agree with its live stack
  object and are retained as immutable provenance after that item is popped,
  so damage, prevention, and replacement paths never reconstruct facts from
  the physical original. It retains copied decisions (modes, targets, X, and
  explicit color choices), but has no cast-payment receipt: `mana_spent`, Convoke cost
  symbols, and generic cost reductions are zero/absent. The lower physical
  original may resolve, be countered, or leave the game before the copy; that
  departure cannot invalidate a copy controlled by a surviving player or make
  the copy read a later incarnation of the original card.
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
  captured exile incarnation is removed from the group; ordinary source
  departure without such a pending return emits `LinkedHandExileExpired` and
  cannot leave unreachable private state. CR 800.4a owner departure always
  emits that expiry before removing the source, even when a return instruction
  was pending: that same transition removes the departing owner's stack
  object, so it can no longer authorize a return. `HandExiledWithSource`,
  `LinkedHandExileReturned`, and expiry receipts name nonempty, unique public
  object identities while ordinary zone/incarnation receipts remain the
  authoritative transition sequence.
- A source-linked creature-exile group is keyed by one exact live battlefield
  source incarnation and contains only nonempty, duplicate-free creature-card
  members still in `Exile` at their captured incarnations.  An exile departure
  removes that exact member and a source departure removes the whole group, so
  a later incarnation cannot claim an earlier exile entitlement.  A target-free
  return instruction opens a public exactly-one current-member choice only
  while that exact source and at least one candidate remain; otherwise it is a
  legal no-op.  The selected object returns through its ordinary zone move and
  a permanent layer-two control effect, retaining its owner as base controller
  while deriving control from the resolving source controller.
- A delayed linked-exile action has a nonzero unique action identity, typed
  `EndStep` timing, and a due turn no earlier than the current turn. It is
  consumed exactly once with its group, removing both from suspended state.
  `DelayedActionScheduled` names the exact exiled members, while
  `DelayedActionConsumed` records only the duplicate-free subset that was
  still eligible to return. Those receipts are replay-audited against their
  schedule; ordinary `CardMoved` and incarnation receipts remain zone truth.
  Every returned member captures its self and applicable controller-scoped
  entry observations after its final attachment state but before the delayed
  action's shared SBA fixed point; the batch stacks only after that checkpoint,
  retaining the returned battlefield incarnation if SBA immediately moves it.
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
- An attachment-relative end-step trigger is valid only for a registered
  creature Aura with a target-free, source-relative effect. It is placed only
  after the attached creature's controller begins their own end step, even
  when that controller differs from the Aura's controller. The queued ability
  retains the Aura's exact source incarnation while using the attached
  creature's controller as its controller. Its attack condition is keyed by
  the exact creature battlefield incarnation declared as an attacker this
  turn; a zone change, reattachment, or source departure therefore cannot
  satisfy the condition with a reused object ID. A new Untap step clears this
  turn history. The public trace is `StepBegan(End) →
  TriggeredAbilityStacked → SacrificedByEffect → CardMoved → AbilityResolved`.
- An Aura-relative combat-damage-to-player trigger observes only its exact
  currently attached creature incarnation and remains sourced and controlled
  by the Aura. A positive committed combat `DamageDealtToPlayer` receipt starts
  one contiguous `AttachedCombatDamageTokenCountCaptured` group: every member
  preserves that packet's creature, recipient, and amount; every Aura ability
  incarnation appears at most once; and every capture preserves its own
  Aura/creature incarnations and bound trigger identity. The capture must match
  a registered target-free token-count trigger and a count representable by the
  token receipt surface. Prevention or a zero packet creates neither capture
  nor trigger; a later zone change or reattachment cannot revise the captured
  count. Materialization yields only the captured count of token receipts
  before that ability's terminal event.
- A non-token object has exactly one catalog definition; a token has exactly
  one token specification and exists only on the battlefield. An object cannot
  be both, and no nonpermanent card can occupy the battlefield.
- When a player leaves the game, a token they control but do not own leaves
  the battlefield and immediately ceases to exist. This is not an exile or
  graveyard move: it emits `TokenCeasedToExist` without a `CardMoved` or an
  incarnation advance, and may observe only ordinary leaves-the-battlefield
  triggers, never dies-only triggers.
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
- Immediately before each zone transition, the engine freezes one private
  last-known `Characteristics` record and source-controller record under that
  object's exact former `(ObjectId, incarnation)`. The two maps have exactly
  the same keys; every record must normally name an existing strictly older
  incarnation, except the exact source of a live ability controlled by a
  survivor after the physical source left the game. That exceptional pair is
  pruned at the first later SBA fixed point once the ability has a terminal
  receipt. No characteristic record may contain `Colorless` as a card color.
  Live battlefield sources use their current characteristics and controller only
  when their exact incarnation equals the resolving stack source; any departed
  or re-entered source instead uses those immutable former facts for
  source-quality and controller-relative damage-prevention rules, including
  `Deathtouch` and `DamageCannotBePrevented`. A later graveyard, exile, or
  re-entered incarnation can therefore neither lose nor invent a quality or
  controller relationship for an already pending effect's damage packet. The
  same exact source incarnation is mandatory while discovering, opening,
  validating, applying, and resuming a prospective damage-replacement
  decision; an affected player can never be offered prevention against
  historically unpreventable damage.
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
- A target-free controller-team regeneration instruction snapshots only the
  resolving controller's live battlefield creatures at its own resolution
  boundary. It creates exactly one source-identified shield and
  `RegenerationShieldCreated` receipt for each snapshot member, never for an
  opposing creature or a later entrant. Its retained source may already be in
  a graveyard after paying a self-sacrifice activation cost; source departure
  does not erase an already-created shield, while the ordinary live-target
  shield invariant still governs each replacement endpoint.
- A targeted damage-prevention shield is private replacement state with a
  positive remaining amount, a seated player or exact live permanent target,
  a current-turn expiry, and a retained source identity. A permanent target's
  exact battlefield incarnation is captured when the shield resolves: its
  later card-type change does not erase the already-resolved effect, but a
  zone departure retires it before the stable `ObjectId` can return in a new
  incarnation. A player target is retired if that player leaves the game.
  Creation is a stack effect and emits `DamageShieldCreated`; the source may
  have left the battlefield as an activation cost, or later leave the game
  with its owner, without invalidating the shield. In the latter case an
  `ObjectLeftGame`/`TokenCeasedToExist` receipt is sufficient historical source
  provenance, but source characteristics are never dereferenced. Damage
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
- A bounded temporary damage-redirection effect retains the exact battlefield
  incarnation of its protected permanent and, when its destination is a
  permanent, that destination's exact battlefield incarnation too. Both
  permanent endpoints must remain current battlefield objects for the effect
  to be live; an endpoint's ordinary zone departure retires the replacement
  before the stable `ObjectId` can return in a later incarnation. A player
  destination has no object incarnation but is retired if that player leaves
  the game. The resolved effect remains independent of a later source
  departure, and a surviving endpoint may change card types without creating
  a new target identity.
- A permanent static source may grant
  `PreventDamageFromControlledSources` to each current creature its controller
  controls. The quality is evaluated for every prospective permanent-damage
  packet against the damage source's exact controller provenance, so it
  prevents only friendly-source damage and leaves an opposing controller's
  source unchanged. A virtual copied spell uses its immutable stack
  controller, never the physical original's controller.
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
- A chosen-X Radiance damage-and-life instruction snapshots that same target
  plus shared-color creature set before any damage packet commits. Its
  aggregate is the sum of only its bounded batch's positive committed
  `DamageDealtToPermanent` or redirected `DamageDealtToPlayer` receipts after
  replacement and prevention; the declared X and prevented prospective
  packets cannot inflate it. A positive aggregate emits
  `DamageBatchLifeGained` with the exact source incarnation immediately before
  an equal `LifeGained` receipt for the resolving controller. A zero aggregate
  emits neither receipt nor a synthetic life trigger.
- A source-side combat-damage prevention record has a unique positive id, a
  current-turn expiry, a retained creating-source identity, and one exact live
  battlefield permanent incarnation. It is created only while its target still
  satisfies the typed attacking-or-blocking creature requirement, emits
  `CombatDamagePreventionCreated`, prevents every positive combat packet from
  that exact creature while it remains a combat source through
  `CombatDamagePrevented`, and cannot follow a leave-and-return incarnation.
  A later card-type change on that same permanent does not erase the
  already-resolved record. Its effect is independent of a later source
  departure, which retains historical source identity without a live-object
  dereference. Cleanup or target departure removes it with
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
  color cannot evade protection during target revalidation, ordinary damage
  prevention, or the prospective replacement-candidate/application/resumption
  path for one suspended direct or global damage packet. Nonmatching-color and
  colorless sources remain unaffected. A
  matching-color Aura becomes illegally attached when protection is gained,
  then the ordinary SBA moves it to its owner's graveyard and expires every
  attachment-linked continuous effect.
- A bounded prospective damage event carries source incarnation, target
  incarnation (when the target is a permanent), affected player, remaining
  amount, and the exact replacement identities already used. For any exact
  targeted `DealDamage` instruction of a represented stack spell or ability, the
  engine gathers live target-shields, permanent shields/protection, and
  bounded redirections before it records any damage. The suspension check uses
  that exact stack object's frozen source colors rather than re-reading a
  physical source, so the identical choice boundary applies to a stack-only
  virtual copy. Two or more candidates
  open the same public, id-bearing `DecisionKind::Replacement` boundary used
  by quantity replacement; its options are `ReplacementChoice::Damage` values
  visible only to the affected player. The continuation captures the immutable
  stack item, current effect index, and target occurrence, so a
  prefix/damage/suffix spell cannot fall through to deterministic direct
  damage. The submitted identity must still be live, is applied once, and
  applicability is recomputed. The stack spell remains live while this
  decision is pending. `DamageReplacementChoiceView` projects that exact
  current `DecisionId`, and the legacy `ChooseDamageReplacement` action must
  echo it as well as the packet's source/incarnation/target identity; an old
  answer cannot consume a later identical packet from the same resolving
  spell. New policies may instead use `SubmitDecision`. Its public
  `DamageReplacementApplied` receipt
  precedes the authoritative `DamagePrevented`, `DamageRedirected`, or
  committed `DamageDealt*` receipt, and only committed positive damage queues
  damage triggers. The generic decision closes after that causal damage batch,
  then resumes only the unresolved suffix or terminal lifecycle.
- The same decision boundary applies to every snapshotted packet of a
  represented untargeted global-damage instruction:
  `DealDamageToEachCreatureAndPlayer`, `DealDamageToEachPlayer`, or
  `DealDamageToEachNonFlyingCreature`. The resolver captures the exact current
  recipient projection before any packet commits: every living player for the
  player forms, every current creature for the all-recipient form, and only
  current non-Flying creatures for the latter form. Singleton packets may
  commit first, but a later recipient with two or more live replacements
  suspends the original stack object with that recipient as the sole decision
  player. Its remaining original packets are duplicate-free, positive, and
  incarnation-audited separately from a partial redirect's new-recipient
  packets. The continuation's global-effect shape is checked against this same
  represented family before it resumes. After the selected packet and any
  redirect remainder finish, the batch resumes the exact remaining snapshot
  once, then skips the already resolved global instruction before its suffix or
  terminal stack lifecycle.
- A registered damage-amount replacement has a catalogued permanent source and
  is fixed before the game begins. Every live source applies once to a
  prospective player or permanent packet only when its typed predicate holds,
  and retains its source incarnation in `DamageReplacementApplied`. A halving
  replacement then writes the immediately following `DamageAmountReplaced`
  arithmetic receipt, whose positive input and bound reduced output name that
  same source/target. A prevention-aware self replacement applies only when
  its exact live source is also the permanent recipient and the incoming
  damage may be prevented; it writes
  `DamageReplacementApplied → DamagePreventedWithPlusOneCounters →
  DamagePrevented`, followed only by counter-placement replacement receipts
  and its matching source `CounterPlaced { PlusOnePlusOne }`. The counter
  total is at least the prevented amount (and may be larger only through the
  independently audited quantity-replacement chain). Direct and combat paths
  use the same live-source discovery and an unpreventable packet never offers
  this prevention candidate.
- A registered source-bound combat replacement may replace that source's
  positive combat-damage packet to a player with immediate non-damage
  consequences. `CombatDamageReplacedWithMillAndCounters` records the exact
  source incarnation, damaged player, and replaced positive amount; it must
  be followed by the matching source `+1/+1` counter placement and no
  intervening ordinary player-damage receipt from that source to that player.
  Any resulting library moves retain their ordinary `CardMoved` and
  incarnation receipts. When it competes with another represented
  amount-changing combat replacement (currently a live halving source or
  targeted player shield), the engine opens a public, affected-player
  `DecisionKind::Replacement` boundary before either consequence commits.
  The typed continuation freezes the exact source incarnation, damaged
  player, positive packet amount, already-used replacement identities, and
  remaining assigned player-damage packets. It applies each selected identity
  at most once, recomputes the live candidates after every choice, and keeps
  the rest of combat-damage assignment unchanged until the packet reaches
  zero or has no candidate. Only then can the queued packet suffix, state
  actions, and triggers continue. Target-specific and target-free global
  all-combat-damage prevention records participate with their exact record
  ids and historical sources; a selected prevention record produces
  `DamageReplacementApplied → CombatDamagePrevented`, zeroes only that
  prospective packet, and is marked used without consuming its turn-bound
  record. `DamageCannotBePrevented` excludes those prevention candidates but
  not non-prevention amount replacements. Other replacement-effect classes
  remain outside this bounded continuation, so the invariant does not claim a
  complete replacement-event algebra.
- `Keyword::DamageCannotBePrevented` excludes prevention only. It bypasses
  target shields, permanent shields, protection, and color-based prevention,
  but does not bypass a non-prevention damage redirection. A redirected event
  receives a new target and is then reconsidered against that recipient's
  applicable replacements; one source-bound replacement identity cannot
  apply twice to the same prospective event.
- The current decision continuation is intentionally narrow: it supports a
  targeted direct-damage stack instruction, the represented untargeted global
  damage instruction family above, and bounded redirections that may split the
  pending event. A partial redirect commits its
  new-recipient packet first and retains the protected remainder as a
  deterministic deferred packet inside the same no-priority stack
  continuation. Every deferred packet has a positive amount, a live matching
  target incarnation, a valid affected player, and unique prior replacement
  identities before it can be resumed; replacements are re-evaluated at each
  recipient. Optional replacements and a fully general replacement-event
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
  before the game begins, and its complete registration batch is atomic: a
  later duplicate or invalid member cannot retain an earlier entry rule. It is
  rechecked from the battlefield at each
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
  a physical cost. Its immutable pregame source-cost registration batch is
  atomic, so a rejected later member cannot retain an earlier sacrifice
  requirement that changes the corrected configuration. The policy submits
  every selected output color bundle
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
  opponent's land. A selected target retains its exact incarnation through
  the response window, must still be a controlled land at resolution, and
  then uses the ordinary owner-indexed hand transition before the terminal
  ability receipt.
- A `LandEntersBattlefield` or `ControlledLandEntersBattlefield` trigger binding
  is permitted only on a permanent source and uses the same checked
  effect/target shape as every other trigger. Every represented land entry
  first makes the land live, reaches its ordinary state-based-action boundary,
  queues that land's own ETB triggers, then scans every live permanent with
  an effective represented definition, including a token copying a card. The
  first condition observes every entry; the second only stacks
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
- A targeted enchantment-return instruction may resolve only for the exact
  current battlefield incarnation of a permanent with the `Enchantment` card
  type. It performs one ordinary owner-indexed `Hand` transition, including
  attachment cleanup, before its spell or ability terminal receipt. The target
  controller is never substituted for the owner; a stale, departed, or
  non-enchantment target yields the ordinary no-op/skipped-target resolution
  path and cannot fabricate a hand move.
- A target-pair instruction owns its ordered target slots as one stack effect:
  stack target count, target-incarnation provenance, and the registered target
  requirements must include both occurrences. Any relation between the pair is
  checked before the decision completes and again when that instruction
  resolves. If either target or the relation is no longer legal, the entire
  paired instruction is a no-op; it cannot create a one-sided control change.
  A completed permanent creature-control exchange creates both layer-two
  effects before either `ControllerChanged` receipt. Each affected permanent
  sources its own durable control effect, so the original spell or ability
  source leaving the battlefield cannot undo either completed half, while a
  later departure of one exchanged permanent expires only that permanent's
  effect. Ownership-indexed zones never change during the exchange.
- Every generic migrated no-priority choice occupies the one typed, clonable
  `PendingDecision` state slot. Its positive `DecisionId` is strictly less
  than the next monotonic id, is never reused after completion, names one
  living deciding player, and has distinct typed options with a valid
  inclusive min/max cardinality. `DecisionOpened` and `DecisionCompleted`
  form one unique, matching public receipt lifecycle per id; neither receipt
  contains hidden candidate or selection identities.
- `DecisionKind::CleanupDiscard` is the active player's private, no-priority
  CR 514.1a boundary when their hand exceeds the engine's default maximum of
  seven cards. Its exact ordered hand snapshot (including object
  incarnations), object-only options, and equal min/max excess count must
  remain live while the stack is empty and Cleanup's repeat marker is set.
  Only that active player may submit the current `DecisionId`; an invalid,
  stale, foreign, duplicate, or wrong-cardinality answer is atomic. A valid
  answer emits `DecisionCompleted`, then one `CardDiscarded` and ordinary
  `CardMoved { to: Graveyard }` receipt per chosen card before damage removal,
  end-of-turn expiry, state-based actions, and any exceptional CR 514.3
  repeat. No opponent view or public decision receipt exposes the hand's
  candidates or selection.
- The specialized optional-trigger compatibility boundary also consumes that
  same monotonic `DecisionId` space even though its accept/decline projection
  is not a cardinality-based `PendingDecision`. Its current positive id must
  be lower than the next allocation, belongs to the exact top triggered stack
  object, and is visible only to its controller alongside the legal payment
  and conditional-target facts. `ResolveOptionalTriggeredAbility` must echo
  that exact id; source and ability names alone are not sufficient because
  one permanent can put indistinguishable trigger instances on the stack at
  different times. A stale, foreign, or malformed response leaves the pending
  choice, stack, zones, mana, and event log unchanged.
- The specialized controller-private top-library choice also consumes that
  same monotonic `DecisionId` space even though it is not represented by the
  generic cardinality-based `PendingDecision`. Its positive id is lower than
  the next allocation, belongs to the exact top spell and controller, and is
  visible only to that controller with the private inspected-card snapshot.
  `ChoosePrivateLibraryCards` must echo that exact id: a source `ObjectId`
  alone is insufficient because the physical spell may leave a zone and be
  recast with a new stack incarnation. A stale, foreign, or malformed answer
  leaves the suspended spell, private snapshot, zones, life, and event log
  unchanged.
- The specialized controller-private opponent-library exile choice likewise
  consumes the shared monotonic `DecisionId` space even though it is not a
  generic `PendingDecision`. Its positive id is lower than the next allocation,
  names the exact top activated ability, controller, opponent, source, and
  ability identity, and is visible only to that controller with the private
  candidate snapshot. `ChoosePrivateOpponentLibraryCardToExile` must echo it:
  source and ability alone do not distinguish an old activation from a later
  activation after the same permanent leaves and re-enters. A stale, foreign,
  or malformed answer leaves the suspended ability, private snapshot, zones,
  and event log unchanged.
- `PolicyAction::SubmitDecision` must supply the currently live exact id and a
  selection whose type, cardinality, uniqueness, and options match the typed
  continuation. A stale id, different player, duplicate, out-of-range count,
  or illegal option rejects atomically. While the decision remains open,
  priority cannot pass or interleave, the suspended stack item remains stable,
  and no other pending decision family may coexist. Compatibility actions for
  older policies dispatch through the same continuation but new policies must
  use the id-bearing generic action.
- `LibrarySearchChoiceView` is a compatibility projection of a live generic
  library-search decision, never an anonymous second boundary. Its
  `ChooseLibrarySearchCard` compatibility action must echo that exact
  `DecisionId` in addition to the source identity before it dispatches to the
  generic continuation. A stale action after a search spell leaves a zone and
  is recast must leave the newer pending decision and stack spell untouched.
- `TriggeredAbilityTargetChoiceView` is likewise only a projection of a live
  `TriggeredAbilityTargets` generic decision. Its
  `ChooseTriggeredAbilityTargets` compatibility action must echo that exact
  `DecisionId` as well as source/ability identity; two otherwise-identical
  trigger instances from one persistent source cannot share an answer. A
  stale action leaves the later target choice, suspended trigger, and event
  sequence unchanged.
- `TriggeredAbilityEffectObjectChoiceView` is only a projection of a live
  `TriggeredEffectObject` generic decision. Its
  `ChooseTriggeredAbilityEffectObject` compatibility action must echo that
  exact `DecisionId` as well as source/ability identity. In particular, an
  empty selection from one multi-player discard trigger cannot complete the
  same source's later empty-hand choice or advance its next chooser. A stale
  action preserves the later choice, continuation, and event sequence.
- `DecisionKind::PublicGraveyardCreatureReturn` retains one exact target-free
  spell stack object while its affected living players choose serially from
  their own public graveyards. Only a player with one or more current creature
  cards receives a required one-card choice; a player with no candidate is
  skipped without a fabricated prompt. The continuation records the stack
  object/source incarnation, the ordered remaining-player suffix, and every
  earlier selected card's owner and incarnation. Before terminal resolution,
  the invariant rederives the current chooser's legal options and validates
  the full stack shape, public visibility, zero-pass boundary, unique living
  player queue, and every stored selected snapshot. Completion revalidates
  every selected card in its owner's graveyard before moving them all to hand,
  then records the ordinary spell terminal lifecycle. No priority window,
  insertion-order fallback, or stale same-`ObjectId` graveyard incarnation can
  change a player's required choice.
- `DecisionKind::PublicGraveyardLandReturn` retains one exact target-free spell
  stack object while its controller selects zero through three of that
  controller's own public graveyard land cards. The current legal options are
  rederived from owner, zone, and printed land type at both decision audit and
  submission; the response is an in-range duplicate-free subset, and only its
  selected card incarnations are moved to hand after `DecisionCompleted`.
  The spell/source stack identity, source incarnation, controller, zero-pass
  boundary, public visibility, and `0..=min(3, candidates)` cardinality must
  all remain exact. Empty candidates open no fabricated prompt and resolve as
  the ordinary no-op instruction. No priority action, zone-order fallback, or
  stale same-`ObjectId` graveyard incarnation can add a land to the return.
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
- `DecisionKind::CounterUnlessDiscardsHand` retains the same exact top
  counterspell and lower *physical spell* source/target incarnations, but has
  no hidden candidate list: only the lower spell's living controller may
  explicitly choose `discard: true` or `discard: false` through the public
  no-priority boundary. The legal true branch snapshots that controller's
  complete current hand after stale stack validation (an empty hand is a
  legal zero-card branch), records `CounterUnlessDiscardHandChosen` directly
  before the matching `DecisionCompleted`, then emits each ordinary
  `CardDiscarded`/graveyard move before the counterspell's terminal receipt.
  The false branch emits the same choice/completion pair then counters only
  the captured lower physical spell. No automatic discard, automatic counter,
  foreign controller, ability-stack target, self-target, reordered stack
  target, malformed selection, or failed card move may mutate stack, hand,
  decision, or public receipts.
- `DecisionKind::TargetPlayerManaColor` retains one exact current instruction
  of a live stack item, including its immutable `effect_index` and target
  occurrence. The decision belongs to that captured recipient rather than the
  resolving controller, is public, requires exactly one answer, and offers
  precisely the represented five card colors that can still fit in the
  recipient's bounded mana pool; it never offers `Colorless`. The source,
  source incarnation, controller, ability identity, stack item, target, and
  current option set are revalidated before completion, so a foreign, stale,
  malformed, noncolored, or capacity-overflow answer is atomic. A nonzero
  cursor may pause an otherwise ordinary prefix/choice/suffix stack item; the
  chosen color is a short-lived typed materialization owned by that exact
  instruction, consumed once by the normal resolver, then emits one
  `ManaAdded` receipt after `DecisionCompleted`. It cannot appear in a
  catalog definition or permanent ability binding, cannot overwrite an
  earlier/future instruction, and no materialization may escape the live
  stack boundary.
- `DecisionKind::TargetPlayerSacrificeCreatureThenControllerDrawsEqualToPower`
  retains one exact non-ability spell stack item with one still-legal player
  target and the matching sole target-player sacrifice instruction. It is a
  public, recipient-owned, exactly-one creature choice: candidates are
  rederived from that target's currently controlled battlefield creatures and
  retain each exact object incarnation. The deciding player cannot choose an
  opponent's creature, while the resolving controller cannot choose for the
  target. Completion revalidates the stack id/source incarnation/controller,
  target occurrence, effect shape, candidate set, and chosen incarnation;
  only then does it capture the selected creature's nonnegative current power,
  record `SacrificedByEffect`, perform the ordinary owner-graveyard move, and
  emit exactly that many controller draw moves before the spell's terminal
  receipt. If the legal target controls no creature, no impossible decision is
  opened and the ordinary instruction performs no draw. A stale answer, zone
  round trip, foreign creature, extra/missing selection, fabricated direct
  resolver call, or wrong stack shape is atomic and fails the invariant audit.
- This first unified-decision migration covers policy-submitted one-card
  library searches, trigger target selection and triggered discard/sacrifice
  object choices, multi-block combat order, spell-copy targets, concurrent
  token/counter quantity replacement ordering, and bounded direct-damage
  prevention/redirection, plus private top-library hand/top/bottom and
  target-player top/bottom partitions. Library search, discard, and
  top-library partition options are
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
  enclosing action. The captured controller is the source's live
  `controller_of` value at the trigger event, never its owner-bound base
  controller, so a layer-two stolen source's generic leave or dies trigger
  belongs to the player who controls it at that moment. A target-bearing event stays outside the stack in its
  generic target decision until its controller chooses every legal target;
  later events cannot overtake that pending placement. Dynamic damage-trigger
  instructions use the captured positive amount, not a later damage
  accumulator. A targetless optional trigger opens the same accept/decline
  boundary even with a zero mana cost. The choice is controller-only: a
  rejected noncontroller submission is atomic and writes no policy or effect
  receipt, while a decline writes the ordinary terminal `AbilityResolved`
  receipt but never opens an effect-specific continuation. A target-bearing optional trigger first
  completes its public target-selection boundary before it is stacked. After
  ordinary priority passes, its all-illegal target plan is checked before any
  optional payment prompt: an all-illegal ability records
  `AbilityCounteredByRules` and opens no optional decision. Otherwise it opens
  the same controller-only accept/decline boundary; declining preserves the
  already selected target without applying its instruction. A decline is
  consumed before any effect-specific
  suspension: it emits the trigger's sole terminal `AbilityResolved` receipt
  without opening a hidden-zone, replacement, or other deferred decision.
  The represented one-effect all-player-
  discard and controller-creature-sacrifice triggers keep their stack object
  live while the relevant chooser submits a legal current hand or battlefield
  object; no deterministic fixture selection may move a card or permanent.
  A controller-creature-sacrifice candidate and its final payment both query
  `controller_of`, never the owner-bound base controller, so a player may
  choose a creature they currently control through a layer-two effect; its
  normal owner-indexed graveyard move then ends the temporary control effect.
- `CastsCreatureSpell` is controller-scoped: after a physical creature
  spell's `SpellCast` receipt and before its caster receives priority, each
  live controller-owned observer queues its normal trigger above that spell.
  An opposing controller's creature cast queues none. A stacked trigger keeps
  its captured source, controller, and incarnation provenance if its source
  subsequently leaves; its targetless optional decision remains the captured
  controller's, and an accepted draw resolves before the underlying creature
  spell.
- `AnyPlayerCastsCreatureSpell` is deliberately distinct from the
  controller-scoped condition. After any physical creature spell's
  `SpellCast` receipt, every live bound observer may capture the exact cast
  card, its positive stack incarnation, and its public name before priority
  returns. A matching-graveyard return trigger materializes only from that
  payload into one target-free effect that retains all three facts. The
  resolver requires the captured physical creature spell to remain below the
  trigger with the same incarnation and definition/name; a later zone
  incarnation or arbitrary same-named card cannot supply provenance. Binding
  and live-stack audits reject targets, an unmaterialized marker, a
  materialized binding effect, or zero/empty captured provenance.
- A simultaneous matching creature-card return snapshots every living
  owner-indexed graveyard before any return moves, moves the complete matching
  set to the battlefield, then applies each entrant's entry replacements from
  the pre-event battlefield plus that entrant alone. Own ETB triggers are
  captured for every entrant, while every controlled-nonartifact observer is
  sampled from one post-event battlefield snapshot. Thus replacement effects
  still cannot let one newcomer change how another entered, but every
  newcomer observes each permanent in the one entry event as CR 603.6a
  requires; ordinary SBAs and trigger placement occur only after the complete
  batch commits.
- An `BeginningOfAnyUpkeep` trigger is stacked only after that upkeep's own
  `StepBegan` receipt and before its first priority window. Its active player
  is captured into a materialized `SacrificeCapturedPlayerCreature` stack
  effect; it is never recomputed from the trigger source's controller or a
  later turn state. The corresponding public trigger-effect decision belongs
  to precisely that captured player, offers only creatures they currently
  control, and rejects a submission from the trigger controller when that is
  another player. The stack-shape audit rejects an unmaterialized template, an
  out-of-range captured seat, or an any-upkeep stack receipt without the
  immediately preceding upkeep boundary.
- An `BeginningOfAnyEndStep` trigger is stacked only after that end step's
  own `StepBegan` receipt and before its first priority window. Its active
  player is captured into a materialized
  `SacrificeCapturedPlayerUntappedLand` stack effect and never recomputed from
  the trigger source's controller or later turn state. The corresponding
  public trigger-effect decision belongs only to that captured player and
  offers only their currently controlled, untapped lands. A stale, tapped,
  opponent-controlled, or wrong-player submission is rejected without a zone
  mutation; when none are legal, the ability resolves as a no-op without a
  fabricated zero-option decision. The stack-shape and step-boundary audits
  reject an unmaterialized template, an out-of-range captured seat, or an
  any-end-step receipt without its immediately preceding end-step boundary.
- A `BeginningOfControllerEndStep` trigger queues only at the End boundary
  for its source's current controller; the captured stack controller must
  equal that active player even if the source later changes controller or
  leaves the battlefield. Its current-turn creature-return effect selects
  only non-token physical creature cards owned by that controller whose exact
  current graveyard incarnation was put there from the battlefield this turn.
  The ordinary `CardMoved(Graveyard)` and
  `ObjectIncarnationAdvanced` receipts are immediately followed by one
  `CreatureCardPutIntoGraveyardFromBattlefieldThisTurn` receipt. The live
  candidate set is keyed by `(owner, card, graveyard incarnation)`, removes a
  card on any graveyard departure, and clears at the next Untap boundary
  (including a forced next turn after active-player elimination). Resolution
  snapshots only still-matching candidates before moving any of them to hand;
  a final-zone lookalike, a token, an earlier incarnation, or a preexisting
  graveyard creature cannot be returned. `clear_event_log` is unavailable
  while this history is live, preserving the receipt provenance that the
  invariant audit requires.
- A `BeginningOfAttachedCreaturesControllerEndStep` trigger uses the same
  end-step boundary but queues only while its exact typed Aura attachment is
  live and the attached creature's current controller is active. The source
  Aura remains immutable receipt provenance; the attached creature's
  controller owns the stack object. Its target-free attached-creature effect
  rechecks both endpoint incarnations at resolution and becomes a no-op if
  either endpoint is gone or detached.
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
  emits `AbilityManaPaid`; decline executes no effect instruction or deferred
  decision, and rejected submissions are atomic no-ops.
- A registered generic-cost reducer has a catalogued permanent source, a
  strictly positive amount, and is registered before the game starts. It
  contributes only while a source with that definition is live on the casting
  player's battlefield, changes generic symbols only, and is applied after an
  explicit chosen-X value but before mana or Convoke payment. Its setup
  registration batch is atomic: an unknown, invalid, or duplicate later
  member cannot retain an earlier reducer that changes a corrected retry's
  legality. A
  `CastsNoncreatureSpell` trigger retains the exact triggering spell in its
  one `NoncreatureSpell` target slot, stacks above that spell after `SpellCast`,
  and either opens one mandatory public `TriggeredEffectObject` choice for its
  resolving controller's current creatures or emits `SpellCountered` followed
  by the target's ordinary terminal move when that option set is empty. The
  pending continuation retains the exact source, ability, and underlying
  noncreature spell; it rejects stale/missing spell provenance, an altered
  candidate set, non-controller answer, decline, duplicate, or non-creature
  selection atomically. The successful choice records
  `DecisionCompleted → SacrificedByEffect → CardMoved →
  ObjectIncarnationAdvanced → AbilityResolved` while leaving the retained
  spell on the stack. No battlefield-order fallback is legal.
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
  or accepted policy receipt. `PolicyAction::CastWithColorChoice` and the
  atomic `PolicyAction::CastWithModeAndColorChoice` retain the exact five-color
  value on the spell stack object and record one matching `SpellColorChosen`
  receipt before `SpellCast`. The latter also records its selected
  `SpellModeChosen` branch in that same cast transaction; no policy can cast a
  modal card and provide its color in a later action. The chosen-color
  requirement is derived from the materialized stack effects, never an
  unselected `ChooseOneOf` wrapper. The choice is neither inferred from mana
  spent nor silently defaulted. An activated or triggered ability cannot
  fabricate this spell-only provenance; a virtual copy instead keeps the
  original spell's retained color. Resolution uses only the retained value to
  install controller-team temporary protection or a target's temporary
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
  occurrences. A token or land cannot occupy the stack as a spell, but either
  may be the source of an activated or triggered ability. A copied token's
  effective layer-one definition, rather than a physical printed definition,
  remains authoritative for that ability's binding and replay validation. A target may later
  become illegal, but it cannot be absent, fabricated, or change enum kind
  after cast time. `Target::Spell` additionally retains and validates the
  immutable card definition after CR 800.4a removes that object from the live
  game. Its typed boundary remains exact: `InstantOrSorcerySpell` accepts only
  an instant or sorcery, `NoncreatureSpell` excludes creature cards, and
  `Spell` accepts any represented physical spell card (including permanent
  spells), never an activated or triggered ability.
  The invariant validates that immutable target shape separately from dynamic
  target legality. Target shape and occurrence order are always derived from
  the immutable stack effect list, so a chosen modal branch cannot be checked
  against its unmaterialized `ChooseOneOf` wrapper. In particular, every stack
  player target names a seated player, although that player may later have lost. At resolution, an all-illegal
  target set emits the matching spell or ability `…CounteredByRules` receipt;
  if at least one target remains legal, the stack item resolves and only
  instructions addressed to the now-illegal
  target slots do nothing. The engine snapshots initial legality in a
  `StackResolutionPlan` before it begins any effect to establish the
  all-targets-illegal boundary. An initially legal slot is rechecked before its
  own instruction, so an earlier instruction that removes a repeated target
  cannot make the complete resolution fail. If a later instruction suspends
  the same resolving stack item for a no-priority decision, that initial
  effect-aligned plan remains attached to the exact nonzero cursor until the
  item reaches one terminal stack lifecycle; resumption must never rerun the
  all-targets-illegal boundary against effects already begun. Each skipped instruction emits its own
  `TargetInstructionSkipped { effect_index, target }` diagnostic receipt. A
  resolving counter effect emits the distinct `SpellCountered` receipt. Land,
  controlled-land, and artifact requirements read the target's current
  layer-four type set for both casting and that resolution recheck, rather
  than its printed definition; a creature made into a land or artifact can be
  legally targeted and resolved by the matching typed instruction.
- A ranged target group is one expansion-neutral stack instruction with a
  single zero-through-maximum occurrence range. Its cast-time members must be
  distinct `GraveyardCard` object targets from one immutable owner-indexed
  graveyard, and each retains an ordinary target-incarnation receipt. An empty
  group has no target and resolves normally; a nonempty group with no legal
  member is countered by rules; otherwise each still-legal member resolves in
  submitted order and every departed member emits its own
  `TargetInstructionSkipped`. Mixed ranged/fixed target instructions and
  malformed range, shape, duplicate, or cross-graveyard membership cannot
  survive the stack invariant audit.
- `TargetedBundle` owns one printed target occurrence for an ordered sequence
  of target-preserving instructions. Its outer requirement and one captured
  target incarnation are the only cast-time and resolution-time legality
  boundary; bundle members do not allocate, duplicate, or independently
  recheck target slots. The members resolve in declared order only after the
  outer target remains legal. An all-targets-illegal bundle is countered by
  rules as one stack instruction, so a broader trailing member cannot act on
  a stale target. Empty, nested, ranged, zone-changing, deferred-decision, or
  multi-target members are rejected identically at spell, activated-binding,
  and triggered-binding definition validation; every accepted member must be
  compatible with the outer target requirement. The directional implication
  matrix admits only sound refinements: creature restrictions entail
  `Creature`/`PlayerOrCreature`/`Permanent`, land restrictions entail
  `Land`/`Permanent`, artifact/enchantment unions entail `Permanent`, and
  player restrictions entail `Player`/`PlayerOrCreature`; it never weakens a
  member's controller, color, zone, or stack restriction. The public event
  log retains ordinary effect receipts but has no synthetic duplicate target
  identity.
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
  implied characteristic rule. Before creature toughness or lethal-damage
  checks in every state-based-action pass, CR 704.5q cancels the minimum
  opposing `+1/+1`/`-1/-1` pair count on each permanent. A stable battlefield
  object therefore never has both positive counts; each cancellation emits one
  source-free, positive `CounterPairsRemovedByStateBasedAction` receipt.
  Every `CounterPlaced` and `CounterRemoved` receipt names a valid kind and a
  positive quantity. Placement applies the prospective quantity-replacement
  pipeline before mutation; source-based removal never does. Removal
  preflights the live counter balance and, if insufficient, atomically restores
  the resolving stack object, event log, and counter map.
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
  from sacrifice-cost materialization: after an exact
  `TriggeredAbilityOrder` decision it reads the source's current counter total
  while that exact battlefield incarnation remains live, preserving the effect
  of an earlier simultaneous counter trigger. If that source departs while the
  sweep remains pending, the departure boundary replaces only the matching
  stack template with an immutable nonnegative mana value and emits a
  `SourceCounterValueMaterialized` receipt immediately before the source's
  normal `CardMoved`/incarnation-advance pair. The audit binds each receipt to
  its registered upkeep trigger, counter kind, source incarnation, and any
  still-live materialized stack object; orphaned or mismatched materialization
  fails. Resolution snapshots the complete creature recipient set before any
  destruction, and a later re-entry cannot donate a new counter total to the
  old stack object.
- A registered quantity replacement has a catalogued permanent source, a
  multiplier of at least two, and is fixed before the game begins. When tokens
  are created or a represented counter is placed, only live battlefield
  sources controlled by the affected player apply. Every candidate and
  `ReplacementEffectApplied` receipt records the source's exact positive
  incarnation; the same source incarnation/effect cannot apply twice to one
  prospective event. Any stack token/counter instruction with two or more
  live candidates—whether it is the first, middle, or final instruction—opens
  a public `DecisionKind::Replacement` boundary for the affected player. The
  immutable stack effect list and target occurrences remain the cast-time
  provenance source; a private nonzero `StackObjectId → effect_index` cursor
  is valid only for that live top stack item and its matching
  `QuantityReplacement` or `TargetPlayerPrivateDiscard` continuation. It
  executes the already-resolved prefix exactly once, holds priority closed
  while the current replacement or recipient-private discard is chosen, and
  resumes only the unresolved suffix. Any initial target-legality snapshot
  sharing that stack identity must have exactly one effect-aligned entry per
  immutable effect, cannot outlive the cursor or top stack item, and is
  cleared with the item's terminal receipt. A targeted-discard continuation binds
  its cursor to the corresponding target occurrence. An event-captured
  combat-player discard instead binds that cursor to the exact positive final
  `DamageDealtToPlayer` recipient, never a target occurrence. Either form
  gives only its recipient the current-hand candidates, revalidates their
  exact incarnations, commits the selected discards before resuming, and
  cannot expose or deterministically select a hidden card for the resolving
  player. The submitted option is revalidated,
  applied once, and candidates are recomputed; a fresh monotonic decision id
  opens only while two or more choices remain, while one remaining candidate
  applies without a prompt. `DecisionCompleted`/`DecisionOpened` and a
  submitted policy receipt may appear between causal replacement receipts, but
  the replay audit still requires a finite same-event chain ending in exactly
  the resulting `TokenCreated` batch or `CounterPlaced` receipt. Direct
  non-suspended helper paths use the same live candidate/application logic in
  stable order. If that cursor resumes an optional triggered ability, it is
  proof that the initial optional decision was already accepted and its prefix
  resolved: only an untouched cursor-zero stack item may open the optional
  accept/decline boundary or charge its resolution-time mana cost. A resumed
  suffix must preserve that accepted decision, never reopen or repay it, and
  emit the one ordinary terminal ability receipt.
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
  `GameView`; opponents see no candidate identities. The opening
  `CardsLookedAt` receipt records only the viewer, current stack source and
  incarnation, and inspected count—never hidden card identities—and must
  exactly agree with the live private snapshot. Its controller-private fresh
  `DecisionId` must be positive and already allocated. The snapshot must still be
  exactly the controller-owned current library top sequence, priority must
  stay with that controller with zero passes, and no draw replacement may
  coexist. A submitted selection is unique and a subset of that snapshot,
  checks the controller's life before mutation, then atomically records
  `LifePaid`, moves each selected card to hand and each other inspected card
  to graveyard, and only then records the spell's terminal resolution and
  source-zone receipts. No priority action or pass can interleave.
- A private opponent-library exile choice is a one-effect, targeted
  activated-ability suspension with a positive inspection count. Its
  controller-private fresh `DecisionId` is positive and already allocated. Its live top
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
  submitted `PolicyAction::Draw` echoes the active player's fresh monotonic
  `DecisionId`; it cannot reuse an earlier draw step's answer. The id is
  controller-private, paired one-to-one with the live marker, positive, and
  already allocated. A stale, foreign, or malformed draw action leaves the
  marker, library, hand, and event log unchanged. A successful receipt follows
  the resulting draw or dredge events.
- `GameView` never exposes an opponent's hand or ordinary hidden library, nor
  does it expose a departed player as an opponent life-total or battlefield
  target. A live registered static top-library reveal source is the narrow
  exception: every seated policy view receives exactly one `CardView` for the
  current final element of each nonempty owner-indexed library, with no second
  card or cached prior top. The projection is recomputed from live battlefield
  source definitions and ordinary library zones, so draws, shuffles, zone
  changes, and source departure immediately change or revoke it without a
  synthetic visibility receipt. An ordinary view never projects a matching
  card from any hidden library, including its own controller's library merely
  because that controller holds a Transmute card. A suspended typed library
  search instead projects only its resolving controller's matching candidate
  identities; opponents receive no candidate list or selected-card identity
  before a public reveal or ordinary zone-move receipt. The exported canonical event log
  follows the same boundary: private inspection receipts contain public
  provenance/count metadata only, while a later public zone move or explicit
  reveal is the first receipt that can name a hidden card.
- While a draw replacement is pending, `GameView` projects only the deciding
  player's fresh decision identity and legal owned-graveyard dredge candidates.
  A policy can take the normal draw or choose one of those candidates; it
  cannot name a hidden or unpayable replacement.
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
  provenance. `CardView::can_attack` applies the same exception and the
  current `CannotAttackOrBlock` restriction, so policies neither hide a legal
  Haste attack nor propose a creature the declaration boundary must reject.
  Non-Haste attack and tap-cost rejection are atomic: they do not tap the
  source, create combat state, add mana, or write accepted-action receipts.
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
- A generalized graveyard-exile cost has an exact positive card count from
  its immutable binding. Each selected object must be distinct, currently in
  the activating player's own graveyard, and a creature card; it is rejected
  before mana, stack, or event mutation otherwise. Each successful selection
  emits `ExiledFromGraveyardAsAbilityCost` immediately followed by the same
  card's `CardMoved { to: Exile }` receipt before `AbilityActivated`. The
  invariant audit rejects an orphaned, duplicate, wrong-owner, noncreature,
  wrong-cardinality, or non-exile receipt.
- A stack-using activated ability captures its controller at activation in the
  immutable stack object and matching `AbilityActivated` receipt. A later
  layer-two control change to its live battlefield source does not change or
  invalidate that historical controller: effects resolve for the captured
  activator, while ordinary source zone and target-incarnation checks continue
  to govern legality independently. In particular, a valid response that
  steals the source must not make the response transition fail its invariant
  audit or retarget the already-activated ability.
- Every engine-created nonmana activated stack object also retains the exact
  definition that supplied its binding. The invariant requires a matching
  `AbilityActivated` receipt with the same source, source incarnation,
  definition, and ability id before consulting that immutable binding. This
  survives an intervening layer-one copy effect: a copied permanent can retain
  a named physical-source ability without the stack audit incorrectly looking
  for that ability on its copied definition. Spells and trigger stack objects
  retain `None` and use their existing source-definition provenance.
- Every activated-ability sacrifice cost is represented by explicit, distinct
  policy-selected controlled battlefield permanents in binding order: source
  sacrifices first, then the configured number of creatures, then lands. The
  engine validates cardinality, control, current zone, and each required type
  before mana, zone, tap, stack, or receipt changes. Each successful selection
  records `SacrificedAsAbilityCost` immediately followed by its graveyard move
  (or token-ceases receipt), before `AbilityActivated`; a rejected selection is
  an atomic no-op with no mana debit or cost receipt.
  When one activation cost selects two or more permanents, their
  battlefield-to-graveyard departures are one simultaneous event for generic
  `AnotherCreatureLeavesBattlefield`, `AnotherCreatureDies`,
  `ControlledNontokenCreatureDies`, and `OpponentCardPutIntoGraveyard`
  observation. Every selected observer is frozen at its exact pre-cost
  battlefield incarnation before the first receipt; the individual ordered
  cost moves suppress only duplicate generic observation, never their normal
  zone/incarnation or source-specific Dies lifecycles.
- A target-free self-regeneration activation that consumes one controlled
  creature has no target slot and never shields the sacrificed offering. Its
  source identity and incarnation are captured at activation; only that same
  current battlefield creature can receive `RegenerationShieldCreated` during
  resolution. The event order is therefore one selected-creature
  `SacrificedAsAbilityCost` and its zone receipt before `AbilityActivated`,
  then ordinary priority receipts, then the source-to-source shield receipt and
  `AbilityResolved`; if the source is no longer the captured creature, the
  resolver creates no substitute shield.
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
  target cannot be dropped or replaced by an empty target vector. Every
  represented simultaneous creature-death batch, including an SBA sweep or
  one destroy-all instruction, freezes every represented
  `AnotherCreatureLeavesBattlefield`, `AnotherCreatureDies`,
  `ControlledNontokenCreatureDies`, and owner-indexed
  `OpponentCardPutIntoGraveyard` observer before any member changes zones.
  This includes an otherwise dying observer's own last-known battlefield
  incarnation, live controller, and colors; ordinary individual zone moves
  then do not duplicate those generic observer events. Source-specific Dies
  triggers and normal zone/incarnation receipts remain separate ordinary
  lifecycles.
  A color-specific attacking-creature modifier is valid only as a target-free
  `Attacks` trigger whose complete effect bundle consists of nonzero modifiers
  for ordinary card colors. On resolution, it samples the exact current
  declared-attacker set, filters each candidate by live creature type and
  color, and creates one independent layer-seven end-of-turn effect per
  matching color. A same-colored nonattacker, departed attacker, or later
  re-entry cannot become a recipient; every installed effect retains exact
  target-incarnation provenance and expires through cleanup with its ordinary
  receipt.
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
  Self-ETB and represented entry-observer triggers capture their source while
  the entering permanent is still its live battlefield object, before any
  resulting state-based actions. They remain pending until the normal SBA
  fixed point and then stack in the shared trigger placement pipeline. Thus a
  zero-toughness creature may move to its graveyard before its ETB trigger is
  stacked, but that `TriggeredAbilityStacked.source_incarnation` must name the
  `ObjectIncarnationAdvanced` receipt immediately following its
  `CardMoved(Battlefield)` event, never the later graveyard incarnation.
  The conditional graveyard-to-battlefield creature-return instruction shares
  this capture-before-counter-and-SBA boundary; it cannot treat a returned
  creature as a bare zone move or reconstruct its trigger from a later
  graveyard incarnation.
- A resolving all-player discard trigger keeps its stack object live while
  each living player with a hand submits a private current-hand selection in
  player order. Every `CardDiscarded` receipt is immediately followed by that
  exact card's `CardMoved { to: Graveyard }` receipt; activation discard costs
  remain separately represented by `DiscardedAsAbilityCost`. A mandatory
  discard trigger has no policy decline at this boundary.
- A resolving controller-creature sacrifice trigger with legal candidates
  keeps its stack object live while its controller submits one public current
  controlled-creature selection. Its `SacrificedByEffect` receipt is
  immediately followed by either that card's graveyard move or a token's
  `TokenCeasedToExist` receipt; the no-candidate case remains a valid
  resolution and cannot roll back the trigger.
- A prospective counter-placement event belongs to the target permanent's
  live `controller_of` player. Both the direct single-replacement path and
  the multi-replacement decision path therefore discover the same currently
  controlled replacement sources; an owner-bound base controller cannot skip
  or select a multiplier for a stolen permanent.
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
- `PolicyAction::CastWithCreatureSpellAdditionalMana` is the one policy-facing
  boundary for a live static source that permits optional extra mana while a
  creature spell is cast. Its immutable pregame modifier registration is an
  atomic batch, so a rejected later member cannot retain an earlier live
  optional-payment source. Each source is a distinct live battlefield object,
  may appear at most once, and captures its exact current incarnation before
  any mana leaves the controller's pool. A noncreature spell, an empty payment,
  a duplicate/departed/foreign source, or a source without the registered
  modifier rejects atomically. Every accepted source emits one positive
  `CreatureSpellExtraManaPaid` receipt in a contiguous group immediately
  before that cast's ordinary payment receipt (if any) and `SpellCast`; no
  priority or unrelated cast can interleave. The matching pending entry state
  belongs only to that exact physical live creature spell, is consumed after
  its battlefield entry and before trigger stacking/state-based actions, and
  places the same
  number of `+1/+1` counters through ordinary counter replacement. It is
  removed on every counter, terminal-zone, or player-departure path, so neither
  a later incarnation nor a direct/noncast battlefield entry can inherit it.
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
- A chosen-X instruction targeting a player may use that immutable stack
  value as a resolution quantity without re-reading mana: it keeps the one
  target through ordinary all-illegal-target checking, mills up to that many
  current library cards in order, then records exactly that declared amount as
  the resolving controller's `LifeGained`. A short or empty target library
  never reduces the retained life-gain amount, while zero X creates neither a
  zero-value life-gain receipt nor a fabricated zone transition.
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
  ability is never taxed. Its pregame registration batch is atomic, so an
  invalid or duplicate later member cannot silently retain an earlier modifier
  that alters a corrected activation-cost configuration. Every live source
  applies once to any player's
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
  activated ability before the game begins. Its whole registration batch is
  atomic, so a later duplicate or invalid profile cannot retain an earlier
  generalized cost. Its concrete choices arrive only
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
  exact instruction cursor of a resolving spell or activated-ability stack
  item with zero passes in a private `PendingDecision`; only that controller
  sees the ordered matching candidates. A multi-instruction stack item keeps
  its prefix effects committed and resumes its suffix exactly once after the
  choice, while a terminal search emits the ordinary spell/ability lifecycle
  once.
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
  card's move in a batch replay. `LibraryTop` is the narrow exception: it is
  legal only for a revealed batch search. Its selected cards remain in the
  library (no `CardMoved` or incarnation advance), are removed before the
  unselected remainder is shuffled, and are restored in submitted
  top-to-bottom order. Its exact public receipt sequence is
  `CardRevealed ×N → LibrarySearchBatchResolved → LibraryShuffled →
  LibrarySearchTopCardsPlaced`; the auditor rejects an unrevealed, duplicate,
  missing, reordered, or orphaned ordered-top receipt.
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
- `PutControllerHandOnLibraryBottomThenDrawSameCount` is a one-effect,
  target-free triggered-ability boundary for `CastsSpell`. It observes either
  a creature or noncreature spell from its current controller exactly once and
  carries no implicit spell target. On resolution it snapshots that
  controller's complete hand and exposes the identities only through one
  private `HandToLibraryBottomDraw` decision pinned to the source and source
  incarnation. The submitted selection must be the exact, duplicate-free
  snapshot at its fixed cardinality; stale, foreign, partial, duplicate, or
  changed-zone answers are atomic rejections. The selected order is
  bottom-to-top. The engine commits it through reverse ordinary library moves,
  then begins exactly that many ordinary draws. `DecisionCompleted` precedes
  the identity-free `HandPutOnLibraryBottomThenDrawn` receipt, followed by
  matching library moves and at most that many hand moves (a short library
  follows the ordinary empty-library-loss path), before the matching
  `AbilityResolved`. No priority action, candidate identity, or selected
  order escapes the private boundary.
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
- `LookAtTopCardsOfTargetPlayerAndReorder` is a one-effect, spell-only
  private decision boundary with one live `Player` target. On resolution its
  controller, rather than the target player, alone receives the exact current
  top-first snapshot of up to its positive requested count. A nonempty
  snapshot opens `TargetPlayerLibraryTopReorder`; an empty target library
  resolves normally without inventing a choice. Its submitted `top`
  (top-to-bottom) and `bottom` (bottom-to-top) vectors must together be an
  exhaustive duplicate-free partition of that exact snapshot. The live stack
  spell, source incarnation, controller, target, target legality, snapshot,
  and private option set are revalidated before either library order changes;
  foreign, stale, malformed, duplicate, or omitted-card answers are atomic.
  `PrivateTargetPlayerLibraryReorderOpened { decision, count }` immediately
  follows its matching private `DecisionOpened`, and matching
  `DecisionCompleted` must precede one identity-free
  `PrivateTargetPlayerLibraryReordered { inspected }` receipt and the normal
  terminal spell lifecycle. A virtual copied reorder spell follows the same
  private snapshot and partition rules, but ends only with
  `SpellCopyResolved`; it never emits `SpellResolved` or attempts a physical
  card-zone transition for its stack-only identity. Candidate identities and submitted order never
  enter the target player's view or the public event log.
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
  transition. A draw-replacement marker and its paired positive
  already-allocated decision id can exist only for the active player at the
  Draw-step decision boundary; neither can outlive that boundary, point at an
  eliminated player, or exist without the other.
- Once the game has ended, gameplay actions, public draw replacements, public
  continuous-effect installation, and the setup-to-live `begin_game`
  transition are rejected atomically, without changing state or emitting
  accepted-action events. Setup hooks remain deliberately separate from
  gameplay methods.
- The complete `begin_game` transition is atomic even when deferred
  cross-binding validation rejects setup. In particular, a missing typed Aura
  attachment binding for an attachment-relative trigger leaves `started`,
  turn/step/priority state, and the event log exactly pregame, so expansion
  setup may register the missing binding and retry the start without a hidden
  partially-started state.
- State-based-action and pending-trigger placement failures always propagate
  as `RulesError`; they never panic after committing a prefix. The public
  state-based-action checkpoint is journaled, while internal checkpoints stay
  within their caller's transaction, so a failed deferred trigger projection
  leaves no partial step, death, zone, or trigger receipts behind.
- Every entry coin-flip binding setup batch is atomic. An unknown source,
  invalid heads/tails characteristic override, or duplicate later binding
  leaves no prefix entry replacement behind, so the corrected batch can be
  registered before the game begins without changing a future entry outcome.
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
  policy action in either is rejected. The narrow CR 514.3 exception is
  explicit and invariant-checked: if Cleanup's automatic expiry/SBA work
  creates a trigger stack item or trigger-placement decision, only that
  Cleanup state receives priority. After all resulting stack work ends, the
  engine clears mana and repeats Cleanup once before advancing to Untap; this
  repeat marker may not escape the Cleanup step.
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
  larger `OpeningHandDrawn` event. Deck loading is likewise pregame-only and
  transactional: it preflights the exact `u16` receipt cardinality before
  creating any object, so `DeckLoaded` and `LibraryShuffled` never saturate or
  claim fewer cards than the library contains. Direct pregame `add_card` and
  every later owner-library zone move share that same capacity boundary;
  attempting to create a 65,536th library card rejects before it changes an
  object, zone, seed, or canonical event. The invariant rejects any fabricated
  oversized library, and every rules shuffle takes its receipt through the one
  checked exact-count helper. Setup events and hidden cards cannot be injected
  into a live turn.
- Once the game has begun, the public direct-draw primitive is legal only for
  the active player's pending Draw-step decision. It resolves that marker
  atomically; an arbitrary Upkeep, main-phase, combat, or opponent draw is
  rejected without changing a zone or canonical receipt. An authored fixture
  that deliberately reaches a pending Draw step before `begin_game` likewise
  consumes that marker when it resolves its ordinary draw, rather than leaving
  a stale mandatory decision to block the next priority window.
- The entire public direct-draw transition is atomic, including its pregame
  Dredge compatibility marker. A rejected Dredge source restores the pending
  marker and shared decision-id allocator, so the first real Draw decision is
  not observably renumbered by a failed setup helper.
- A successful public Dredge is itself one complete Draw replacement: its
  ordered mill, graveyard-to-hand move, and trigger flush complete before it
  clears the matching mandatory marker. No invariant-valid state can retain a
  second ordinary draw after the replacement has succeeded.

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
- The public attacker-declaration boundary is transactional. Any downstream
  failure while creating its mandatory triggered-ability placement or public
  decision rolls back every tap, combat provenance field, pending trigger, and
  `AttackersDeclared` receipt; a rejected declaration is indistinguishable
  from not having been attempted.
- Vigilance is attacker-declaration provenance: it records exactly the
  attackers that had Vigilance when they were declared and is a subset of the
  live declared-attacker set. It explains why those attackers did not tap at
  declaration, but it is never compared with a later layer-six keyword query:
  a legal continuous effect may add or remove Vigilance later in the same
  combat without rewriting the completed declaration or tapping an attacker
  retroactively.
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
- A must-block availability scan evaluates the same current blocker legality
  as a submitted assignment: it considers every battlefield creature derived
  to be controlled by the defending player, rather than their owner-indexed
  battlefield vector, and excludes every creature barred by that attacker's
  current landwalk. It never forces an impossible block merely because a
  matching basic land is controlled or an owner no longer controls their card.
- A source-relative block requirement identifies one exact live attacker and
  one exact defending creature.  It is enforced only at the blocker-declaration
  snapshot: when that creature can legally block that attacker, the submitted
  assignments contain that exact pair; if it cannot legally do so, the effect
  creates no impossible requirement.  A related combat-only target is legal
  both when activated and when resolved only while it is blocking, or blocked
  by, that exact live source in the current combat.
- The next seated defending player is fixed when attackers are declared; only
  that player declares blockers. Each blocker is a unique untapped creature
  they control; every assigned attacker was declared; and each attacker may
  retain an ordered list of distinct blockers. After the post-attackers
  priority window, an attacker's *current* Flying accepts only a blocker with
  current Flying or Reach; current Fear accepts only a black or artifact
  blocker; current RAV black-only evasion accepts only a black blocker; and
  current `Unblockable` accepts no blocker. The same blocker-declaration
  snapshot applies to current landwalk and must-block restrictions. The combat
  state records that completed declaration-time projection only after its
  legality check, so a later characteristic change cannot rewrite legal block
  history while a pre-blockers change has its normal rules effect. If that
  defender leaves the game, the declared attackers are removed from combat
  rather than being retargeted to another surviving seat.
- The public blocker-declaration boundary is transactional through required
  attacker damage-order setup. A rejection while encoding one order decision
  restores the exact pre-block state: it leaves no blocker map or history,
  blocker-declaration projection, `BlockersDeclared` receipt, or pending
  combat-order choice behind.
- Declare-blockers cannot begin without an attacker declaration, and combat
  damage cannot begin without both declarations. A participant may leave after
  declaration, so later combat bookkeeping preserves the exact declared pair
  without dereferencing a vanished token. Every *live attacker* remains a
  battlefield object; any ordinary departure, SBA token-cessation, or
  CR 800.4a owner-departure removal removes that attacker and its live blocker
  group before its object record or zone incarnation disappears. A current live
  blocker assignment requires its
  current exact incarnation in history; a departed blocker or ceased token
  instead requires a matching immutable declared pair and may have no
  remaining `CardObject`. Before blockers are declared, every blocker map,
  block-history record, ordered-damage group, departed-blocker marker, and
  evasion-qualified blocker set must be empty. Historical block records remain
  only for delayed effects and must never make the priority transition roll
  back.
- Every layer-two controller transition records `ControllerChanged` and then
  removes that permanent from live combat membership before the next combat
  decision or damage batch. A changed attacker is removed with its live blocker
  group and retires every live blocker qualification/first-strike marker for
  that group while preserving immutable `CombatBlockHistory`. A changed blocker
  is retained only as historical block provenance, so the attacker remains
  blocked but neither former participant assigns combat damage as a consequence
  of the changed control.
- `unblockable_attackers` is completed blocker-declaration provenance only:
  it is a subset of the uniquely declared attackers and no blocker map entry
  may name one of those attackers. A rejected block writes no
  `BlockersDeclared` receipt.
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
- Static and temporary landwalk are blocker-declaration provenance: every
  recorded attacker is in the unique declared-attacker set and retains a
  nonempty set of typed basic land types sampled after the post-attackers
  priority window. A submitted blocker is rejected exactly when the fixed
  defender controls at least one currently relevant land type for that
  attacker. The provenance is cleared when that creature leaves combat or the
  combat declaration resets, so a later keyword or land change cannot rewrite
  the legality already recorded for that combat; a rejected block emits no
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
- Every represented combat-damage packet, whether its current recipient is a
  defending player or a blocked creature, enters the same stackless
  prospective-event state machine before commitment. A concurrent choice
  retains the source's exact live incarnation, current target/incarnation,
  duplicate-free replacement history, any partial-redirection packets, and
  every remaining permanent and player assignment in deterministic combat
  order. The recipient's affected player alone owns the public replacement
  decision; no `DamageDealt*`, `DamagePrevented`, or `DamageRedirected` receipt
  may commit before that decision. When a selected redirect changes the
  recipient, the new packet recomputes its affected player and replacement
  candidates. Every direct or continuation-resumed combat assignment derives
  `DealsCombatDamageToCreature` provenance from each positive final
  `DamageDealtToPermanent` receipt in that assignment, not from its original
  blocker target. `DealsCombatDamageToPlayer` likewise derives its exact
  player recipient only from a positive final `DamageDealtToPlayer` receipt.
  Prevention and redirection that leave no corresponding final receipt queue
  neither trigger; player-captured effects may not reopen a public target
  choice or substitute a later player identity.
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
  those sources cannot assign again in normal combat damage. A separate
  resolved marker proves that the first-strike batch reached its shared
  SBA/trigger boundary; removing an attacker from combat also removes its
  first-strike source record without making a completed step appear
  unprocessed. An already-removed blocker can retain its ordinary
  declared-pair history but never assigns later damage. State-based actions run
  after the first-strike batch, so a lethal blocker does not remain to assign
  later normal damage. If no participant has first strike, the extra step is
  absent rather than an empty priority window.
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
- A token created as a permanent copy is still a token for zone/departure
  rules, but its layer-one `CardDefinition` values remain authoritative for
  characteristics, definition-bound triggers, and immutable static metadata.
  A controller-upkeep attachment trigger captures the exact attached creature
  and incarnation before it enters the stack; a later attachment move or zone
  round trip cannot redirect the token copy to a newer object. When one copy
  effect creates one or more tokens, the full token batch shares one
  post-entry snapshot for self ETBs, controller-scoped entry observers, and
  one land-entry event per copied land; a copied creature–land token can
  therefore trigger itself and the original represented creature–land from
  the same APNAP-orderable batch. A definitionless ordinary token has no
  definition-bound self ETB, but still participates in that one entry event:
  all represented nonartifact/Aura/land-entry observers see it, and each Land
  token produces its own represented land-entry event. The same copied
  layer-one definition is authoritative at every upkeep and end-step boundary:
  a copied token queues its matching controller, any-player, or eligible
  attachment-relative turn trigger exactly as a non-token copy source does.
  End-step replay validation resolves copied or departed source provenance
  before classifying the receipt, so no copied token trigger is silently
  exempt from the event-boundary audit.
- Every registered legendary card definition, and every legendary token value,
  participates in CR 704.5j according to its current layer-one name. When a
  controller has two or more matching live permanents, the engine has exactly
  one public `LegendRule` no-priority decision with every current member as a
  unique candidate and cardinality one. The selected permanent remains; every
  other captured member moves in one simultaneous state-based action with its
  exact incarnation checked before the move. Until that decision completes no
  priority action may interleave, and a continuing started game may not retain
  an illegal legendary group without that live decision boundary.
- A registered entry-copy permanent opens a public zero-or-one source decision
  only while its physical spell remains the exact top stack item. Every option
  is a live permanent of the registered type; a selected source snapshots its
  exact copiable values and source incarnation, while an empty selection
  resolves the printed permanent. No priority action can interleave with this
  replacement-style boundary, and a stale source, entrant, option, or copied
  value rejects atomically.
- The selected snapshot is installed as layer one only after the entrant's new
  battlefield incarnation exists and before copied static entry rules or ETB
  triggers are captured. If those values are an Aura, the source choice admits
  it only with a current legal endpoint, then a separate required public Aura
  endpoint decision completes before the entrant reaches the battlefield. The
  entry therefore cannot expose an unattached copied Aura to a state-based
  action or priority boundary; its typed attachment uses the copied values'
  target restriction and colors.
- Continuous effects are applied in the implemented layer order (1, 2, 4--7).
  Timestamped type/color effects are evaluated before every static layer-six
  or layer-seven binding, so a static creature/color predicate observes the
  already-derived layer-four/five characteristics. Static bindings then run
  before timestamped ability/P/T effects, preserving the bounded substrate's
  existing same-layer precedence; timestamped effects remain timestamp-ordered
  within each phase. End-of-turn effects expire during cleanup; marked damage
  clears there. An effect removed because its source or target leaves the
  battlefield emits an explicit expiration lifecycle receipt.
- A layer-five `ReplaceColorsWith` effect clears the currently derived color
  set and installs exactly one colored card color at its timestamp. It is not
  an additive grant, so a multicolor target is exactly the selected color
  until expiry; any later layer-five change still applies in timestamp order.
  The effect is target-incarnation-bound and therefore cannot affect a new
  creature object after the original target changes zones.
- A typed `LandWithBasicLandType` target is legal only for a live battlefield
  land whose currently derived basic-land type matches exactly; activation and
  resolution both recheck that live characteristic. `AnimateTargetLand` admits
  a nonempty, noncolorless replacement color set, one or more creature
  subtypes, and nonnegative base power/toughness. It creates its creature and
  subtype changes in layer four, complete color-set replacement in layer five,
  and base power/toughness in layer-seven 7b. Later layer-seven modifiers and
  counters apply after that base setting.
- A target-free source animation may instead install a layer-7b base P/T
  value equal to the **current** number of creature cards in one captured,
  living player's graveyard. The effect captures the resolving controller's
  `PlayerId`, not the source's later controller; it counts only public
  non-token card definitions whose card types include `Creature`, and derives
  the value whenever characteristics are read. An ordinary graveyard move
  therefore updates the value without a synthetic layer receipt, while a
  noncreature card does not contribute. The effect is valid only for the
  exact live source incarnation on the battlefield and expires at that
  resolving turn's cleanup with its ordinary type, color, and P/T lifecycle
  receipts.
- Every continuous effect names extant source and target objects, has a unique
  positive monotonic timestamp, captures both endpoint incarnations, and has a
  valid duration. A permanent-duration effect cannot outlive either matching
  battlefield endpoint; an end-of-turn effect belongs to the current turn
  only and may retain its historical source after a spell has left the stack.
  In either duration, it cannot apply to a target that has left and returned.
- The sole source-object exception is an explicitly marked virtual spell copy:
  it may be created only while that copy resolves, only with the current-turn
  `EndOfTurn` duration, and only against the target's exact live battlefield
  incarnation. The frozen virtual source must have no physical object or
  retained live-copy record once resolution finishes. It cannot create a
  source-relative controller change, attachment-controller redirection, or
  activated-ability grant, because those effects need a live source. Cleanup
  expires the remaining source-independent effect normally and records its
  ordinary lifecycle receipt.
- A target-side damage-prevention shield uses the same explicit virtual-source
  boundary when an instant or sorcery copy creates it. It retains a nonzero
  unique shield identity, legal live player-or-creature target, positive
  remaining amount, and current-turn expiry, but neither a fabricated
  physical source nor a lingering virtual-copy map entry. Its source is
  informational receipt provenance only; consuming or expiring the shield
  never requires the former spell object to exist.
- Targeted and global combat-damage prevention records follow the same rule:
  their virtual source is allowed only during resolving-copy construction and
  becomes historical receipt provenance once the copy resolves. A targeted
  record additionally retains the exact live creature incarnation; both
  records have a unique positive identity and expire in the creating turn.
  The global-prevention receipt audit accepts only a matching bound ability,
  physical spell, or `SpellCopied` provenance whose catalog definition
  contains that exact effect.
- A virtual spell copy may suspend for a private library choice. The pending
  choice and `CardsLookedAt` receipt use the live stack item's immutable
  source incarnation rather than a physical-object lookup; only its
  controller receives the candidate identities. When the submitted choice
  completes, the virtual stack item emits `SpellCopyResolved` and is removed
  from copy provenance without a fabricated `SpellResolved` or card-zone
  movement.
- The same terminal rule applies to a virtual copy's public
  `LibraryReorder` decision: its complete `DecisionCompleted →
  LibraryReordered → SpellCopyResolved` lifecycle removes only stack/copy
  provenance. The selected order is committed atomically before the terminal
  copy receipt, and never authorizes a physical zone move for the virtual id.
- The same terminal rule applies to a virtual copy's private
  `TargetPlayerLibraryTopReorder` decision. Its private target-library order
  is committed before `SpellCopyResolved`; neither `SpellResolved` nor a
  physical terminal-zone move may name the virtual source.
- A virtual copy's private `LibraryTopPartition` completion follows that
  exact terminal boundary after its private hand/top/bottom movements. The
  hidden selected identities remain absent from public receipts except for
  ordinary revealed zone transitions; the terminal source is always
  `SpellCopyResolved`, never `SpellResolved` or a physical card-zone move.
- A virtual copy of a may-retarget copying spell may open its own public target
  decision, create a child virtual copy with the selected target provenance,
  then emit its own `SpellCopyResolved`; parent and child each have one
  independent terminal lifecycle and neither may move a physical spell zone.
- A virtual copied policy-submitted library search retains controller-only
  hidden candidate visibility. It commits the selected physical card, records
  exact search/shuffle receipts, then emits `SpellCopyResolved`; neither
  `SpellResolved` nor a physical terminal-zone move may name the source.
- A virtual copied multi-card library search commits every selected exact
  physical card before recording its batch-search and shuffle receipts. Its
  stack-only source then emits only `SpellCopyResolved`; it cannot emit
  `SpellResolved` or create a physical terminal-zone move.
- A virtual copied quantity-replacement decision commits the selected
  replacement before materializing the resulting token or counter quantity.
  Each applied replacement is recorded in order, and the stack-only source
  emits only `SpellCopyResolved` after the complete prospective event is
  committed; no virtual identity may enter a physical terminal zone.
- A virtual copied damage spell suspended for an affected player's
  replacement order retains its stack provenance through the selected shield
  and any forced follow-up replacements. After authoritative prevention and
  damage receipts commit, the source emits exactly one `SpellCopyResolved`;
  it cannot emit `SpellResolved` or move the virtual source to a physical zone.
- A virtual copy that creates a current-turn end-of-combat delayed action
  retains only immutable definition, incarnation, colors, and controller
  provenance after `SpellCopyResolved`. That provenance must name exactly one
  delayed action or stacked ability, supply its frozen source facts without a
  fabricated `CardObject`, and be pruned immediately after resolution.
- A delayed action has one live controller. If that player leaves a
  continuing multiplayer game, every delayed action and already-stacked
  ability they control leaves before any later timing window; linked-exile
  schedule metadata retires with the departed controller while exiled cards
  remain in their actual zones. No later end-step or end-of-combat receipt may
  stack an ability controlled by an eliminated player. An ability already on
  the stack receives exactly one `AbilityLeftGame` terminal receipt rather
  than a fabricated resolution or rules-counter receipt.
- A counter-unless decision binds its lower spell by the exact live lower
  stack item's source incarnation, not by a physical-zone lookup. The same
  public pay-or-counter and discard-your-hand boundaries must work for
  physical spells and stack-only virtual copies, with no fabricated card
  identity entering a decision or terminal receipt. If a response has already
  removed that lower spell before either decision opens, no choice is exposed:
  the common all-targets-illegal plan records the counter-unless spell's
  ordinary rules-counter terminal lifecycle instead of returning an error or
  leaving it stranded on the stack.
- If the counter-unless controller declines payment for a virtual target,
  continuation reuses that captured stack identity and records exactly one
  `SpellCopyCountered` terminal receipt. It cannot re-read a physical zone or
  move the stack-only copy to a card zone.
- A virtual copy of an activated ability applies its submitted retarget to
  the captured lower ability before committing that ability's effect. The
  target-change receipt and all resulting zone/other effects precede exactly
  one `SpellCopyResolved`; the virtual copy cannot remain live or emit a
  physical terminal-zone move.
- A virtual copy's public `PublicGraveyardLandReturn` completion commits only
  the selected live physical land cards in their captured incarnations before
  emitting `SpellCopyResolved`; the stack-only source never emits
  `SpellResolved` or a physical terminal-zone movement.
- A virtual copy's public graveyard-creature return preserves each affected
  player's selection order, commits all selected exact-incarnation physical
  creatures, then emits only `SpellCopyResolved`; neither the virtual source
  nor an omitted player selection may create a physical terminal-zone move.
- A virtual copied draw-then-conditional-discard spell retains the same
  boundary after its target player's valid private selection: its ordinary
  draw and discard receipts commit first, then its stack-only source emits
  `SpellCopyResolved`. A virtual source can never appear in a terminal
  `SpellResolved` receipt or an owner-zone move.
- An end-of-turn effect created by a resolved instruction is independent of a
  former battlefield source. Source departure preserves it through that
  turn's cleanup; target departure still expires it immediately. In contrast,
  a permanent-duration effect remains source-dependent and expires when either
  matching battlefield endpoint leaves.
- `UntilTargetLeavesBattlefield` is an independent target-lifetime duration:
  it retains the exact target incarnation, survives source departure without
  a synthetic expiration receipt, and expires immediately if that target
  leaves. It can never attach its stored change to a new incarnation of the
  same physical card.
- A public continuous-effect installation is one atomic engine transition. A
  rejected request restores the target's runtime state, live-effect list, and
  canonical receipt log exactly. In particular, `AddDamageShield` requires a
  strictly positive amount before installation, and the live-effect invariant
  rejects any nonpositive shield that bypasses the public constructor.
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
- A player's departure first expires every active layer-two effect that gives
  that player control, including a source-relative effect whose source is
  presently controlled by that player. The expiry batch records
  `ContinuousEffectExpired` before its derived `ControllerChanged` reversions,
  so CR 800.4a never exiles an opponent-owned permanent merely because stale
  temporary control was still visible to departure cleanup.
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
  illegal target leaves both zones and the event log unchanged. In a started
  game, a legal no-cast entry captures the Aura's own and controller-scoped
  entry observations only after attachment has established its endpoint but
  before its post-entry SBA checkpoint, then places them through the shared
  trigger pipeline. Its pregame fixture form deliberately creates no trigger
  provenance. Equipment
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
- A source-bound compatible-Aura library search attaches the selected Aura
  before capturing its normal entry batch. That batch includes the Aura's own
  ETB plus every applicable controlled Aura/nonartifact observer, and remains
  queued through the resolving search ability's terminal receipt and SBA
  checkpoint; a self-only Aura trigger path is invalid.
- The bounded linked-exile resolver intentionally stores no closures. Its
  typed group and delayed-action records remain invariant-valid while a sole
  target creature, or that creature plus its linked Auras, is suspended in
  exile, and the consuming end-step transition reaches the normal SBA boundary
  after returns. A multi-member return captures self ETBs and controller-scoped
  nonartifact/Aura observers from one post-entry snapshot, so every newcomer
  observes every member of that event before the shared SBA/trigger boundary.
  It is a substrate only: individual card promotion, arbitrary simultaneous
  delayed action ordering, and broader blink/zone-replacement interactions
  remain separate coverage work.
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
  departure immediately revokes that derived contribution. A source-excluding
  `other permanent` keyword binding applies to every other controlled
  battlefield permanent regardless of card type. Its derived `Shroud` is a
  target-legality rule, not a damage or prevention effect: targeted spells and
  abilities reject that permanent before costs or receipts and treat it as
  illegal on resolution recheck, while the excluded source remains targetable.
  No static characteristic evaluation creates a continuous-effect receipt. A
  static change cannot be inserted into the timestamped continuous-effect
  list.
- Static attack-restriction bindings are likewise immutable expansion data and
  can be registered only before game start; their complete registration batch
  is atomic, so a duplicate or invalid later member cannot retain an earlier
  hidden combat restriction. Each names a permanent definition,
  remains active only while a matching source is on the battlefield under the
  defending player's control, and is checked before a nonempty attacker
  declaration mutates any tapped state, combat provenance, or event log. A
  rejected declaration is therefore atomic; normal source departure revokes
  the restriction without a synthetic event or stale combat marker.
- Static top-library reveal bindings are immutable pregame data that name only
  permanent definitions. Their full registration batch is atomic; duplicate,
  unknown, nonpermanent, or live-game registration leaves no visibility
  prefix. A binding supplies public information
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
  spell has resolved before that same SBA check. A physical cast likewise
  reaches its post-cost SBA fixed point before placing any triggers observed
  from that cast or its additional costs. Thus, if an additional cost removes
  a static effect and creates another death, both observations enter one
  APNAP placement batch; no earlier `TriggeredAbilityStacked` receipt may
  preempt the required shared order decision. A public land play captures its
  own ETB and all land-entry observations while their live source incarnations
  still exist, then reaches that same post-entry SBA boundary. If an entry
  life payment ends the game, that boundary discards the captured events; it
  must record the legal land-entry and player-departure lifecycle without
  dereferencing a departed source or rolling the land play back. A resolving
  library search likewise captures each permanent entry (including land entry)
  while it is live but must not place the event until the original spell's
  terminal lifecycle and post-resolution SBA checkpoint. Thus an ETB cannot
  preempt the resolving search on the stack, and a short-lived entrant retains
  its historical battlefield source through the resulting SBA death. A
  permanent that enters only after a no-priority private resolution decision
  follows the same capture-before-SBA rule, including land-entry observers;
  its historical battlefield incarnation must survive a resulting SBA death
  rather than being recreated from the new graveyard object. The deterministic
  library-search compatibility path shares that exact capture pipeline for
  `Battlefield` and `BattlefieldTapped` results; it may not silently skip a
  selected permanent's self or controller-scoped entry observation. The
  deterministic and policy-submitted multi-card search branches first commit
  every selected battlefield result as one zone-change event. Their entry
  replacements see only the pre-event battlefield plus the entrant itself;
  self ETBs and controlled nonartifact/Aura observers then use one post-event
  snapshot, so a selected newcomer observes every peer as required by CR
  603.6a. Land-entry observers are captured only after that group snapshot.
  This all occurs before batch-result, shuffle, terminal, and post-resolution
  SBA receipts. Every represented simultaneous-entry route, including
  matching-creature graveyard returns and delayed linked-exile returns, uses
  that same post-entry order and separately captures one land-entry event for
  each returned creature–land; no represented land observer may be lost
  merely because its entrant was selected by a delayed or creature-return
  instruction. The common single physical-permanent entry capture likewise
  includes land-entry observation, so targeted reanimation and one-card
  library search cannot retain self ETBs while silently omitting a
  creature–land's land-entry event. For creature deaths selected
  in one pass, regeneration shields are consumed first, then the remaining
  death set receives one shared last-known-information observer snapshot
  before its members take their individual graveyard/token-departure
  transitions. No member's departure may suppress another member's generic
  death or leaves-the-battlefield trigger from that same SBA event.
- A represented simultaneous destruction instruction deduplicates its
  selected battlefield recipients, consumes each eligible regeneration shield
  before committing a death, and uses the same shared creature-departure
  observer snapshot for every remaining recipient. Its individual
  `CardDestroyed`, zone, and incarnation receipts are still ordered, but they
  cannot make an earlier dying source invisible to a later same-instruction
  departure or opposing graveyard entry. This applies to the non-token
  creature, mana-value creature and nonland, and represented Radiance
  enchantment destruction batches.
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
- A spell or ability draw is performed only after all earlier instructions
  resolve successfully. A successful draw moves exactly one library card to
  hand; an empty library instead creates a private player-specific pending
  loss marker. That marker remains only while its enclosing stack object (or
  its no-priority continuation) is live, blocks ordinary priority, and is
  consumed at the next state-based-action fixed point. Therefore every later
  instruction and the resolving object's `SpellResolved`/`AbilityResolved`
  plus terminal zone receipt precede `PlayerLost`; no pending empty-library
  marker may escape an externally observable transition.
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
  name the copy itself as its countering source. Where a receipt copies a
  virtual predecessor, that predecessor's opening receipt must appear earlier
  in the same epoch and it must still be nonterminal. A scenario event-log
  reset is rejected while a virtual copy is live, preserving the opening
  `SpellCopied` provenance that the lifecycle audit requires.
- A copy whose target set may be changed suspends through the same monotonic
  `DecisionId` state machine as other public choices. Its options are public
  legal target values, and its cardinality is zero through the original
  spell's target-slot count, whether that original is physical or a live
  virtual copy. Zero selections is the explicit optional-decline branch: the
  virtual copy retains the original target values and captured target
  incarnations. A nonempty selection must replace every target slot. Its
  continuation can complete only while the copying spell is still on top of
  the stack with the same original-source provenance. A stale decision, an
  altered source, a partial retarget, or an illegal target rejects atomically.
- A source-scoped exiled-spell-copy group is keyed by the permanent's exact
  `(ObjectId, incarnation)` and retains only physical instant-or-sorcery cards
  at their exact current `Exile` incarnation. An observed physical spell first
  records its ordinary `CardMoved` and `ObjectIncarnationAdvanced` receipts,
  then exactly one matching `SpellExiledByTrigger` receipt. An observed virtual
  copy instead has one `SpellCopyExiledByTrigger` terminal receipt and can
  never become a retained template or create a physical zone entry. If a
  prior response has already countered or otherwise ended that exact virtual
  copy, its observing trigger's impossible exile instruction is a no-op: it
  emits no duplicate copy-terminal receipt and may still offer retained
  physical templates normally.
- The parent triggered ability remains live beneath each serial
  `ExiledSpellCopyCast` decision. Each accepted card selection creates one
  fresh virtual copy with its own target/mode/color choices and a zero X value;
  a selected physical card cannot recur within that parent resolution. There
  is no priority window between those selections or after a selected free cast.
  Cast triggers made by the virtual copy are deferred until the parent records
  `AbilityResolved`; only then can ordinary trigger ordering and priority
  resume. A card leaving its retained exile incarnation is unlinked before it
  can be offered by a later decision.
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
