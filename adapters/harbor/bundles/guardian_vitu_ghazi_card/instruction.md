# CardBench Magic single-card task: Guardian of Vitu-Ghazi

Implement Guardian of Vitu-Ghazi in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/guardian_of_vitu_ghazi.rs` by returning the correct declarative `CardDefinition`.

Guardian of Vitu-Ghazi is a green-white 4/7 Creature with mana cost `{6}{G}{W}`, Convoke, and Vigilance. In the shuffled 60-card game, Scatter the Seeds creates three Saprolings alongside three Elves. Six distinct creatures each contribute one Generic mana through Convoke; the spell then pays exactly one Green and one White mana.

The authority checks the ordered typed payment, cast, resolution, entry, attack, blocker, combat-damage, and game-end events. It verifies the live 4/7 keywords at entry and attack and confirms that Guardian remains untapped when it attacks. Deterministic replay and compiled mutants cover Convoke, generic cost, and Vigilance. The report is diagnostic-only with one of 291 names passing out of 306 printings, zero credit, and zero reward.
