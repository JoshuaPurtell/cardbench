# Source note: Agrus Kos, Wojek Veteran

Ravnica: City of Guilds #190 is a legendary creature — Human Soldier with mana cost `{3}{R}{W}`, red and white colors, and base power/toughness 3/3. Its attack trigger grants attacking red creatures +2/+0 and attacking white creatures +0/+2 until end of turn. A red-white attacker receives both effects. The set FAQ explicitly confirms this, including for Agrus Kos itself.

The engine's `CardDefinition` stores creature identity and base characteristics; the set's legendary-permanent binding carries the legendary supertype. This module owns the printed definition and the attack-trigger binding.

Source: [Ravnica: City of Guilds Official FAQ](https://www.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224968-rav-official-faq), card-specific notes for Agrus Kos, Wojek Veteran. The FAQ reproduces the printed card text and clarifies the simultaneous red and white bonuses.

Cross-check: [mtg.wtf RAV card record](https://mtg.wtf/card/rav/190/Agrus-Kos-Wojek-Veteran).
