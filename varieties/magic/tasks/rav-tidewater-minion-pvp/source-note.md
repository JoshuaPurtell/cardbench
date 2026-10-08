# Tidewater Minion source reconciliation

The compatibility target is the original Ravnica: City of Guilds printing, collector #71. It is a blue `{3}{U}{U}` 4/4 Creature — Elemental Minion with Defender and two activated abilities:

- `{4}: Tidewater Minion loses defender until end of turn.`
- `{T}: Untap target permanent.`

Current Oracle wording uses “This creature” in the first ability, while the printed text names Tidewater Minion. The effect is source-relative in both wordings. The printed `{4}` generic cost and `{T}` tap cost are retained exactly. The Defender ability is not sorcery-speed and has no target; the untap ability targets any permanent and taps its source as an activation cost.

The engine currently models card types but not creature subtypes. “Elemental Minion” is preserved in `features.toml` and this source note; runtime `CardDefinition` asserts `Creature` only. The authority's decisive target is the opponent's tapped Boros Signet, which demonstrates a legal nonland permanent target and a real untap state change.

Sources:

- [Original RAV #71 record and current Oracle text](https://scryfall.com/card/rav/71/tidewater-minion)
- [Wizards Gatherer record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83679)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No image, scan, flavor, or rules-text corpus is included.
