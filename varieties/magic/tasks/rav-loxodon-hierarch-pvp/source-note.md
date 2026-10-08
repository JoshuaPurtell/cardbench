# Loxodon Hierarch source reconciliation

The compatibility target is **Loxodon Hierarch**, Ravnica: City of Guilds (RAV) collector #214. The original printing identifies a white-green 4/4 Creature — Elephant Cleric with mana cost `{2}{G}{W}`. Its rules text is “When Loxodon Hierarch comes into play, you gain 4 life. `{G}{W}`, Sacrifice Loxodon Hierarch: Regenerate each creature you control.” The source records the RAV card identity, mana cost, colors, creature body, subtype line, entry trigger and activated ability needed for this task.

Current Oracle wording uses “When this creature enters” and “Sacrifice this creature.” Those are wording updates for the same battlefield-entry event and source sacrifice. The mana cost, 4 life amount, `{G}{W}` activation cost, affected set (“each creature you control”), and regeneration effect match. The engine's `CardDefinition` currently has no subtype field; the candidate represents only `CardType::Creature`, so Elephant and Cleric are retained here as source identity facts but are not asserted by executable state. No subtype behavior is scored.

The task also checks the 2005 ruling that the activation creates separate regeneration shields for each creature controlled by the ability's controller, and that the shields are available only for the rest of the turn. The task uses a shield to replace lethal destruction from the opponent's Char after the Hierarch has been sacrificed as the activation cost. It does not infer rules from flavor text or use an image as grading authority.

Sources:

- [RAV printing record](https://scryfall.com/card/rav/214/loxodon-hierarch)
- [Wizards Gatherer search for the card](https://gatherer.wizards.com/Pages/Search/Default.aspx?name=+[Loxodon]+[Hierarch])
- [Current Oracle wording and the 2005 regeneration ruling](https://mtg.wtf/card/ddh/15/Loxodon-Hierarch)
- [RAV source contract](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md)

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective August 7, 2026, with authority cutoff September 24, 2026. No card scan, image, full rules-text corpus, or upstream response payload is stored in this task.
