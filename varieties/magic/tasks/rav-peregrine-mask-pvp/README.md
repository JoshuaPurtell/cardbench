# Peregrine Mask full-game card task

Implement the candidate-owned RAV Peregrine Mask definition, equip ability, and Equipment attachment binding in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/peregrine_mask.rs`.

Peregrine Mask is a colorless `{1}` Artifact Equipment. Its equip ability costs `{2}`, may be activated at sorcery speed, and targets a creature its controller controls. While attached, that creature has Defender, Flying, and First Strike.

The trusted authority plays shuffled, constructed 60-card decks. It casts the Mask and a Watchwolf, pays the equip cost, probes an illegal attack with the equipped creature, then has the equipped Watchwolf block a Skyknight Legionnaire. It checks the exact attachment, all three granted keywords, the rejected Defender attack, the flying block, three First Strike damage, the Skyknight's move to the graveyard before it can damage Watchwolf, replay, and a terminal PvP result. In-game mutations remove each granted keyword or change the equip cost.

This is an uncredited single-card diagnostic pilot. Every report and score keeps `pilot_reward`, `harbor_reward`, and `cards_credited` at zero. Materialize a public workspace with `materialize_peregrine_mask_task.py --workspace PATH --manifest PATH`; verify it with the same command plus `--verify`.
