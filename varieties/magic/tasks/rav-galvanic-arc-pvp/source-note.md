# RAV source note: Galvanic Arc

The Ravnica: City of Guilds #126 printing is Galvanic Arc, `{2}{R}`, Enchantment — Aura, `Enchant creature`; its functional text says that when Galvanic Arc enters the battlefield, it deals 3 damage to target creature or player, and that the enchanted creature has First Strike. This task retains that RAV target boundary as `PlayerOrCreature` and grants First Strike only while the exact Aura attachment is live. The engine models the Aura as `CardType::Enchantment` with typed attachment behavior; its card type model cannot store the printed Aura subtype separately.

Current Oracle wording modernizes the trigger to “When Galvanic Arc enters, it deals 3 damage to any target.” The RAV compatibility contract retains the printed creature-or-player target boundary instead of silently broadening it in this engine slice. The timing phrase is also modernized, without changing the enters-the-battlefield trigger. No other difference is claimed.

Public printing reference: https://scryfall.com/card/rav/126/galvanic-arc

Retained facts only: public name/set/collector number, mana cost, color, Enchantment/Aura identity, Enchant creature, ETB target boundary and damage amount, and attached First Strike. No artwork, scan, flavor text, or bulk rules-text corpus is retained.
