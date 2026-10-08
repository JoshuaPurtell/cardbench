# Stone-Seeder Hierophant source reconciliation

The compatibility target is the original Ravnica: City of Guilds printing, collector #184. The RAV card is a green `{2}{G}{G}` 1/1 Creature — Human Druid. Its printed abilities are a controller-land-entry trigger that untaps Stone-Seeder Hierophant and `{T}: Untap target land.` The card has no mana ability.

Current Oracle wording labels the trigger Landfall and uses “a land you control enters” and “this creature.” Those are terminology/self-reference updates; both wordings restrict the event to a land entering under the creature controller's control. The implementation uses `ControlledLandEntersBattlefield` and `UntapSource`. An opponent's land entry does not trigger it. The activated ability remains target-bearing, stack-using, and restricted to a land permanent.

`CardDefinition` currently has no creature-subtype field. Human and Druid are retained in `features.toml` but are not asserted by executable state.

Sources:

- [RAV #184 printing text transcription](https://gnomegames.com/products/stone-seeder-hierophant-ravnica-city-of-guilds-13429)
- [Current card record and Oracle wording](https://mtg.wtf/card/rav/184/Stone-Seeder-Hierophant)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No card image, scan, flavor text, full rules-text corpus, or upstream response payload is stored in this task.
