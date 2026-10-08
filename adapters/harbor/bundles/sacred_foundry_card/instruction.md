# CardBench Magic single-card task: Sacred Foundry

Implement RAV #280 in varieties/magic/sets/ravnica_city_of_guilds/src/cards/sacred_foundry.rs. Keep the module to the candidate-owned card definition, white/red mana binding, and optional entry-life binding.

The trusted verifier runs a shuffled 60-card two-player duel. It requires one Sacred Foundry to enter untapped after its controller pays two life and a second to enter tapped after the payment is declined. The untapped copy must produce white that is spent to cast Gate Hound ({2}{W}) and red that is spent for the red half of Boros Recruit's {R/W} hybrid cost. Island and Swamp basics provide the two generic mana for Gate Hound. The full game reaches a terminal result and replays exactly. This task awards no card credit or Harbor reward.
