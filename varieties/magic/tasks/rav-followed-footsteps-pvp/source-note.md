# Source note: Followed Footsteps

RAV collector #51 is a blue `{3}{U}{U}` Enchantment — Aura with “Enchant creature.” Its original Ravnica wording says: “At the beginning of your upkeep, put a creature token into play that's a copy of enchanted creature.” Here “your” refers to the controller of Followed Footsteps, because that player controls the triggered ability. The controller creates the token even when another player controls the enchanted creature. This task retains that printed timing and control scope.

Current Oracle wording modernizes “put a creature token into play” to “create a token,” but the Aura-controller upkeep timing and token-copy behavior remain the same. This task does not use the distinct “enchanted creature's controller's upkeep” behavior.

The candidate `CardDefinition` cannot represent the Aura subtype by itself. The candidate-owned `AttachmentBinding` identifies an Aura that enchants a creature, and the triggered binding uses `BeginningOfUpkeep` plus the attached-creature snapshot effect. The full-game trace deliberately attaches the P0-controlled Aura to a P1-controlled Watchwolf to prove upkeep scope, trigger control, token control, and copied source incarnation separately.

Public references:

- [RAV Official FAQ with original RAV wording](https://www.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224968-rav-official-faq)
- [Original-text column and RAV printing record](https://manabase.com/card.php?name=Followed+Footsteps)
- [Current Oracle text](https://mtg.wtf/card/rav/51/Followed-Footsteps)
- [RAV set catalog row](../../sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv)
- [Shared RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Only public identity, mana cost, color, card type, subtype, rules behavior, and identifiers are retained. No card image, scan, flavor text, or rules-text corpus is bundled.
