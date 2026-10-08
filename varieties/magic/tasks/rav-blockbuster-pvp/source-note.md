# Blockbuster #115 source note

## Retained facts

The RAV printing identifies Blockbuster as a red enchantment with mana cost `{3}{R}{R}`. Its activated operation costs `{1}{R}`, sacrifices Blockbuster itself, and deals 3 damage to each tapped creature and each player. The task retains only those public identity and rules facts needed by the candidate definition and trace; it stores no card image, scan, flavor text, or long rules-text corpus.

## Source audit

- RAV printing text transcription and set identity: [LastDodo, Blockbuster 115/306](https://www.lastdodo.nl/nl/items/829007-blockbuster). Its listing gives the RAV set, collector number, `{3}{R}{R}` card cost, enchantment type, `{1}{R}` activation, sacrifice of Blockbuster, and the global damage effect.
- Current Oracle rendering and collector number: [mtg.wtf, Blockbuster](https://mtg.wtf/card/rav/115/Blockbuster). Oracle changes the self-reference to “this enchantment.” Cost, source-sacrifice operation, damage amount, tapped-creature restriction, and both-player damage are unchanged.

The implementation follows the RAV printing. The catalog's legacy semantic/ability label `sacrifice-tapped-creature-and-player-damage` is misleading because it may be read as sacrificing a creature. The actual printed cost sacrifices Blockbuster. The candidate module uses a clarified card-local label and the established engine source-sacrifice field.
