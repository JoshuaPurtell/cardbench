# Undercity Shade source reconciliation

The compatibility target is the Ravnica: City of Guilds printing, collector #110. The card is a black `{4}{B}` 1/1 Creature — Shade with Fear and a black-mana self-pump that lasts until end of turn. Fear's RAV reminder identifies artifact creatures and black creatures as eligible blockers. The authority therefore challenges the nonblack, nonartifact green-white Watchwolf and requires the game to reject the illegal block without recording a move.

Current Oracle records retain Fear, the artifact/black blocker exception, and the same `{B}` self-pump. Records that render the pump as a self-reference instead of repeating the card name do not change the source or effect. The candidate module implements the RAV printing's gameplay behavior and uses `Keyword::BlackEvasion`, whose combat rules include both printed legal blocker categories.

`CardDefinition` currently has no creature-subtype field. Shade is retained in `features.toml` but is not asserted by executable state.

Sources:

- [RAV #110 printing record](https://www.cardmarket.com/en/Magic/Products/Singles/Ravnica-City-of-Guilds/Undercity-Shade)
- [Current Oracle card record](https://scryfall.com/card/rav/110/undercity-shade)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No card image, scan, flavor text, full rules-text corpus, or upstream response payload is stored in this task.
