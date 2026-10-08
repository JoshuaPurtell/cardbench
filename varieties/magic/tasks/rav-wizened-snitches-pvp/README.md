# Wizened Snitches full-game card task

Implement the candidate-owned RAV Wizened Snitches definition and global top-library reveal binding in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wizened_snitches.rs`. Wizened Snitches is collector #75, a blue 1/3 Faerie Rogue with Flying costing `{3}{U}`. While it is on the battlefield, each player plays with the top card of each library revealed.

The isolated authority casts the card from an ordinary shuffled 60-card deck in a two-player duel. Both players' typed views must expose both current library tops while it remains on the battlefield. The opponent then casts Last Gasp to remove it; both players' views must stop revealing tops immediately. The trace continues to a clean pilot win and checks deterministic replay, event order, and gameplay mutations for Flying, reveal presence, reveal scope, and printed cost.

This is a single-card diagnostic pilot covering one of 291 RAV names and 306 printings. It grants zero card credit and zero reward. Harbor/provider evaluation has not been run. Materialize with `materialize_wizened_snitches_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
