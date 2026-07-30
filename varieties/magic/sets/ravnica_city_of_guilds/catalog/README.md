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

Executable compatibility definitions use only the small public facts needed by
their declared `supported_rules` fields (for example, card identity, colors,
mana cost, types, and base characteristics). Those facts are CardBench-authored
semantic data derived from the same public set identity source; they do not
embed card rules text, art, flavor text, or upstream JSON. Watchwolf is the
current base-characteristics example: its slice covers only normal colored-cost
casting and its creature characteristics.

The TSV's structure and the Rust parser are CardBench-authored MIT-licensed code and
data structure. Magic set names and card names are used solely as nominative
interoperability identifiers and remain their owners' marks.
