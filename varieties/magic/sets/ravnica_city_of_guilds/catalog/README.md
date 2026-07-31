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

`RAV_FULL_FIDELITY_DEFINITION_IDS` is a deliberately small positive manifest,
not an inference from executable status. Char, Lightning Helix, Scatter the
Seeds, Guardian of Vitu-Ghazi, Last Gasp, Elves of Deep Shadow, Boros Recruit,
Nightguard Patrol, Watchwolf, Glass Golem,
Cleansing Beam, Rally the Righteous, Wojek Siren, Rain of Embers, Dogpile,
Overwhelm, Gather Courage, Seeds of Strength, Darkblast, Greater Mossdog,
Seismic Spike, Incite Hysteria, the
four RAV Signets, the five RAV basic lands, Conclave Equenaut, Snapping Drake,
Goliath Spider, Courier Hawk, Skyknight Legionnaire, Birds of Paradise, and
Fiery Conclusion, Ribbons of Night, and Smash are listed only after an
ability-by-ability contract proves their complete represented behavior and
public receipt traces. The radiance entries were
checked against their public set identity and the official Comprehensive Rules'
target, resolution, damage, continuous-effect, and state-based-action rules;
this repository retains only CardBench-authored semantic operations, never card
rules text. Every other executable definition remains a bounded compatibility
slice unless it is explicitly added to that manifest after the same audit.

The focused Darkblast/Scatter the Seeds/Siege Wurm/Guardian of Vitu-Ghazi audit
uses the same fail-closed rule. Darkblast is in the positive manifest because its
entire functional behavior is covered by the creature-targeted temporary modifier
and the fixed Dredge replacement substrate, with an exact public event-log
scenario. Scatter the Seeds is also in the positive manifest: its Convoke cost
and each created token's count, color, card type, typed creature subtype, and
base power/toughness are all represented and covered by a direct Rust contract
and fixed public trace. Guardian of Vitu-Ghazi is also in the positive manifest:
its Convoke payment, base characteristics, and vigilance declaration exception
are covered by direct Rust contracts and fixed public traces. Siege Wurm remains
bounded because trample combat-damage assignment is not executable.

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
Blue-draw and non-Blue paths. Dryad's Caress still lacks a graveyard-return
operation. Muddle the Mixture and Dizzy Spell retain their correctly tested effect
and Transmute compatibility slices, but Transmute is executed immediately in this
engine rather than as a stack object, so normal response behavior is absent. Their
deterministic public scenarios assert receipts only for the represented slices.
These are explicit coverage gaps, not no-op fallbacks or full-fidelity claims.

The five RAV basic-land definitions are positive-manifest entries. Their typed
single-color intrinsic mana result, tapping, and basic-land deck-construction
exception are represented and checked in public nonstack and cast-payment
traces. A cast request explicitly names each typed basic-land source; the
engine validates its fixed color, writes ordered intrinsic receipts, and rolls
back every source, pool, stack, and log mutation if a later payment fails. The
four RAV Signets are separately positive-manifest entries because their
definition-bound abilities use the same ordered atomic cast-payment boundary.

The creature batch for Golgari Thug, Stinkweed Imp, and Root-Kin Ally is
explicitly compatibility-bounded. The first two support only normal creature
casting, base characteristics, and the shared Dredge replacement; Root-Kin Ally
supports only normal creature casting, base characteristics, and the shared
Convoke payment hook. Every other printed ability on those cards is unsupported,
including their card-specific triggered or combat behavior. The checked-in
definitions and public scenarios make that omission visible without reproducing
card rules text.

The generic-keyword coverage batch for Conclave Phalanx, Golgari Grave-Troll,
Necroplasm, Grave-Shell Scarab, Shambling Shell, and Autochthon Wurm follows
the same fail-closed boundary. Each
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
Machinations, Shred Memory, Clutch of the Undercity, and Perplex remain
expressly transmute-only compatibility definitions: their hand-zone transmute
activations are exercised in public scenarios, while each printed spell effect
remains non-covered.

The creature-chassis batch for Dromad Purebred, Carrion Howler, Coalhauler
Swine, Bramble Elemental, Carven Caryatid, Boros Swiftblade, Loxodon Hierarch,
and Moroii is also deliberately
bounded. Each records only public identity, mana cost, color, type, and base
power/toughness facts, and supports only normal creature casting and those base
characteristics. Every printed keyword, static rule, triggered behavior, and
activation on those cards is intentionally outside the executable slice. The
public scenarios exercise exact colored-cost payment, stack resolution, zone
movement, and base P/T values; they do not imply full-card fidelity.

The second creature-chassis batch adds Elvish Skysweeper, Frenzied Goblin,
Grayscaled Gharial, Greater Forgeling, Ivy Dancer, Lore Broker, Mortipede,
Selesnya Evangel, and Selesnya Sagittars. These definitions likewise
record only public identity, mana cost, color, creature type, and base
power/toughness. Their card-specific activated, triggered, evasion, token, and
combat behavior is intentionally unsupported. Four public cast-and-resolve
scenarios cover exact single-, double-, and multicolored payment, stack
resolution, graveyard movement, and retained base P/T; they do not assert any
omitted abilities. Goblin Fire Fiend is a later static-keyword exception: its
Haste and same-turn attack are covered, while its must-block restriction and
activated power boost remain bounded.

The focused noncreature spell batch promotes Ribbons of Night, Dogpile, and
Overwhelm as ability-complete
positive-manifest entries. Dogpile's original RAV player-or-creature target
boundary uses the shared resolution-time count of controller-owned attacking
creatures; Overwhelm uses the shared Convoke hook and a controller-wide
temporary layer-7 modifier. Public scenarios cover their accepted selection,
rejected noncreature target, cost payment, event chronology, and state-based
action boundaries.

The easy-creature wave adds Benevolent Ancestor, Surveilling Sprite, Terraformer,
Roofstalker Wight, Sewerdreg, Goblin Spelunkers, Ordruun Commando, Viashino
Slasher, Civic Wayfinder, and Dowsing Shaman. These are likewise limited
to public identity, mana cost, color, creature type, and base power/toughness,
with normal colored-cost creature casting as their only executable behavior.
Every printed keyword, activation, and triggered behavior remains deliberately
unsupported. Five deterministic public scenarios exercise cast payment, stack
resolution, zone movement, priority, and their retained base P/T values only.

The first-range easy-creature wave adds Divebomber Griffin, Sandsower, Votary
of the Conclave, Drake Familiar, Ethereal Usher, and Grozoth. Each supports
only normal colored-cost creature casting and base power/toughness. Every
printed keyword, activated ability, triggered ability, and hand-zone behavior
is deliberately unsupported. Two deterministic public scenarios cover their
exact colored-cost payments, stack resolution, zone movement, priority, and
retained base P/T values only.

The following bounded keyword wave adds the static Defender slice for Torpid
Moloch and the static Flying slices for Belltower Sphinx, Screeching Griffin,
and Tattered Drake. Their trigger, activation, and regeneration behavior
remains outside the executable scope, so each has a focused compatibility trace
plus an ignored full-fidelity probe rather than a positive-manifest claim.

Carven Caryatid is likewise bounded to its static Defender slice in the
default fixture constructor. The expansion now also publishes an opt-in typed
enter-the-battlefield draw binding (`rav_triggered_ability_bindings`); its stack-backed
trace is covered separately, while the default compatibility trace and ignored
full-fidelity probe keep the constructor boundary explicit.

Selesnya Sagittars extends that wave with the shared static Reach slice. Its
tap-to-damage activation remains outside the executable scope; the public trace
proves only that Reach legally qualifies it to block a Flying attacker.

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
