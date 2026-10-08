# Incite Hysteria full-game card task

Implement the candidate-owned Incite Hysteria definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/incite_hysteria.rs`. RAV #132 is a red `{2}{R}` sorcery. Its target creature and every other creature sharing one of the target's colors cannot block until end of turn.

The local reference trace casts the card from a shuffled constructed 60-card deck, pays the printed cost, and resolves it against an opposing red creature while another red creature shares its color. A green creature remains unaffected and able to block. The authority submits an otherwise legal block with the targeted red creature, requires rejection with unchanged state, then observes a legal block by the green creature. The match reaches a terminal P0 win, replays deterministically, and detects Radiance-scope, keyword, and cost mutations.

This single-card diagnostic reports one passing card among 291 RAV names and 306 printings. It grants no card credit or Harbor reward. No provider trial is part of local qualification. Materialize with `materialize_incite_hysteria_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
