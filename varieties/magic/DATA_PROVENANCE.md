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

The Birds of Paradise full-fidelity slice retains only its public RAV identity,
collector number, mana-cost/color/type facts, base power/toughness, and
CardBench-authored semantic markers for Flying plus its generic one-unit,
chosen-color mana ability. It does not retain or reproduce card rules text,
art, flavor, or source JSON.

The Molten Sentry full-fidelity slice retains only its public RAV identity,
collector number, mana-cost/color/type facts, and the CardBench-authored
entry-time characteristic outcomes needed by its deterministic public scenario.
It stores no card rules text, art, flavor, scans, or source response payload.

The creature-chassis batch (Dromad Purebred, Carrion Howler, Coalhauler Swine,
Bramble Elemental, Carven Caryatid, Boros Swiftblade, Loxodon Hierarch, and Moroii)
retains only public RAV identity,
mana cost, color, type, and base power/toughness facts. Any printed ability is
intentionally absent from its compatibility definition. The public source
responses and their rules fields were not committed.

The Junktroller PvP diagnostic retains only the public RAV identity,
collector number, mana-cost/color/type facts, base power/toughness facts,
Defender marker, and the tap-only graveyard-to-owner-library-bottom ability.
The printed tap symbol was checked against the original RAV #264 card scan
linked from its task README; the scan itself is not stored in the repository.
The previously recorded `{2}` activation was unsupported by the scan and has
been removed from the authored model and task contract.

The Hunted Dragon PvP diagnostic retains only the public RAV #131 identity,
mana-cost/color/type facts, 6/6 characteristics, Flying and Haste markers, and
CardBench-authored semantics for the opponent-targeted Knight-token entry and
same-turn combat trace. Public identity and characteristics were cross-checked
against the public [Hunted Dragon RAV printing summary](https://mtg.wtf/card?q=%21Hunted+Dragon).
No card prose, image, scan, or source response is retained.

The Hunted Phantasm PvP diagnostic retains only the public RAV #55 identity,
mana-cost/color/type facts, 4/6 characteristics, unblockable marker, and
CardBench-authored semantics for its opponent-targeted Goblin-token entry and
later combat trace. Its public [RAV printing record](https://www.mtg.wtf/card/rav/55/Hunted-Phantasm)
was used to check identity and the scoped compatibility facts. No card prose,
image, scan, or source response is retained.

The Hunted Horror PvP diagnostic retains only the public RAV #90 identity,
black `{B}{B}` cost, Creature supertype, 7/7 characteristics, Trample marker,
and CardBench-authored semantics for its opponent-targeted entry trigger that
creates two green 3/3 Centaurs with Protection from Black. The public [RAV
printing record](https://www.mtg.wtf/card/rav/90) was checked for identity and
represented facts. Horror subtype is retained as a source fact, but the
current CardDefinition model cannot assert creature subtypes; the created
Centaur tokens are typed. No card prose, image, scan, or source response is
retained.

The Drake Familiar PvP diagnostic retains only the public RAV #44 identity,
mana-cost/color/type/subtype facts, 2/1 characteristics, Flying marker, and
CardBench-authored semantics for its target-free return-or-sacrifice entry
choice. The [RAV printing record](https://scryfall.com/card/rav/44/drake-familiar)
was checked for public identity and characteristics. The [RAV FAQ](https://www.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224968-rav-official-faq)
and the public card record's [2005 ruling](https://mtg.wtf/card/rav/44/Drake-Familiar)
establish that the resolution-time choice is non-targeting and can return any
battlefield enchantment, including an opponent-controlled or untargetable one,
to its owner's hand; declining or having no eligible enchantment requires the
source to be sacrificed. Current [Oracle wording](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87960)
modernizes the entry timing language while preserving these relevant semantics.
Only public identity and characteristics plus these CardBench-authored semantic
facts are retained; no card prose corpus, image, scan, or source response is stored.

The Hour of Reckoning PvP diagnostic retains only the public RAV #21 identity,
white `{4}{W}{W}{W}` Sorcery facts, Convoke marker, and CardBench-authored
semantics for destroying every nontoken creature while preserving creature
tokens. The public [RAV printing record](https://mtg.wtf/card/plst/RAV-21/Hour-of-Reckoning)
was checked for identity and the represented facts. No card prose, image, scan,
or source response is retained.

The Blockbuster PvP diagnostic retains only the public RAV #115 identity,
red `{3}{R}{R}` enchantment facts, and CardBench-authored semantics for its
`{1}{R}`, sacrifice-source activation that deals 3 damage to tapped creatures
and both players. The public [RAV printing record](https://www.lastdodo.com/en/items/829007-blockbuster)
was checked for identity and the represented facts; current Oracle was
consulted for the modern self-reference. No card prose, image, scan, or source
response is retained.

The Loxodon Hierarch PvP diagnostic retains only the public RAV #214 identity,
mana-cost/color/type facts, 4/4 characteristics, and CardBench-authored
semantics for its four-life entry trigger and `{G}{W}`, sacrifice-source
regeneration ability. The public [RAV printing record](https://scryfall.com/card/rav/214/loxodon-hierarch)
was checked for the Elephant Cleric subtype line as well; the current
CardDefinition model cannot assert subtypes, so runtime fidelity is limited to
the Creature supertype. No card prose, image, scan, or source response is retained.

The Stinkweed Imp PvP diagnostic retains only the public RAV #107 identity,
black `{2}{B}` cost, Creature supertype, 1/2 characteristics, Flying and
Dredge 5 markers, and CardBench-authored semantics for destroying the exact
creature it damages in combat. The public [RAV printing record](https://www.mtg.wtf/card/rav/107/Stinkweed-Imp)
was checked for identity and represented facts; Wizards' [Time Spiral Remastered
release notes](https://media.wizards.com/2021/downloads/TSR_Release_Notes/EN_MTGTSR_FAQ_20210118.pdf)
were consulted for current trigger-template interpretation. Imp subtype is a
public source fact, but the current CardDefinition model cannot assert it.
No card prose, image, scan, or source response is retained.

The Tidewater Minion PvP diagnostic retains only the public RAV #71 identity,
blue `{3}{U}{U}` cost, Creature supertype, 4/4 characteristics, Defender
marker, and CardBench-authored semantics for tapping to untap a target
permanent and paying `{4}` to lose Defender until end of turn. The public [RAV
printing record](https://mtg.wtf/card/rav/71/Tidewater-Minion) was checked for
identity and represented facts. Elemental and Minion subtypes are retained as
source facts, but the current CardDefinition model cannot assert creature
subtypes. No card prose, image, scan, or source response is retained.

The Golgari Brownscale PvP diagnostic retains only the public RAV #166
identity, green `{1}{G}{G}` cost, Creature supertype, 2/3 characteristics,
Dredge 2 marker, and CardBench-authored semantics for the two-life trigger on
its return from its owner's graveyard to hand. The public [RAV printing
record](https://www.mtg.wtf/card/rav/166/Golgari-Brownscale) was checked for
identity and represented facts. Lizard subtype is retained as a source fact,
but the current CardDefinition model cannot assert creature subtypes. No card
prose, image, scan, or source response is retained.

The Golgari Thug PvP diagnostic retains only the public RAV #87 identity,
black `{1}{B}` cost, Creature supertype, 1/1 characteristics, Dredge 4 marker,
and CardBench-authored semantics for its death trigger targeting a creature
card in its controller's graveyard and putting it on that card owner's library
top. The official [Wizards RAV card-gallery record](https://mtg-jp.com/products/card-gallery/0000057/88994/)
was checked for identity and printed trigger facts. Human Warrior subtype is
retained as public identity but cannot be asserted by the current
CardDefinition model. No card prose, image, scan, or source response is
retained.

The Stone-Seeder Hierophant PvP diagnostic retains only the public RAV #184
identity, green `{2}{G}{G}` cost, Creature supertype, 1/1 characteristics, and
CardBench-authored semantics for untapping itself when a land enters under its
controller's control and for tapping to untap a target land. Public [RAV
printing record](https://mtg.wtf/card/rav/184/Stone-Seeder-Hierophant) and
[Gnome Games listing](https://gnomegames.com/products/stone-seeder-hierophant-ravnica-city-of-guilds-13429)
were checked for identity and represented facts. Human and Druid subtypes are
retained as source facts, but the current CardDefinition model cannot assert
creature subtypes. No card prose, image, scan, or source response is retained.

The Undercity Shade PvP diagnostic retains only the public RAV #110 identity,
black `{4}{B}` cost, Creature supertype, 1/1 characteristics, and
CardBench-authored semantics for Fear's black-or-artifact blocker restriction
and its `{B}` self-pump. The public [RAV printing
record](https://www.cardmarket.com/en/Magic/Products/Singles/Ravnica-City-of-Guilds/Undercity-Shade)
and [current Oracle record](https://scryfall.com/card/rav/110/undercity-shade)
were checked for identity and represented facts. Shade subtype is retained as
a source fact, but the current CardDefinition model cannot assert creature
subtypes. No card prose, image, scan, or source response is retained.

The Darkblast PvP diagnostic retains only the public RAV #82 identity, black
`{B}` Instant facts, Dredge 3 marker, and CardBench-authored semantics for a
target creature's temporary -1/-1 modifier plus Dredge's three-card return
replacement. The official [Wizards RAV card-gallery
record](https://mtg-jp.com/products/card-gallery/0000057/87922/) and a [current
Oracle record](https://mtgseeker.com/card/rav/82/darkblast) were checked for
identity and represented facts. No card prose, image, scan, or source response
is retained.

The Muddle the Mixture PvP diagnostic retains only the public RAV #60 identity,
blue `{U}{U}` Instant facts, Transmute `{1}{U}{U}` cost marker, and
CardBench-authored semantics for countering a target instant or sorcery spell
or using Transmute to find a card with mana value two. The public [RAV printing
record](https://scryfall.com/card/rav/60/muddle-the-mixture) and [Magic
Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf)
were checked for identity and represented mechanics. No card prose, image,
scan, or source response is retained.

The Twilight Drover PvP diagnostic retains only the public RAV #33 identity,
mana-cost/color/type facts, 1/1 characteristics, and CardBench-authored
semantics for gaining a +1/+1 counter when another creature token leaves and
for removing one such counter to create two white flying Spirit tokens. The
public [RAV printing record](https://scryfall.com/card/rav/33/twilight-drover)
was checked for identity and the represented facts. The card's Spirit subtype
is retained as a source fact, but the current CardDefinition model cannot
assert creature subtypes; the generated Spirit token itself is typed. No card
prose, image, scan, or source response is retained.

The Vinelasher Kudzu PvP diagnostic retains only the public RAV #189 identity,
green `{1}{G}` cost, Creature supertype, 1/1 characteristics, and
CardBench-authored semantics for placing a +1/+1 counter when a land enters
under its controller's control. The public [RAV printing record](https://scryfall.com/card/rav/189/vinelasher-kudzu)
and [Gatherer record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83559)
were checked for identity and represented facts. Plant subtype is retained as
a source fact, but the current CardDefinition model cannot assert creature
subtypes. No card prose, image, scan, or source response is retained.

The Flame-Kin Zealot PvP diagnostic retains only the public RAV #206 identity,
mana-cost/color/type facts, 2/2 characteristics, and CardBench-authored
semantics for its controller's creature-wide temporary power/toughness and
Haste effect. The public [RAV printing record](https://www.mtg.wtf/card/rav/206/Flame-Kin-Zealot)
was checked for identity and these compatibility facts. Current Oracle wording
replaces the printing's “comes into play” terminology with “enters”; the scoped
resolution behavior also follows the rule that the affected creature set is
determined when the trigger resolves. No card image, rules-text corpus, or
source response is retained.

The Lore Broker PvP diagnostic retains only the public RAV #57 identity,
mana-cost/color/type facts, 1/2 characteristics, and CardBench-authored
semantics for its tap activation and each player's draw followed by private
discard choice. The public [RAV printing record](https://www.mtg.wtf/card/rav/57/Lore-Broker)
was checked for identity and the scoped behavior. The engine's current
CardDefinition model does not represent the printed Human Rogue subtype; this
task retains the card's Creature supertype but does not assert that subtype in
its gameplay trace. No card prose, image, scan, or source response is retained.

The Ghosts of the Innocent PvP diagnostic retains only the public RAV identity,
collector number, mana-cost/color/type facts, base power/toughness facts, and a
CardBench-authored damage-amount-halving binding. The card facts were checked
against the public [Scryfall RAV #20 record](https://scryfall.com/card/rav/20/ghosts-of-the-innocent).
Focused tests cover player and permanent damage and both affected-player
choices against Szadek's competing combat-damage replacement. The card's
represented characteristics and ability are included in the positive
full-fidelity manifest; no card prose, image, or source response is retained.

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

- The first per-card PvP pilot, Char (RAV collector 117), was source-reviewed on
  2026-09-23 against the RAV printing, with current Oracle wording reviewed as a
  cross-check. The mana-cost fact was also checked against the public
  [RAV printing record](https://scryfall.com/card/rav/117/char). This
  original-set compatibility task follows the RAV printing's
  player-or-creature target domain and deliberately excludes later Oracle
  target-domain errata. The retained CardBench-authored model covers only the public
  identity, red instant / mana-cost facts, the legal player-or-creature target
  branch, ordered damage instructions, stack resolution, and graveyard move.
  No card-text corpus, art, scan, flavor text, or source response was retained.
  The rules baseline is the official [Comprehensive Rules effective August 7,
  2026](https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf),
  the latest effective edition by the 2026-09-23 cutoff. Future-effective
  changes are excluded; see the official [rules page](https://magic.wizards.com/en/rules)
  for the current document.

- The second single-card PvP pilot, Lightning Helix (RAV collector 213), was
  source-reviewed on 2026-09-23 against the RAV printing, with current Oracle
  wording checked as a cross-check. Its retained CardBench-authored model covers
  only the public identity, red-white instant / mana-cost facts, player-or-creature
  target branch, ordered damage and controller life gain, stack resolution, and
  graveyard move. No card-text corpus, art, scan, flavor text, or source response
  was retained. The rules baseline and official-update cutoff match the Char
  pilot above. The mana-cost fact was checked against the public
  [RAV printing record](https://scryfall.com/card/rav/213/lightning-helix).

- The official [Magic rules page](https://magic.wizards.com/en/rules) is the
  authoritative source for the comprehensive rules framework.
- Thundersong Trumpeter (RAV collector 235) was checked on 2026-09-24 against
  the official [RAV Gatherer printing record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83611).
  The retained CardBench-authored model covers only its public identity,
  red-white cost, creature type, 2/1 characteristics, and temporary targeted
  combat-restriction operation. The source card text, flavor, imagery, and
  upstream payload were not retained. Rules behavior uses the Comprehensive
  Rules effective 2026-08-07 and the 2026-09-23 source cutoff.
- Autochthon Wurm (RAV collector 191) was checked on 2026-09-24 against the
  official [RAV Gatherer printing record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=89096).
  The retained CardBench-authored model covers only its public identity,
  mana-cost, color, type, 9/14 characteristics, Convoke and Trample keyword
  identities, plus the scoped convoke-payment and ordered multi-block trample
  combat behavior. Card prose, flavor, imagery, and upstream payload were not
  retained.
- The Gather Courage and Seeds of Strength fidelity checks, and the bounded
  Muddle the Mixture compatibility check, were performed on 2026-07-30
  against the public Comprehensive Rules framework: target occurrence and
  partial resolution (CR 115.3 and 608.2b), convoke (CR 702.51), and
  transmute (CR 702.53). Only the resulting CardBench-authored semantic model
  and public identifier facts are committed: no response payload, card rules
  prose, art, or scans were retained.
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
- The focused Darkblast/Scatter the Seeds/Siege Wurm/Guardian of Vitu-Ghazi
  fidelity audit was checked on 2026-07-30 against their public official
  Gatherer records ([Darkblast](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87922),
  [Scatter the Seeds](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87931),
  [Siege Wurm](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87932),
  and [Guardian of Vitu-Ghazi](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=89030)).
  The audit retained only the semantic facts needed to determine whether each
  executable substrate was complete. No source response, card prose, art, or
  JSON is checked in.
  Scatter the Seeds was re-audited after the engine added typed token creature
  subtypes; its positive manifest entry retains only CardBench-authored semantic
  operations and the public event-log baseline, not source card prose or data.
  Guardian of Vitu-Ghazi was re-audited after the engine added the
  expansion-neutral vigilance declaration substrate. Only the public identity,
  cost, color, creature type, base power/toughness, and two keyword facts needed
  by its direct contracts were retained, along with the public rules reference
  for vigilance; no card prose, art, or response payload is checked in.
- The focused Flying/Reach audit was checked on 2026-07-30 against public
  Gatherer records for [Conclave Equenaut](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87915),
  [Snapping Drake](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87986),
  [Goliath Spider](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=88959),
  and [Courier Hawk](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=87913),
  together with the public Comprehensive Rules' Flying, Reach, and vigilance
  keyword sections. The retained facts are only the public identity, cost,
  color, creature type, base power/toughness, and keyword identities needed
  for the direct contracts. No card prose, art, scans, source JSON, or response
  payload is checked in.
- The focused Haste audit was checked on 2026-07-30 against the public
  [Skyknight Legionnaire RAV printing record](https://scryfall.com/card/rav/232/skyknight-legionnaire)
  and the official [Magic keyword glossary](https://magic.wizards.com/en/keyword-glossary).
  It retained only the public identity, cost, color, creature type, base
  power/toughness, and the two represented generic keyword identities. The
  source response, card prose, art, scans, and JSON remain uncommitted. The
  other RAV Haste candidates retain unrepresented non-keyword functionality and
  therefore remain bounded rather than being approximated.
- The Molten Sentry entry-characteristics audit was checked on 2026-08-03
  against its public [RAV printing record](https://scryfall.com/card/rav/136/molten-sentry).
  It retained only public identity, collector number, mana cost, color, type,
  and the two modeled power/toughness-plus-keyword outcomes required by the
  deterministic entry replacement. The source response, card prose, art,
  scans, and JSON remain uncommitted.
- The RAV basic-land boundary audit was checked on 2026-07-30 against the
  public official Gatherer records for representative RAV printings:
  [Plains](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=95108),
  [Island](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=95103),
  [Swamp](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=95111),
  [Mountain](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=95096),
  and [Forest](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=95097).
  The public Comprehensive Rules' basic-land and mana-ability sections were
  also consulted. Only the semantic identity, basic-land status, typed land
  identity, and one-color mana result required by the represented definitions are
  retained; no card prose, art, or response payload is checked in.
- The Glass Golem compatibility facts were checked on 2026-07-30 against a
  single public [Scryfall named-card response](https://api.scryfall.com/cards/named?exact=Glass%20Golem),
  corroborated by the RAV inventory. The checked fields were identity, set,
  collector number, mana cost, type, power, toughness, and whether the public
  rules field was empty. That last fact permits its vanilla compatibility
  definition without silently dropping a printed ability; the response and
  any card text remain uncommitted.
- The Watchwolf compatibility facts were checked on 2026-07-30 against a
  public [Scryfall named-card response](https://api.scryfall.com/cards/named?exact=Watchwolf),
  corroborated by the RAV inventory. The checked fields were identity, set,
  collector number, mana cost, type, power, toughness, and whether the public
  rules field was empty. The response and any card text remain uncommitted.
- The Nightguard Patrol fidelity check was performed on 2026-07-30 against its
  public [RAV printing record](https://scryfall.com/card/rav/26/nightguard-patrol).
  It retained only identity, set, collector number, mana cost, color, type,
  base power/toughness, and the presence of the two represented generic combat
  keywords. The source response, card prose, art, and JSON remain uncommitted.
- The creature-chassis compatibility facts were checked on 2026-07-30 against
  public [Scryfall named-card responses](https://api.scryfall.com/cards/named)
  and corroborated with the RAV catalog. Only identity, set, collector number,
  mana cost, color, type, power, and toughness fields were used; the responses
  and all card text remain uncommitted.
- The public [Scryfall cards API](https://api.scryfall.com/cards/search?q=e%3ARAV&unique=prints&order=set)
  was used only to reproduce the RAV collector-number/name inventory documented
  above. The source snapshot is not checked in, and this use makes no ownership or
  endorsement claim.

- The Galvanic Arc card facts were checked against the public [RAV printing record](https://scryfall.com/card/rav/126/galvanic-arc). Only its mana cost, color, Aura status, creature enchant restriction, entry damage, and attached-creature first-strike effect are retained in the implementation; card prose and art are not copied.
- The Ethereal Usher card facts were checked against the public [RAV card record](https://mtg.wtf/card/rav/47/Ethereal-Usher). Only its mana cost, color, creature stats, unblockable activation, and Transmute cost are retained; the Spirit subtype is outside the current card-definition model.
- The Shambling Shell card facts were checked against the public [RAV card record](https://scryfall.com/card/rav/230/shambling-shell). Its black-green cost, 3/1 creature characteristics, Dredge 3, and sacrifice-for-counter ability are routed through its candidate-owned module; Plant and Zombie subtypes remain outside the current card-definition model.
- The Halcyon Glaze card facts were checked against the public [RAV card record](https://mtg.wtf/card/rav/54/Halcyon-Glaze). Its blue enchantment characteristics and creature-spell trigger that animates its source until end of turn are routed through its candidate-owned module. The old printing's “play” wording is represented as casting a creature spell, and the animation adds Creature while retaining Enchantment.
- The Voyager Staff card facts were checked against the public [RAV card record](https://mtg.wtf/card/rav/274/Voyager-Staff). Its colorless artifact characteristics and `{2}`, sacrifice, target creature, linked exile, and next-end-step return behavior are routed through its candidate-owned module.
- The Mnemonic Nexus card facts were checked against the public [RAV #59 printing record](https://scryfall.com/card/rav/59/mnemonic-nexus). Its blue `{3}{U}` instant identity and owner-relative graveyard shuffle instruction are routed through its candidate-owned module.
- The Grave-Shell Scarab card facts were checked against the public [RAV #211 card record](https://scryfall.com/card/rav/211/grave-shell-scarab). The candidate-owned module retains its black-green `{2}{B}{G}{G}` 4/4 identity, Dredge 1, and generic `{1}` sacrifice-to-draw activation. The shared draw-replacement path also supports Dredge during spell-effect draws. No full card text, art, or source payload is retained.
- The Copy Enchantment card facts were checked against the public [RAV #42 card record](https://mtg.wtf/card/rav/42/Copy-Enchantment). Its blue `{2}{U}` enchantment identity is routed through a candidate-owned module, while its optional entry-copy behavior remains in the typed shared entry-copy binding.
- The Privileged Position card facts were checked against the public [RAV #251 card record](https://mtg.wtf/card/rav/251/Privileged-Position). Its hybrid green-white enchantment identity and layer-six hexproof grant to other permanents its controller controls are routed through candidate-owned and typed shared static-effect bindings.
- The Drift of Phantasms card facts were checked against the public [RAV #46 card record](https://mtg.wtf/card/rav/46/Drift-of-Phantasms). Its blue `{2}{U}` 0/5 creature identity, Defender, Flying, and `{1}{U}{U}` Transmute are routed through a candidate-owned module; Transmute uses the shared private-search and shuffle lifecycle. Spirit subtype representation remains outside the current card-definition model.
- The Agrus Kos, Wojek Veteran card facts were checked against the public [RAV #190 card record](https://mtg.wtf/card/rav/190/Agrus-Kos-Wojek-Veteran). Its red-white `{3}{R}{W}` 3/3 identity and attack trigger are routed through a candidate-owned module; the shared trigger and continuous-effect machinery applies each color modifier to declared attackers of that color.
- The Primordial Sage card facts were checked against the public [RAV #177 card record](https://mtg.wtf/card/rav/177/Primordial-Sage). Its green `{4}{G}{G}` 4/5 identity and optional controller-creature-spell draw trigger are routed through a candidate-owned module.
- The Stasis Cell card facts were checked against the public [RAV #66 card record](https://mtg.wtf/card/rav/66/Stasis-Cell). Its blue `{4}{U}` Aura restricts its enchanted creature from attacking or blocking, and its `{3}{U}` activated ability can reattach it to a target creature; both bindings are candidate-owned.
- The Sadistic Augermage facts were checked against the public [RAV #103 card record](https://mtg.wtf/card/rav/103/Sadistic-Augermage). Its black `{2}{B}` 3/1 identity, mandatory own-death trigger, and private one-card choice for each player's library top are routed through a candidate-owned module and shared typed decision effect.
- The Dimir Infiltrator card facts were checked against the public [RAV #203 card record](https://mtg.wtf/card/rav/203/Dimir-Infiltrator). Its blue-black `{U}{B}` 1/3 identity, unblockability, and `{1}{U}{B}` Transmute ability are routed through a candidate-owned module.
- The Induce Paranoia card facts were checked against the public [RAV #56 card record](https://aetherhub.com/Card/RAV/Induce%20Paranoia/56). The candidate-owned definition declares its blue `{2}{U}{U}` instant identity and conditional counter-then-mill operation.
- The Followed Footsteps card facts were checked against the public [RAV #51 card record](https://mtg.wtf/card/rav/51/Followed-Footsteps). Its blue Aura identity, creature attachment, Aura controller's upkeep trigger, and token-copy effect are routed through its candidate-owned module.
- The Grifter's Blade card facts were checked against public [RAV card records](https://starcitygames.com/grifters-blade-sgl-mtg-rav-263-enf/) and cross-checked against the [current card record](https://www.cardkingdom.com/mtg/ravnica/grifters-blade). Retained facts are its RAV identity, cost, type, Flash, non-targeting entry attachment choice, equipped-creature bonus, and Equip cost; no image or complete card-text corpus is retained.
- The Indentured Oaf card facts were checked against the public [RAV #133 card record](https://aetherhub.com/Card/RAV/Indentured-Oaf/133). The retained correction is that this source prevents damage it would deal to red creatures; it does not prevent incoming damage from red sources.
- Empty the Catacombs card facts were checked against the public [RAV #86 card record](https://mtg.wtf/card/rav/86/Empty-the-Catacombs) and [AetherHub printing record](https://aetherhub.com/Card/RAV/Empty%20the%20Catacombs/86). Its black `{3}{B}` sorcery definition returns every creature card from each player's own graveyard to that player's hand without a card-selection choice. The former one-card-per-graveyard model was incorrect. No card text corpus, image, or source response is retained.
- Dream Leash card facts were checked against the original [RAV #45 card record](https://mtg.wtf/card/rav/45/Dream-Leash) and its current Oracle interpretation. The tapped restriction applies when the Aura spell is cast; an untap before resolution does not make that target illegal. Its continuous control effect follows the Aura's current controller. No card text corpus, image, or source response is retained.
These sources inform compatibility facts only. They do not grant a license to
redistribute copyrighted game content. The project makes no affiliation or
endorsement claim.

## License and marks

CardBench-authored code and manifest structure are released under the root
MIT license. `Magic: The Gathering`, set names, card names, and related marks
are owned by their respective rights holders. Their use here is nominative,
for interoperability and benchmark identification. Downstream users are
responsible for reviewing their own rights and distribution obligations.
