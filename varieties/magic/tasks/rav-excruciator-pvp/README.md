# Excruciator full-game card task

Implement the candidate-owned RAV Excruciator definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/excruciator.rs`. Excruciator is collector #121, a red 7/7 creature costing `{6}{R}{R}` with damage that can't be prevented.

The isolated authority runs a shuffled 60-card PvP game from ordinary opening hands. The pilot casts Excruciator and attacks into an Ordruun Commando that has resolved its one-point damage shield. The trace verifies the exact card identity, cost and characteristics, a blocked seven-damage combat packet, the Commando moving to its graveyard, no prevention receipt for Excruciator's damage, deterministic replay, and a terminal pilot win.

The report covers one RAV identity among 291 names and 306 printings. It awards zero card credit and zero reward. Harbor/provider evaluation is outside the local reference gates. Materialize with `materialize_excruciator_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
