# Ravnica: City of Guilds public catalog

`rav_main_set.tsv` is a minimal, checked-in inventory of all 306 RAV main-set
collector-number printings. It is deliberately not a card-text corpus: it carries
only the collector number, card name, and a CardBench semantic-status declaration.
It contains no art, scan, flavor text, Oracle text, or hidden benchmark material.

The expected published collector-number range is `1..=306`: 286 non-basic-card
printings plus four printings each of Plains, Island, Swamp, Mountain, and Forest.
The Rust validator enforces every number in that range, the expected 291 distinct
names, and the four-printing basic-land exception.

The five RAV basic-land definitions are full-fidelity positive-manifest entries.
They use the expansion-neutral typed basic-land binding substrate: each public
type line is bound to one basic-land definition, and the engine checks that its
intrinsic mana ability produces the corresponding single color. Public traces
cover both a direct intrinsic activation and explicit in-cast payment, including
the ordered receipts and full rollback of a failed final payment. This repository
retains only `CardBench` semantic identifiers and operations, never card prose
or art.

The source snapshot was extracted from the public [Scryfall cards API](https://api.scryfall.com/cards/search?q=e%3ARAV&unique=prints&order=set)
on 2026-07-30, retaining only collector number and name. Its retained-field SHA-256
is recorded in the manifest header so the inventory can be reproduced without
committing the upstream JSON. Scryfall is a public data source, not an assertion of
ownership or affiliation.

The 2026-07-30 ability-completeness audit for Rain of Embers, Dogpile, and
Overwhelm consulted the public RAV printing records [#138](https://scryfall.com/card/rav/138/rain-of-embers),
[#120](https://scryfall.com/card/rav/120/dogpile), and
[#175](https://scryfall.com/card/rav/175/overwhelm), plus the public
[Comprehensive Rules index](https://magic.wizards.com/en/rules). It retained no
upstream card prose, scans, art, or JSON; executable semantics and tests are
CardBench-authored descriptions of the audited behavior.

Rows without an explicit third-column override inherit the documented
`catalog-only:card-specific-rules-not-implemented` status. This is fail-closed:
they are not inserted into `card_definitions()` and `executable_definition_id_for_collector`
returns their capability gap instead of a blank executable spell. Rows that point to
an executable compatibility definition remain subject to that definition's
`supported_rules` scope; catalog coverage does not claim full rules fidelity.

Auratouched Mage is a full-fidelity source-bound Aura-search definition. Its
stack-backed ETB opens a controller-private, zero-or-one choice over the
currently compatible Auras in that controller's library. The chosen Aura
enters attached only to the Mage's captured current incarnation; an explicit
decline or no candidate records the same search-and-shuffle lifecycle without
a zone move. The fetched Aura's ordinary ETB trigger is queued only after the
Mage trigger's terminal receipt. The opponent sees neither unselected Aura
identities nor a private decision projection.

The current catalog partition is 229 full / 28 partial / 34 catalog-only unique
names (244 / 28 / 34 printings). The independently checked coverage-report
binary emits the same partition.

Bramble Elemental is full-fidelity through the expansion-neutral, current-
controller-scoped Aura-entry trigger condition. Its controller receives the
ordinary optional stack decision, where accepting creates one typed Saproling
and declining creates none; an opponent-controlled Aura cannot queue that
trigger.

Spawnbroker is a positive-manifest entry through the expansion-neutral paired-
target control-exchange instruction. Its ETB target decision retains both
ordered creature occurrences and validates the opponent creature's current
power against the controlled one before stack placement and resolution. A
completed exchange writes two durable layer-two effects before either control
receipt; a response that makes either target illegal leaves neither permanent
partially exchanged.

`RAV_FULL_FIDELITY_DEFINITION_IDS` is a deliberately small positive manifest,
not an inference from executable status. Char, Lightning Helix, Scatter the
Seeds, Guardian of Vitu-Ghazi, Last Gasp, Elves of Deep Shadow, Boros Recruit,
Nightguard Patrol, Watchwolf, Glass Golem, Junktroller,
Cleansing Beam, Rally the Righteous, Wojek Siren, Rain of Embers, Dogpile,
Overwhelm, Gather Courage, Seeds of Strength, Darkblast, Greater Mossdog,
Stinkweed Imp, and Golgari Thug,
Seismic Spike, Incite Hysteria, Searing Meditation, the
four RAV Signets, the five RAV basic lands, Conclave Equenaut, Snapping Drake,
Goliath Spider, Courier Hawk, Skyknight Legionnaire, Birds of Paradise, and
Golgari Germination are listed only after an
ability-by-ability contract proves their complete represented behavior and
public receipt traces. Sundering Vitae and Recollect are now included after
their typed target and zone-transition contracts: Sundering Vitae's
artifact-or-enchantment destruction receipt and Recollect's controller-scoped
graveyard-return receipt. The radiance entries were
checked against their public set identity and the official Comprehensive Rules'
target, resolution, damage, continuous-effect, and state-based-action rules;
this repository retains only CardBench-authored semantic operations, never card
rules text. Every other executable definition remains a bounded compatibility
slice unless it is explicitly added to that manifest after the same audit.

Bottled Cloister is a positive-manifest entry. Its two upkeep triggers use the
same reusable source-incarnation hand-exile group: an opponent's upkeep moves
the controller's current hand to exile, and the controller's upkeep returns
only those exact exile incarnations before the following draw instruction. A
source departure with no pending return ability records group expiry while the
cards correctly remain in exile; a later re-entry cannot reclaim them. The
link is source-incarnation rather than controller scoped, so a control change
preserves former cards' owner-hand return while the new controller draws.

Flame Fusillade is a positive-manifest entry. Its resolving spell snapshots
only the caster's current creatures and gives each a typed layer-six tap
activation through cleanup. The recipient remains the ability source; source
or recipient departure after activation cannot erase the normal stack object,
while cleanup and recipient re-entry revoke the grant for future activations.

Faith's Fetters is also a positive-manifest entry. Its typed enchant-permanent
attachment carries reusable combat and nonmana-activation restrictions while
leaving mana abilities legal, and its separate ETB trigger gains four life only
after the Aura has legally resolved and attached. The focused contract covers
both a creature and a land target, atomic rejected activation, receipts, and
attachment invariants.

Glare of Subdual is a positive-manifest entry. Both of its zero-mana activated
abilities require one explicit, distinct, untapped controlled creature as a
cost: one uses the ordinary target-tap stack instruction, and the other creates
one target-free record that prevents every combat-damage packet through cleanup.
The public scenario pins the cost/stack/event sequence; the focused combat
contract demonstrates prevention, `DamageCannotBePrevented` precedence, expiry,
and the state-machine invariant audit without retaining card prose or art.

The focused Darkblast/Scatter the Seeds/Siege Wurm/Guardian of Vitu-Ghazi audit
uses the same fail-closed rule. Darkblast is in the positive manifest because its
entire functional behavior is covered by the creature-targeted temporary modifier
and the fixed Dredge replacement substrate, with an exact public event-log
scenario. Scatter the Seeds is also in the positive manifest: its Convoke cost
and each created token's count, color, card type, typed creature subtype, and
base power/toughness are all represented and covered by a direct Rust contract
and fixed public trace. Guardian of Vitu-Ghazi is also in the positive manifest:
its Convoke payment, base characteristics, and vigilance declaration exception
are covered by direct Rust contracts and fixed public traces. Siege Wurm is also
in the positive manifest: its direct contract pays five generic and two Green
Convoke contributions, then verifies a legal Trample combat packet through a
RAV Watchwolf with the resulting permanent and player damage receipts.

Greater Mossdog is also in the positive manifest. On 2026-07-30, a live
public Scryfall API lookup for its RAV collector `#169` verified the encoded
mana cost, color, creature characteristics, and that its sole functional rule
is the engine's generic Dredge replacement. The source response's reminder
annotation was not retained. A direct Rust contract pins the complete
dredge-then-cast public receipt, including all three library moves.

The 2026-07-30 simple-spell audit removed Gaze of the Gorgon from the executable
slice. A prior fixture had both an incorrect hybrid-cost model and an unrelated
temporary power/toughness operation; neither is a safe approximation. The
original RAV functionality requires regeneration replacement handling and a
delayed end-of-combat action based on combat block history. Both substrates are
currently absent, so collector `#246` is catalog-only with that explicit
capability gap and its former scenario has been removed. This repository keeps
only this CardBench-authored semantic summary, not upstream card prose or art.

The same audit rechecked the remaining executable simple instants and sorceries
that reuse existing `Effect` operations. Fiery Conclusion now has its required
controlled-creature sacrifice bound as an explicit, transactional cast cost;
its public trace records the cost departure before the targeted spell enters the
stack. Ribbons of Night now requires an explicit generic/hybrid mana selection,
stores its ordered paid-color receipt on the stack object, and tests both its
Blue-draw and non-Blue paths. Dryad's Caress is now a positive-manifest entry:
at resolution it counts only creature cards in its controller's graveyard for
life, then returns its typed creature-card target to hand; public evidence
excludes a noncreature graveyard card from that count. Muddle the Mixture and
Dizzy Spell retain their correctly tested effect and Transmute compatibility
slices, but Transmute is executed immediately in this engine rather than as a
stack object, so normal response behavior is absent. Their deterministic public
scenarios assert receipts only for the represented slices. These are explicit
coverage gaps, not no-op fallbacks or full-fidelity claims.

Infectious Host is a positive-manifest entry. Its controller submits the
target player for the dies trigger through a public target-bearing decision;
the selected ability then resolves on the ordinary stack and records the
non-damage life-loss receipt. The public scenario and focused Rust contract
cover the `DecisionOpened → DecisionCompleted → TriggeredAbilityStacked →
LifeLost → AbilityResolved` lifecycle and run the state-machine invariant
audit after resolution.

The five RAV basic-land definitions are positive-manifest entries. Their typed
single-color intrinsic mana result, tapping, and basic-land deck-construction
exception are represented and checked in public nonstack and cast-payment
traces. A cast request explicitly names each typed basic-land source; the
engine validates its fixed color, writes ordered intrinsic receipts, and rolls
back every source, pool, stack, and log mutation if a later payment fails. The
four RAV Signets are separately positive-manifest entries because their
definition-bound abilities use the same ordered atomic cast-payment boundary.

Root-Kin Ally is now a positive-manifest entry: its ETB trigger retains each
actual Convoke contributor's exact object incarnation, placing a counter only
on contributors still represented by that incarnation when the trigger
resolves. Conclave Phalanx likewise records its controller-white-creature life
ETB using the shared resolution-time count. Their focused traces cover
`ConvokeUsed`, spell resolution, trigger stacking, contributor-only
`CounterPlaced`, and `LifeGained` receipts. Siege Wurm and Autochthon Wurm now
have complete Convoke-and-Trample contracts, including the attacker-owned
multi-block damage-order decision and exact excess-damage receipts.
Golgari Brownscale, Golgari Grave-Troll, Stinkweed Imp, and Golgari Thug instead
are positive-manifest
cards. Brownscale's Dredge replacement retains its exact prior graveyard
incarnation and queues its life trigger only for a resulting graveyard-to-hand
move. Grave-Troll now counts controller-graveyard creatures as entry counters
before state-based actions and exposes its typed counter-removal regeneration
activation; the Doubling Season replacement trace is covered separately. The Imp's
static Flying and shared Dredge behavior are joined by the existing
exact-incarnation combat-damage-recipient trigger, which has no free target choice
and destroys only the creature that actually received the combat damage. The Thug
adds a policy-submitted current controller-graveyard creature-card target that
moves to its owner's library top after its historical battlefield incarnation dies.
The checked-in definitions and public scenarios make every remaining omission
visible without reproducing card rules text.

Farseek is a positive-manifest entry: its typed non-Forest land search is a
controller-private policy decision, so a legal Island or Plains selection is
captured before the tapped battlefield move, shuffle, and terminal resolution.
The public scenario and focused Rust contracts retain that private decision
boundary without copying card rules text.

Dizzy Spell is also positive-manifest: its target-creature layer-seven
modifier and hand-zone Transmute search are both represented by typed effects,
public scenarios, and invariant-checked event traces.

Muddle the Mixture is positive-manifest as well: its typed instant/sorcery
counterspell and hand-zone Transmute paths each have deterministic public
scenario coverage and stack/zone receipts.

Necroplasm is now a bounded trigger slice: its controller submits the order of
its two simultaneous upkeep triggers, and the generic creature-only sweep
samples the live `+1/+1` counter quantity only when it resolves. The public
scenario keeps that policy decision and its resulting destruction receipts
visible. It remains partial: a trigger whose source left before resolution
does not yet use last-known counter information, so it is not in the positive
full-fidelity manifest.

The generic-keyword coverage batch for Conclave Phalanx, Golgari Grave-Troll,
Grave-Shell Scarab, and Shambling Shell follows the same
fail-closed boundary. Each
definition records only its public identity, mana cost, color, type, base
characteristics, and the engine's existing generic Dredge or Convoke hook. The
public fixed scenarios exercise that generic hook and normal casting where it is
safe to do so. The definition comments name every omitted printed behavior;
none of these eight cards claims full-card fidelity. The author checked public
RAV collector identity and the presence of its generic keyword against the
Scryfall API snapshot described above, but did not retain source JSON or copy
rules text into this repository.

Executable compatibility definitions use only the small public facts needed by
their declared `supported_rules` fields (for example, card identity, colors,
mana cost, types, and base characteristics). Those facts are CardBench-authored
semantic data derived from the same public set identity source; they do not
embed card rules text, art, flavor text, or upstream JSON. Watchwolf and Glass
Golem are the positive vanilla-card examples and are in the full-fidelity
manifest: their public printed functional rules fields are empty, so normal
casting and their characteristics complete their represented functionality.
Nightguard Patrol is the positive keyword-creature example: its public RAV
record [#26](https://scryfall.com/card/rav/26/nightguard-patrol) was checked on
2026-07-30 for identity, cost, color, type, base power/toughness, and its two
generic combat keywords. The expansion-neutral first-strike and vigilance
substrates, direct Rust contract, and public event trace represent both
functional rules; the source response and card prose were not retained.
Birds of Paradise is a positive-manifest entry: Flying, normal creature
characteristics, each explicit single-color choice, summoning-sickness
rejection, and atomic cast-payment receipts have direct Rust contracts and
shown traces. The retained material is CardBench-authored semantic data, not
card rules text, art, flavor, or source JSON.

## Vanilla/chassis audit boundary

The 2026-07-30 follow-up audit of every RAV printing used the public Scryfall
RAV-print search with a nonempty-rules-field exclusion. It identified only
Watchwolf (#239) and Glass Golem (#261) as ability-free printed creature or
artifact-creature candidates. Both are already present in
`RAV_FULL_FIDELITY_DEFINITION_IDS`; their direct Rust contract pins cost,
colors, types, power/toughness, and the absence of executable abilities.

Every other executable creature chassis remains outside that positive manifest.
Its current empty `keywords` or `effects` vector is a deliberately bounded
engine representation, not evidence that the printing is vanilla. Public
source responses, card prose, art, and JSON from this audit were not retained.

The four RAV Signets are positive-manifest entries. Their definitions record
each artifact's colorless casting cost, artifact characteristics, and paid
fixed two-color mana ability; the shared engine now also accepts an ordered,
typed definition-bound mana-activation list within the enclosing
`CastRequest` payment transaction. Public scenarios cover both direct
activation and activating each Signet while paying a second artifact spell,
with fixed event-log digests for context declaration, activation cost, two
mana outputs, one spell cast, and spell resolution. The cast transaction is
atomic, and the engine invariant audit rejects missing or reordered
payment-context receipts. The catalog maps Dimir Signet at its retained public
collector number, `#260`; `#261` is Glass Golem and remains mapped to its
separate executable definition.

Rain of Embers is an ability-complete positive-manifest entry: the shared,
target-free global creature-and-player damage operation snapshots all affected
objects, then runs state-based actions after its complete damage batch. Dimir
Machinations, Shred Memory, and Perplex remain expressly transmute-only
compatibility definitions: their hand-zone transmute activations are exercised
in public scenarios, while each printed spell effect remains non-covered.
Clutch of the Undercity separately executes its targeted permanent-bounce
front face, snapshots the bounced permanent's last battlefield controller for
the three-life loss, and retains its immediate hand-zone Transmute operation as
an explicit bounded compatibility action.

Brainspoil is a separate bounded slice: its typed front face destroys only a
nonblack creature through the ordinary regenerable destruction lifecycle, and
its immediate hand-zone Transmute operation remains explicitly outside the
stack-backed fidelity boundary.

The creature-chassis batch for Coalhauler Swine, Bramble Elemental, Boros
Swiftblade, and Loxodon
Hierarch is also deliberately
bounded. Each records only public identity, mana cost, color, type, and base
power/toughness facts, and supports only normal creature casting and those base
characteristics. Every printed keyword, static rule, triggered behavior, and
activation on those cards is intentionally outside the executable slice. Moroii
and Dromad Purebred have since been promoted: Moroii's Flying and stack-backed
upkeep life-loss trigger, Dromad's received-damage one-life trigger, and
Carven Caryatid's Defender plus ETB draw are fully represented. The
public scenarios exercise exact colored-cost payment, stack resolution, zone
movement, and base P/T values; they do not imply full-card fidelity.

Carrion Howler is a positive-manifest entry. Its sole activation uses the
engine's generalized life-payment cost profile, records `AbilityLifePaid`
before the stack object, and applies the exact temporary source modifier only
on normal ability resolution. The shown scenario records the entire lifecycle
and the focused contract ends with the state-machine invariant audit.

Mortipede is a positive-manifest entry. Its zero-target `{2}{G}` activation
uses the ordinary ability stack to grant its source the temporary
`MustBeBlockedIfAble` keyword. The shown scenario proves both the resulting
combat constraint and a legal blocker assignment; the focused contract also
proves that an empty assignment is rejected atomically when a legal blocker is
available.

Selesnya Sagittars is a positive-manifest entry. Its static Reach remains live
for blocker legality, and its zero-cost tap activation targets exactly one
attacking or blocking creature. The shown scenario captures the one-damage
stack resolution against an attacking creature; the focused contract validates
the typed target restriction, source tap, damage receipt, and final invariant
state.

The second creature-chassis batch adds Elvish Skysweeper, Frenzied Goblin,
Greater Forgeling, Lore Broker, and Selesnya Evangel. These definitions likewise
record only public identity, mana cost, color, creature type, and base
power/toughness. Their card-specific activated, triggered, evasion, token, and
combat behavior is intentionally unsupported. Four public cast-and-resolve
scenarios cover exact single-, double-, and multicolored payment, stack
resolution, graveyard movement, and retained base P/T; they do not assert any
omitted abilities. Goblin Fire Fiend is a later static-keyword exception: its
Haste and same-turn attack are covered, while its must-block restriction and
activated power boost remain bounded.

Grayscaled Gharial is now a full-fidelity static-keyword entry. Its typed
Islandwalk is captured when it is declared as an attacker; a defender that
controls a registered Island cannot submit a blocker for that attacker. The
focused trace proves the rejection is atomic and leaves no blocker-declaration
receipt, so this combat rule is not inferred merely from final battlefield
state.

The focused noncreature spell batch promotes Ribbons of Night, Dogpile, and
Overwhelm as ability-complete
positive-manifest entries. Dogpile's original RAV player-or-creature target
boundary uses the shared resolution-time count of controller-owned attacking
creatures; Overwhelm uses the shared Convoke hook and a controller-wide
temporary layer-7 modifier. Public scenarios cover their accepted selection,
rejected noncreature target, cost payment, event chronology, and state-based
action boundaries.

The original easy-creature wave adds Benevolent Ancestor, Surveilling Sprite,
Terraformer, Roofstalker Wight, Sewerdreg, Goblin Spelunkers, Ordruun Commando,
and Viashino Slasher. Its remaining bounded entries are limited
to public identity, mana cost, color, creature type, and base power/toughness,
with normal colored-cost creature casting as their only executable behavior.
Benevolent Ancestor has graduated: its static Defender, ordinary tapped
activation, player-or-creature target validation, target-side one-damage shield,
and receipt/invariant lifecycle are all represented. The separate compatibility
scenario continues to exercise its rejected Defender attack declaration.
Roofstalker Wight has graduated from that
compatibility chassis: it retains its exact Black casting identity and uses the
shared target-free stack activation to gain Flying through the current turn's
cleanup. Civic Wayfinder is now full-fidelity: its stack-backed ETB exposes only
legal basic-land candidates to its controller, accepts an explicit selection or
failure to find, publicly reveals a selected card immediately before moving it
to hand, and then shuffles. Declining does not disclose a card but still
performs the required shuffle.

Dowsing Shaman has graduated from that bounded wave. Its fully represented
activation uses the typed controller-owned enchantment-card graveyard target,
pays `{2}{G}` and taps before entering the ordinary stack lifecycle, then
returns that card to hand only if it remains a legal target at resolution. The
focused public Rust contract records the payment, activation, priority passes,
zone move, terminal receipt, and the atomic rejected non-enchantment control.

Ivy Dancer has also graduated. Its zero-mana tap activation grants the typed
Forest landwalk keyword to exactly one creature through the ordinary stack,
then the declaration-time combat provenance rejects a block only while the
defender controls a Forest. The focused contract proves the targeted temporary
effect, the rejected noncreature target before tapping, the absence of a
`BlockersDeclared` receipt for the illegal block, and the final invariant audit.

Seed Spark and Leave No Trace are also fully represented White instant slices.
Seed Spark retains the typed artifact-or-enchantment target boundary, destroys
the target, then creates exactly two typed Saprolings only after the normal
all-illegal-target check. Leave No Trace uses a narrower typed enchantment
target and snapshots its Radiance destruction batch before moving the target
and every other color-sharing enchantment to graveyards. Direct Rust traces
prove the event ordering, nonmatching-enchantment preservation, atomic
nonenchantment rejection, and final invariant audit.

Hunted Lammasu is a complete creature-and-trigger slice. It preserves its
Flying base creature characteristics, then puts a targeted-opponent ETB
ability on the stack; after its own priority window the selected opponent
receives exactly one typed black 4/4 Horror token. The focused trace asserts
the source spell, battlefield transition, trigger, target, token receipt, and
final invariant audit.

Hour of Reckoning is a complete Convoke sorcery slice. Its target-free
resolution snapshots every current nontoken creature, applies the ordinary
regenerable destruction lifecycle to that complete set, and leaves tokens
untouched. The focused trace creates three Saprolings first, then records four
`CardDestroyed` transitions before Hour's `SpellResolved` receipt; the
Saprolings remain on the battlefield and the final state-machine audit passes.

Oathsworn Giant and Veteran Armorer are complete static-creature-effect slices.
Their controller-scoped modifiers apply only to other friendly creatures:
Veteran adds toughness, while Oathsworn adds toughness and Vigilance. The
derived-characteristics contract proves both sources exclude themselves, stack
on a shared friendly creature, never affect an opponent's creature, and vanish
immediately when the relevant source leaves the battlefield. Static bindings
are expansion metadata, so these effects emit no artificial lifecycle event.

Gate Hound is a complete Aura-conditioned static-creature-effect slice. While
any live Aura is attached to the Hound, the Hound and every other creature its
controller controls derive Vigilance; no opposing creature does. The contract
attaches Moldervine Cloak, then destroys that Aura with Seed Spark and proves
the ordinary destruction and zone transition revoke all derived keywords
without a synthetic static-effect receipt.

Blazing Archon is a complete static-combat-restriction slice. Its immutable
battlefield binding rejects any nonempty attacker declaration against its
controller before attacker state or an `AttackersDeclared` receipt can exist.
The focused contract separately removes the Archon through ordinary state-based
actions, then proves the same attacker declaration becomes legal immediately.

Caregiver is a complete targeted-prevention activation slice. Its typed `{W}`
activation sacrifices the source before placing the ability on the stack, then
creates a one-shot shield for a player or creature target. The shield remains
valid across the source's graveyard move, emits explicit creation and
prevention receipts, consumes only the next damage, and is removed at target or
turn departure. The focused Char trace checks the two damage events, the
prevented portion, and the final invariant audit.

Ghosts of the Innocent is an executable bounded compatibility slice. Its RAV
collector identity, color, cost, creature characteristics, and sole global
damage-amount behavior were checked against the public
[Scryfall RAV #20 record](https://scryfall.com/card/rav/20/ghosts-of-the-innocent)
on 2026-08-02; no source response, card prose, image, or art is retained. The
expansion-neutral binding discovers each live source incarnation and records a
replacement receipt before the reduced player- or permanent-damage receipt.
The focused Rust trace covers both recipient kinds and the final invariant
audit. Concurrent ordering with the engine's broader replacement families is
still explicitly bounded, so this entry is not added to the positive
full-fidelity manifest.

Chant of Vitu-Ghazi is a complete dynamic-Convoke life-gain slice. Its
resolution counts every current battlefield creature, including opposing
creatures and tokens, then emits one source-controller `LifeGained` receipt.
The focused trace pays with two Convoke creatures and verifies the count at
resolution rather than at cast time.

The first-range easy-creature bounded wave includes Drake Familiar and Grozoth.
Each supports normal colored-cost creature casting and base power/toughness;
every other printed keyword, activated ability, triggered ability, and hand-zone
behavior is deliberately unsupported. Their deterministic public scenarios cover
exact colored-cost payments, stack resolution, zone movement, priority, and
retained base P/T values only.

Ethereal Usher is a full-fidelity entry: its typed Blue tap activation grants
the selected creature temporary Unblockable through the ordinary stack and
continuous-effect lifecycle, and its `{1}{U}{U}` Transmute is the existing
stack-backed hand-zone ability with a controller-private library search. Focused
Rust traces show both response windows, terminal receipts, rejected illegal
blocks, and invariant validation.

Sandsower has since graduated from that bounded wave: its full activation uses
three policy-selected, distinct, untapped creatures its controller controls as
a cost and taps the target creature only when the ability resolves. Its focused
regression covers the cost receipts, target timing, stack lifecycle, and
invariant audit.

Divebomber Griffin has likewise graduated: its full slice retains Flying and
uses the existing sacrifice-as-an-ability-cost boundary to deal damage only to
an attacking or blocking creature. The focused combat trace checks the source
graveyard move, target legality, stack passes, damage, terminal receipt, and
subsequent state-based action.

The following bounded keyword wave adds the static Defender slice for Torpid
Moloch and the static Flying slices for Belltower Sphinx, Screeching Griffin,
and Tattered Drake. Their trigger, activation, regeneration, and
damage-triggered destruction behavior remains outside the executable scope, so
each has a focused compatibility trace plus an ignored full-fidelity probe rather
than a positive-manifest claim. Stinkweed Imp has since graduated separately
through the exact combat-damage trigger substrate.

Woebringer Demon is now a positive-manifest entry. Its beginning-of-each-upkeep
trigger captures the active player at the upkeep boundary, opens that player's
public creature-sacrifice choice after ordinary priority passes, and records the
selected permanent's sacrifice and zone transition before the terminal ability
receipt. The shown trace exercises the controller's upkeep; the focused Rust
trace separately confirms that an opponent, rather than the Demon controller,
owns the choice on that opponent's upkeep.

Vulturous Zombie is also a positive-manifest entry. Its battlefield observer
uses the shared opponent-graveyard transition condition, so an opponent-owned
discard, mill, destruction, countered spell, or paid cost can queue one ordinary
source counter trigger. The focused Rust trace exercises the private-discard
case and records the normal stack, priority, counter, and terminal receipts;
the invariant audit checks the same cross-player provenance.

Vinelasher Kudzu is a positive-manifest entry. Its shown land-play trace proves
that only a land entering under the source controller queues its ordinary
counter trigger; the two players receive the normal response window before the
source's one `+1/+1` counter receipt. The focused negative contract separately
checks that an opponent's land entry creates neither a trigger nor a counter.

Carven Caryatid's full trigger binding is published through
`rav_triggered_ability_bindings`; its focused traces cover both the ordinary
stack-backed draw and terminal empty-library draw order. The compatibility
scenario still independently demonstrates the static Defender declaration
rule.

Drift of Phantasms is a narrower compatibility exception: its public definition
does encode Defender and the established hand-zone Transmute operation, and a
shown trace proves both the rejected Defender attack and the matching-value
search receipt. It remains outside `RAV_FULL_FIDELITY_DEFINITION_IDS`: the
current Transmute operation resolves immediately rather than becoming an
activated ability on the stack, so opponents do not receive the printed response
window. The ignored full-fidelity probe remains intentionally red until that
engine gap is closed.

Grozoth has the same bounded Defender/immediate-Transmute compatibility slice:
its public definition and shown trace cover its rejected attack declaration and
matching-value hand-zone search. Its entry-triggered library search is not
represented, and immediate Transmute still has no activated-ability stack
object or response window. Accordingly Grozoth remains outside the positive
fidelity manifest; its ignored full-fidelity probe documents both boundaries.

Moonlight Bargain is a full-fidelity private-library choice slice. Its
selection opens only after both players have passed and the spell begins
resolving; the engine keeps that spell on top of the stack, shows candidate
identities only to its controller, forbids any intervening priority action,
and records the selected-card life payment and every zone move before the
spell's terminal receipt. This is a state-machine boundary rather than a
pre-cast or pre-resolution fixture choice.

Thoughtpicker Witch is a full-fidelity activated-ability private-choice
slice. After its explicit generic mana and creature-sacrifice cost, the
targeted ability remains on the stack through both priority passes. The engine
then projects the target opponent's top-two snapshot only to the source
controller, permits no intervening priority action, and moves exactly one
chosen candidate to exile before the terminal ability receipt. No candidate
identity enters the public event log; unchosen cards retain library order.

The focused Flying/Reach/Haste audit promotes Conclave Equenaut, Snapping Drake,
Goliath Spider, Courier Hawk, and Skyknight Legionnaire into the positive fidelity manifest. Their
complete printed functional behavior falls within Convoke, normal casting/base
characteristics, Flying/Reach blocker declarations, and (for Courier Hawk)
vigilance. Public scenarios retain canonical declarations for Flying blocked by
Reach, Flying blocked by Flying, an untapped Flying/Vigilance attacker, and a
same-turn Flying/Haste attack after the creature resolves.

The TSV's structure and the Rust parser are CardBench-authored MIT-licensed code and
data structure. Magic set names and card names are used solely as nominative
interoperability identifiers and remain their owners' marks.
