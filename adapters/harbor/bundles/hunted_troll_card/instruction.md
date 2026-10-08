# CardBench Magic single-card task: Hunted Troll

Implement Hunted Troll in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs`, including `definition()`, `activated_ability()`, and `triggered_ability()`.

Hunted Troll is a green 8/4 creature costing `{2}{G}{G}`. Its mandatory ETB ability targets an opponent who creates four blue 1/1 Faerie creature tokens with flying. Its `{G}` activation regenerates Hunted Troll.

The verifier uses two full 60-card constructed games. The first checks the opponent-targeted ETB and four live Faeries. The second replaces the opponent deck's Putrefy playset with Brainspoil, then responds to Brainspoil targeting Hunted Troll with regeneration. It checks shield creation and use, Troll's tapped state, and continued battlefield presence. Both traces verify explicit payments, ordered typed events, deterministic replay, and semantic controls. This is diagnostic-only: 291 names, 306 printings, zero card credit, zero reward.
