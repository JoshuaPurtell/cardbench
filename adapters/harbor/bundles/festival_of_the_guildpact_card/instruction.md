# CardBench Magic single-card task: Festival of the Guildpact

Implement the RAV Festival of the Guildpact module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/festival_of_the_guildpact.rs`.

Festival is a white Instant with cost `{X}{W}`. Its controller chooses and pays X, prevents the next X damage that would be dealt to them this turn, then draws a card. Preserve the candidate module boundary and the printed instruction order.

The trusted authority plays a shuffled 60-card duel, chooses X=4, pays one white plus four generic mana, verifies a four-damage prevention shield against an opponent's Char and the subsequent draw, then continues through combat to a terminal seat-zero win. Follow the task's CardBench-authored semantic summary. This diagnostic awards no RAV set credit or Harbor reward.
