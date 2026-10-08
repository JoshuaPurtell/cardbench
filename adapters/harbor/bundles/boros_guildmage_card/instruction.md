# CardBench Magic single-card task: Boros Guildmage

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_guildmage.rs` as a literal-only declarative module with `definition()`, `grant_haste_ability()`, and `grant_first_strike_ability()`.

The printed creature is red-white, `{R/W}{R/W}`, 2/2. Its untapped, non-sorcery-speed activated abilities target one creature: `{R}` grants Haste and `{W}` grants First Strike until end of turn. Do not put either ability binding outside this candidate-owned module.

The verifier casts Guildmage from a shuffled legal 60-card deck, pays one red and one white for its hybrid symbols, activates both abilities against Guildmage itself, and checks each colored payment and ordered stack resolution. Guildmage must attack the turn it enters after gaining Haste. An opposing 1/1 Elf blocks it; First Strike damage must kill the blocker before normal combat damage, with no damage from the blocker to Guildmage. The authority includes separate effect-removal and colored-cost mutation controls. The report is diagnostic-only and credits no RAV coverage or Harbor reward.
