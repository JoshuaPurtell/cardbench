# Sandsower full-game card task

Implement the candidate-owned RAV Sandsower definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sandsower.rs`. Sandsower is collector #28, a white 1/3 creature costing `{3}{W}`. Its ability has no mana cost and reads: “Tap three untapped creatures you control: Tap target creature.” Sandsower itself has no tap cost.

The isolated authority runs a shuffled 60-card PvP game from ordinary opening hands. It casts Sandsower and three other RAV creatures, then activates the ability at instant speed while the opponent’s Watchwolf is untapped. The trace checks the exact Sandsower payment and characteristics, three distinct controlled creatures tapped as costs, no tap on Sandsower, the target remaining untapped through activation and becoming tapped on resolution, ordered engine receipts, deterministic replay, and a full-game terminal pilot win.

The report covers one RAV identity among 291 names and 306 printings. It awards zero card credit and zero reward. Harbor/provider evaluation is outside the local reference gates. Materialize with `materialize_sandsower_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
