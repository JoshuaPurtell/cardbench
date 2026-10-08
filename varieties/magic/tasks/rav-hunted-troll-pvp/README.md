# CardBench Magic single-card task: Hunted Troll

Implement Hunted Troll's green 8/4 creature definition, its opponent-only ETB trigger, and its green self-regeneration activation in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs`.

The diagnostic authority runs two legal 60-card Golgari games. The first casts Hunted Troll and proves that the chosen opponent receives exactly four live blue 1/1 Faerie creature tokens with flying. The second replaces the opponent deck's Putrefy playset with Brainspoil, then responds to Brainspoil targeting the nonblack Troll with `{G}` regeneration. It proves the shield replaces Brainspoil's ordinary destroy event, taps Hunted Troll, and leaves it on the battlefield. Both traces check explicit cast/payment, ordered typed events, live characteristics, and deterministic replay; semantic controls mutate the body, target, token, trigger, and regeneration cost.

The task is diagnostic-only against 291 distinct RAV names and 306 printings. It awards zero card credit and zero reward; candidate evaluation is separate.
