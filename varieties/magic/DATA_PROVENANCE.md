# Magic fixture provenance and rights

## What is in this repository

- CardBench-authored Rust source that models generic game rules and a small
  executable RAV compatibility slice.
- Public set identifiers, names, color/type/mana-value facts, and compact
  machine-executable effect keys needed by the published scenarios.
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

## Sources consulted

- The official [Magic rules page](https://magic.wizards.com/en/rules) is the
  authoritative source for the comprehensive rules framework.
- Wizards of the Coast's [Ravnica design history](https://magic.wizards.com/en/news/making-magic/city-planning-part-iii-2005-09-19)
  confirms the original set's guild/mechanic structure: Boros, Dimir, Golgari,
  and Selesnya; radiance, transmute, dredge, and convoke.
- The original-block 4–3–3 guild allocation is described in the official
  [Ravnica block design history](https://magic.wizards.com/en/news/making-magic/city-planning-part-ii-2005-09-12).

These sources inform compatibility facts only. They do not grant a license to
redistribute copyrighted game content. The project makes no affiliation or
endorsement claim.

## License and marks

CardBench-authored code and manifest structure are released under the root
MIT license. `Magic: The Gathering`, set names, card names, and related marks
are owned by their respective rights holders. Their use here is nominative,
for interoperability and benchmark identification. Downstream users are
responsible for reviewing their own rights and distribution obligations.
