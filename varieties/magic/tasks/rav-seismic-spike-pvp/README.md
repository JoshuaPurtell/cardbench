# RAV Seismic Spike single-card task

Implement the candidate-owned Seismic Spike module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs`. Seismic Spike is RAV collector #141, a red sorcery costing `{2}{R}{R}`. Its ordered instructions destroy a target land, then add two red mana to its controller's mana pool.

The authority runs a legal shuffled sixty-card match. The opponent plays Mountains; the pilot pays four red mana to cast Seismic Spike at an opposing Mountain. The full-game trace records the exact destruction and graveyard movement, the two-red-mana addition before spell resolution ends, and a subsequent Viashino Slasher paid with two red mana. The pilot continues to a seat-zero combat win.

The authority validates RAV catalog identity, declarative source syntax, cost and effect order, target provenance, mana addition and follow-up payment, deterministic replay, and a semantic effect-order mutation. Reference reports award no set coverage or Harbor reward. Independent review and a real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH` and verify with the same arguments plus `--verify`. The Harbor authority bin is `rav-seismic-spike-pvp`; the local Cargo target is `rav_seismic_spike_pvp`.
