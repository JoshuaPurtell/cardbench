# Magic fixture provenance and rights

## What is in this repository

- CardBench-authored Rust source that models generic game rules, a small
  executable RAV compatibility slice, and a complete public RAV printing
  inventory with an explicit executable/catalog-only boundary.
- Public set identifiers, names, collector-number facts, color/type/mana-value
  facts, and compact machine-executable effect keys needed by the published
  scenarios.
- Authored TOML manifests, deck counts, scenario identifiers, and event-log
  baseline digests.

## What is deliberately absent

- Official card art, scans, frames, symbols, flavor text, collector-number
  database, or an Oracle-text corpus.
- Any proprietary benchmark fixtures, held-out scenarios, credentials, or
  upstream private evaluation assets.

The executable effects are CardBench-authored semantic representations, not a
redistribution of complete card text. `supported_rules` on each `CardDefinition`
names the exact scoped fragment. A missing fragment is intentionally not a
claim of unsupported behavior.

## Complete RAV inventory boundary

`sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv` inventories the 306
published RAV main-set collector-number printings while retaining only the
collector number, card name, and a CardBench semantic-status declaration. The
inventory has no card text or image data. Each record is either explicitly mapped
to an executable compatibility definition or marked catalog-only with a declared
capability gap; catalog-only records are deliberately rejected before game setup
and cannot resolve as blank cards. See that directory's README for its snapshot
hash and reproduction details.

## Sources consulted

- The official [Magic rules page](https://magic.wizards.com/en/rules) is the
  authoritative source for the comprehensive rules framework.
- Wizards of the Coast's [Ravnica design history](https://magic.wizards.com/en/news/making-magic/city-planning-part-iii-2005-09-19)
  confirms the original set's guild/mechanic structure: Boros, Dimir, Golgari,
  and Selesnya; radiance, transmute, dredge, and convoke.
- The original-block 4–3–3 guild allocation is described in the official
  [Ravnica block design history](https://magic.wizards.com/en/news/making-magic/city-planning-part-ii-2005-09-12).
- The narrow Dizzy Spell, Brainspoil, Clinging Darkness, and Darkblast
  compatibility facts were checked against their public official Gatherer
  records ([Dizzy Spell](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87925),
  [Brainspoil](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=88965),
  [Clinging Darkness](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83822),
  and [Darkblast](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87922)).
  Only identity, set, collector number, mana cost, color, type, and keyword
  facts required by the scoped compatibility definitions were retained; no
  card text, image, or response payload is checked in.
- The Glass Golem compatibility facts were checked on 2026-07-30 against a
  single public [Scryfall named-card response](https://api.scryfall.com/cards/named?exact=Glass%20Golem),
  corroborated by the RAV inventory. The checked fields were identity, set,
  collector number, mana cost, type, power, toughness, and whether the public
  rules field was empty. That last fact permits its vanilla compatibility
  definition without silently dropping a printed ability; the response and
  any card text remain uncommitted.
- The public [Scryfall cards API](https://api.scryfall.com/cards/search?q=e%3ARAV&unique=prints&order=set)
  was used only to reproduce the RAV collector-number/name inventory documented
  above. The source snapshot is not checked in, and this use makes no ownership or
  endorsement claim.

These sources inform compatibility facts only. They do not grant a license to
redistribute copyrighted game content. The project makes no affiliation or
endorsement claim.

## License and marks

CardBench-authored code and manifest structure are released under the root
MIT license. `Magic: The Gathering`, set names, card names, and related marks
are owned by their respective rights holders. Their use here is nominative,
for interoperability and benchmark identification. Downstream users are
responsible for reviewing their own rights and distribution obligations.
