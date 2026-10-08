# Grayscaled Gharial full-game card task

Implement the candidate-owned RAV Grayscaled Gharial definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grayscaled_gharial.rs`. Grayscaled Gharial is collector #52, a blue 1/1 creature costing `{U}` with islandwalk.

The isolated authority runs a shuffled 60-card PvP game from ordinary opening hands. The pilot casts and attacks with Grayscaled Gharial while the opponent controls an Island and a live Drift of Phantasms blocker. A direct attempted block must be rejected by landwalk without changing the game state; the opponent then submits an empty legal block declaration and takes unblocked damage. The trace checks exact identity and payment, ordered receipts, deterministic replay, a terminal pilot win, and gameplay mutations.

The report covers one RAV identity among 291 names and 306 printings. It awards zero card credit and zero reward. Harbor/provider evaluation is outside the local reference gates. Materialize with `materialize_grayscaled_gharial_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
