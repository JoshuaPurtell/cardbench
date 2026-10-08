# Siege Wurm full-game card task

Implement the candidate-owned Siege Wurm definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/siege_wurm.rs`. Siege Wurm is RAV collector #183, a green 5/5 Creature costing `{5}{G}{G}` with Convoke and Trample.

The reference authority plays a shuffled 60-card RAV duel from normal opening hands. It casts RAV creatures and Scatter the Seeds from the pilot deck, then casts Siege Wurm by tapping seven distinct creatures through Convoke: two contribute green mana and five contribute generic mana. No creatures are seeded directly onto the battlefield. The opponent casts a RAV Watchwolf and blocks the Wurm. The trace checks exact Convoke receipts, the live 5/5/Convoke/Trample characteristics, three lethal damage to Watchwolf, two trample damage to the opponent, Watchwolf's lethal-damage state-based action and graveyard move, replay, and a terminal pilot win.

This is a diagnostic pilot only. It reports one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. No provider trial is part of the reference checks. Materialize with `materialize_siege_wurm_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
