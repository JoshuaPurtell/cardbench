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
embed card rules text, art, flavor text, or upstream JSON. Watchwolf is one
base-characteristics example: its slice covers only normal colored-cost casting
and its creature characteristics. Birds of Paradise is separately bounded to
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

The TSV's structure and the Rust parser are CardBench-authored MIT-licensed code and
data structure. Magic set names and card names are used solely as nominative
interoperability identifiers and remain their owners' marks.
