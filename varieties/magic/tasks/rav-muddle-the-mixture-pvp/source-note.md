# Muddle the Mixture source reconciliation

- Ravnica: City of Guilds collector number: 60.
- Printed identity retained: blue Instant, mana cost `{U}{U}`; “Counter target instant or sorcery spell”; Transmute `{1}{U}{U}`.
- The RAV Transmute reminder text uses “converted mana cost.” Current Oracle wording uses “mana value.” This is a terminology update and does not change the card's mana-value comparison or the Transmute cost/timing.
- Current `CardDefinition` expresses the printed cost, blue color, Instant type, exact counter-target restriction, and Transmute cost. It does not encode printed reminder text as a separate field.

Sources:

- [Scryfall: RAV #60 Muddle the Mixture](https://scryfall.com/card/rav/60/muddle-the-mixture) — printing identity, cost, card text, and collector number.
- [Magic Comprehensive Rules, effective 2026-08-07](https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf) — spell casting/targets, countering, searching and shuffling, and Transmute rules.

No art, scan, flavor, or rules-text corpus is part of this task package.
