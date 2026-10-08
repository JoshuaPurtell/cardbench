# Halcyon Glaze source note

Ravnica: City of Guilds #54 is a blue `{1}{U}{U}` Enchantment. The original RAV printing says: “Whenever you play a creature spell, Halcyon Glaze becomes a 4/4 Illusion creature with flying until end of turn.” Current Oracle says: “Whenever you cast a creature spell, this enchantment becomes a 4/4 Illusion creature with flying in addition to its other types until end of turn.”

The differences are the rules-term update from “play” to “cast,” the name-to-self reference update, and an explicit statement that its other card types remain. The task uses the printed RAV identity and the current rules meaning: this is a mandatory, controller-scoped cast trigger; it resolves above the creature spell that caused it; it adds Creature and Illusion, sets 4/4, grants Flying, and expires at Cleanup without removing Enchantment or changing the permanent's blue color.

The candidate-owned trigger is `CastsCreatureSpell` with `AnimateSourceIntoCreatureUntilEndOfTurn`. The effect adds Creature and Illusion, preserves Enchantment, retains Blue, sets power/toughness to 4/4, and adds Flying. The Creature subtype is represented only while animated because the base `CardDefinition` does not encode subtypes.

Public source references:

- Current card record and Oracle wording: https://mtg.wtf/card/rav/54/Halcyon-Glaze
- RAV printing text: https://www.lastdodo.com/en/items/829225-halcyon-glaze

Retained facts: public card name, RAV collector number, printed/current rules wording, mana cost, color, Enchantment type, Illusion animation subtype, 4/4 values, Flying, trigger timing, type preservation, and end-of-turn duration. No artwork, scan, flavor text, or bulk card-text corpus is retained.
