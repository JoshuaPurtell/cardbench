# Vinelasher Kudzu source reconciliation

The compatibility target is the original Ravnica: City of Guilds printing, collector #189. It identifies a green `{1}{G}` 1/1 Creature — Plant. Its printed ability is “Whenever a land comes into play under your control, put a +1/+1 counter on Vinelasher Kudzu.”

Current Oracle wording labels this Landfall and says “Whenever a land you control enters, put a +1/+1 counter on this creature.” The wording is updated, while the event condition, controller scope, counter kind and amount, and affected source remain the same. The implementation uses the engine's `ControlledLandEntersBattlefield` trigger and `AddPlusOneCounterToSource` effect. Opponent-controlled land entries do not satisfy that condition.

`CardDefinition` currently models the card type but has no creature-subtype field. The Plant subtype is therefore retained as printed identity metadata in `features.toml`; it is not asserted by executable state.

Sources:

- [RAV printing record and current Oracle text](https://scryfall.com/card/rav/189/vinelasher-kudzu)
- [Wizards Gatherer record](https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=83559)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No card image, scan, full rules-text corpus, or upstream response payload is stored in this task.
