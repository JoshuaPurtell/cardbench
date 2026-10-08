# Indentured Oaf source note

Ravnica: City of Guilds #133 is a red `{3}{R}` 4/3 creature with type line Creature — Ogre Warrior. The original printing says: “Prevent all damage that Indentured Oaf would deal to red creatures.” Current Oracle wording says “Prevent all damage that this creature would deal to red creatures.” This is a wording modernization only; the task preserves the printed source identity and source-to-target direction.

The engine representation is `PreventDamageToCreaturesOfColor(Red)` on Oaf's exact damage-source characteristics. It prevents Oaf's damage when the target is a red creature; it does not prevent incoming damage from a red source to Oaf. The full-game trace exercises both directions in the same combat. The current card-definition model cannot encode subtypes; Ogre Warrior is retained as public identity metadata and not claimed as executable behavior.

Public source: https://aetherhub.com/Card/RAV/Indentured-Oaf/133

Retained facts: public card name, RAV collector number, `{3}{R}` cost, red color, 4/3 creature characteristics, Ogre Warrior subtype, and source-bound prevention direction. No artwork, scan, flavor text, or bulk card-text corpus is retained.
