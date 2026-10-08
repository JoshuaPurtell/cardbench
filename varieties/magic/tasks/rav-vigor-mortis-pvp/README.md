# RAV Vigor Mortis single-card PvP task

Implement Vigor Mortis (#111) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vigor_mortis.rs`. It is a black sorcery costing `{2}{B}{B}`. Return target creature card from your graveyard to the battlefield. If green mana was spent to cast this spell, put a +1/+1 counter on that creature.

The trusted authority runs normal shuffled 60-card duels. It proves the creature first reached its owner's graveyard through legal combat, then checks a green-generic-payment cast and a black-generic-payment control. Both targets return through spell resolution; only the green-paid return receives a persistent +1/+1 counter. Each duel reaches a seat-zero win and replays deterministically. In-game mutations check the printed cost, return effect, and conditional color.

Materialize with `materialize_vigor_mortis_task.py --workspace PATH --manifest PATH`; verify with the same arguments and `--verify`. The scorer checks the trusted authority report outside the candidate process. This is local diagnostic evidence only: no provider trial, card credit, set completion, or Harbor reward is awarded.
