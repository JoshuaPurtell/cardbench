# RAV Belltower Sphinx single-card PvP task

This task qualifies Belltower Sphinx (RAV #38) in a shuffled two-player 60-card game. Seat zero pays `{4}{U}` to cast the blue 2/5 creature and uses its Flying to continue the game toward a terminal win. Seat one casts Viashino Fangtail and activates its tap ability to deal exactly one damage to the Sphinx. The mandatory, non-targeted damage trigger then mills exactly one card from the controller of that damage source (seat one), not from the Sphinx controller's library.

The authority checks the cast and battlefield entry, live 2/5 Flying characteristics, source and target identities for damage, trigger stacking and resolution, the single owner-preserving library-to-graveyard move, deterministic replay, and the final PvP winner. Reference mutations remove Flying, remove the triggered ability, or duplicate its mill instruction so the full-game oracle sees an incorrect mill count. The report credits no set coverage or Harbor reward.

Run `materialize_belltower_sphinx_task.py --workspace PATH --manifest PATH`, then verify with `--verify`. The authority bin is `rav-belltower-sphinx-pvp`; the scorer takes `--workspace`, `--manifest`, `--output-root`, and `--candidate-kind reference|agent`. Harbor/provider evaluation has not been run.
