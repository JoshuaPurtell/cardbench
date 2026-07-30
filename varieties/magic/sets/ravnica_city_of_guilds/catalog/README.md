# Ravnica: City of Guilds public catalog

`rav_main_set.tsv` is a minimal, checked-in inventory of all 306 RAV main-set
collector-number printings. It is deliberately not a card-text corpus: it carries
only the collector number, card name, and a CardBench semantic-status declaration.
It contains no art, scan, flavor text, Oracle text, or hidden benchmark material.

The expected published collector-number range is `1..=306`: 286 non-basic-card
printings plus four printings each of Plains, Island, Swamp, Mountain, and Forest.
The Rust validator enforces every number in that range, the expected 291 distinct
names, and the four-printing basic-land exception.

The source snapshot was extracted from the public [Scryfall cards API](https://api.scryfall.com/cards/search?q=e%3ARAV&unique=prints&order=set)
on 2026-07-30, retaining only collector number and name. Its retained-field SHA-256
is recorded in the manifest header so the inventory can be reproduced without
committing the upstream JSON. Scryfall is a public data source, not an assertion of
ownership or affiliation.

Rows without an explicit third-column override inherit the documented
`catalog-only:card-specific-rules-not-implemented` status. This is fail-closed:
they are not inserted into `card_definitions()` and `executable_definition_id_for_collector`
returns their capability gap instead of a blank executable spell. Rows that point to
an executable compatibility definition remain subject to that definition's
`supported_rules` scope; catalog coverage does not claim full rules fidelity.

`RAV_FULL_FIDELITY_DEFINITION_IDS` is a deliberately small positive manifest,
not an inference from executable status. Char, Lightning Helix, Last Gasp,
Elves of Deep Shadow, Boros Recruit, Cleansing Beam, Rally the Righteous, and
Wojek Siren, Watchwolf, and Glass Golem are listed only after an ability-by-
ability contract proves their complete represented behavior and public receipt
traces. The three radiance entries were checked against their public set
identity and the official Comprehensive Rules' target, resolution, damage,
continuous-effect, and state-based-action rules; this repository retains only
CardBench-authored semantic operations, never card rules text. Every other
executable definition remains a bounded compatibility slice unless it is
explicitly added to that manifest after the same audit.

The creature batch for Golgari Thug, Stinkweed Imp, Greater Mossdog, and
Root-Kin Ally is explicitly compatibility-bounded. The first three support only
normal creature casting, base characteristics, and the shared Dredge replacement;
Root-Kin Ally supports only normal creature casting, base characteristics, and the
shared Convoke payment hook. Every other printed ability on those cards is
unsupported, including their card-specific triggered or combat behavior. The
checked-in definitions and public scenarios make that omission visible without
reproducing card rules text.

The generic-keyword coverage batch for Conclave Equenaut, Conclave Phalanx,
Golgari Grave-Troll, Necroplasm, Grave-Shell Scarab, Shambling Shell, Guardian
of Vitu-Ghazi, and Autochthon Wurm follows the same fail-closed boundary. Each
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
Birds of Paradise is separately bounded to
normal casting, base characteristics, and a CardBench-authored binding to the
engine's generic tap-and-single-color-choice mana-ability substrate; no other
card-specific behavior is represented.

The four RAV Signets are executable only through the shared paid fixed-bundle
mana-ability substrate. Their compatibility slice records each artifact's
colorless casting cost, artifact characteristics, and one `{1}` plus tap
activation producing its two fixed guild colors. The public scenarios assert
the cost-payment receipt, one receipt for each output color, an empty stack,
and retained activator priority. The catalog maps Dimir Signet at its retained
public collector number, `#260`; `#261` is Glass Golem and remains mapped to
its separate executable definition.

Five additional low-complexity spells make the executable boundary more useful
without pretending to complete their set mechanics. Rain of Embers uses the
shared, target-free global creature-and-player damage effect, with one-pass
selection and state-based actions after the complete damage batch. Dimir
Machinations, Shred Memory, Clutch of the Undercity, and Perplex are expressly
transmute-only compatibility definitions: their hand-zone transmute activations
are exercised in public scenarios, while each printed spell effect remains
non-covered. These are bounded semantic slices, not full-card claims.

The creature-chassis batch for Dromad Purebred, Snapping Drake, Carrion
Howler, Coalhauler Swine, Bramble Elemental, Carven Caryatid, Boros Swiftblade,
Loxodon Hierarch, Moroii, and Skyknight Legionnaire is also deliberately
bounded. Each records only public identity, mana cost, color, type, and base
power/toughness facts, and supports only normal creature casting and those base
characteristics. Every printed keyword, static rule, triggered behavior, and
activation on those cards is intentionally outside the executable slice. The
public scenarios exercise exact colored-cost payment, stack resolution, zone
movement, and base P/T values; they do not imply full-card fidelity.

The second creature-chassis batch adds Elvish Skysweeper, Frenzied Goblin,
Grayscaled Gharial, Greater Forgeling, Goliath Spider, Ivy Dancer, Lore Broker,
Mortipede, Selesnya Evangel, and Selesnya Sagittars. These definitions likewise
record only public identity, mana cost, color, creature type, and base
power/toughness. Their card-specific activated, triggered, evasion, token, and
combat behavior is intentionally unsupported. Four public cast-and-resolve
scenarios cover exact single-, double-, and multicolored payment, stack
resolution, graveyard movement, and retained base P/T; they do not assert any
omitted abilities.

The focused noncreature spell batch maps Ribbons of Night, Dogpile, and
Overwhelm to three named compatibility slices. Ribbons of Night has only its
fixed creature-damage and controller-life-gain fragment; its conditional card
draw is deliberately excluded because paid mana colors are not retained. Dogpile
uses the shared resolution-time count of controller-owned attacking creatures.
Overwhelm uses the shared Convoke hook and a controller-wide temporary layer-7
modifier. The corresponding Rust contracts and public event-log scenarios test
those exact fragments; none is a claim of complete card or set fidelity.

The easy-creature wave adds Benevolent Ancestor, Surveilling Sprite, Terraformer,
Roofstalker Wight, Sewerdreg, Goblin Spelunkers, Ordruun Commando, Viashino
Slasher, Civic Wayfinder, and Dowsing Shaman. These are likewise limited
to public identity, mana cost, color, creature type, and base power/toughness,
with normal colored-cost creature casting as their only executable behavior.
Every printed keyword, activation, and triggered behavior remains deliberately
unsupported. Five deterministic public scenarios exercise cast payment, stack
resolution, zone movement, priority, and their retained base P/T values only.

The first-range easy-creature wave adds Courier Hawk, Divebomber Griffin,
Sandsower, Votary of the Conclave, Drake Familiar, Drift of Phantasms, Ethereal
Usher, and Grozoth. Each supports only normal colored-cost creature casting and
base power/toughness. Every printed keyword, activated ability, triggered
ability, and hand-zone behavior is deliberately unsupported. Two deterministic
public scenarios cover their exact colored-cost payments, stack resolution, zone
movement, priority, and retained base P/T values only.

The TSV's structure and the Rust parser are CardBench-authored MIT-licensed code and
data structure. Magic set names and card names are used solely as nominative
interoperability identifiers and remain their owners' marks.
