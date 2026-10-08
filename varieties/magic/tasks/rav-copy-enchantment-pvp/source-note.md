# Source note: Copy Enchantment

RAV collector #42 is a blue `{2}{U}` Enchantment. Its Ravnica rules text uses the “comes into play” formulation: as Copy Enchantment comes into play, its controller may choose an enchantment in play; if they do, Copy Enchantment comes into play as a copy of that enchantment. Current Oracle uses “enters the battlefield” wording for the same choice. This task retains the RAV mana cost, color, card type, optional choice, and copy-on-entry behavior; it does not replace the printed contract with later wording.

The reference fixture chooses an existing Fists of Ironwood Aura, then chooses the controlled Watchwolf as the copied Aura's attachment. The task also declines the optional choice on a second Copy Enchantment. Copying Fists of Ironwood copies its copiable characteristics and its printed triggered ability, so its own ETB trigger creates two Saprolings. The target must remain a legal Aura attachment throughout. Decisions and receipts are public game events; their source and target object incarnations are checked independently.

`CardDefinition` does not directly encode enchantment subtypes or a built-in copy effect. The candidate definition retains the supported-rules tags, while the set-owned `EntryCopyBinding` supplies the typed as-enters choice. The task reviews that binding and the engine's copy/attachment/event implementation alongside the candidate.

Public references:

- [RAV card record and current Oracle text](https://mtg.wtf/card/rav/42/Copy-Enchantment)
- [RAV set catalog row](../../sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv)
- [Shared RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Only public card identity, mana cost, color, type, rules behavior, and card-definition identifiers are retained. No card image, scan, flavor text, or rules-text corpus is bundled.
