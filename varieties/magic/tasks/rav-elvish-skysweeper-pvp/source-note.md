# Elvish Skysweeper source reconciliation

The compatibility target is the original Ravnica: City of Guilds printing, collector #162. It is a green `{G}` 1/1 Creature — Elf Warrior with `{4}{G}, Sacrifice a creature: Destroy target creature with flying.` The later Oracle wording retains the same cost, sacrifice, target restriction, and destruction effect; no functional reconciliation is needed.

The current `CardDefinition` model represents the Creature type but does not assert creature subtypes. `features.toml` preserves Elf and Warrior as printed identity metadata and sets `runtime_subtypes_asserted = false`.

Sources:

- [Scryfall RAV #162 entry](https://scryfall.com/card/rav/162/elvish-skysweeper)
- [Wizards Gatherer record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83679)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No image, scan, flavor, or rules-text corpus is included in this task.
