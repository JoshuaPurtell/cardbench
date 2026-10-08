# Vedalken Dismisser full-game card task

Implement the candidate-owned Vedalken Dismisser module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_dismisser.rs`. Vedalken Dismisser is RAV collector #73: a blue 2/2 Creature costing `{5}{U}`. Its mandatory enters-the-battlefield trigger targets a creature and puts that exact creature on top of its owner's library.

The authority runs a shuffled 60-card PvP duel. The opponent establishes an early creature, then the pilot casts Dismisser and chooses that exact opposing permanent while its mandatory trigger waits for a target. The trace checks the controller-only target-decision boundary, trigger/source identity, the target's owner, the exact target-object move to the owner's library, the library top identity and size change, and the matching source incarnation at resolution. A Watchwolf continues through combat to a terminal pilot win. The duel replays deterministically and five semantic mutations check trigger presence and optionality, destination, body, and cost.

This is a diagnostic pilot only. The report covers one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. Do not run a provider trial for reference validation. Materialize with `materialize_vedalken_dismisser_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
