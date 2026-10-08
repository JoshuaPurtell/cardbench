# RAV Ghosts of the Innocent single-card PvP task

Implement Ghosts of the Innocent (#20) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ghosts_of_the_innocent.rs`. It is a white creature costing `{5}{W}{W}` with base power and toughness 4/5. If a source would deal damage to a permanent or player, it deals half that damage, rounded down, to that permanent or player instead.

The trusted authority runs an ordinary shuffled 60-card duel. After Ghosts resolves, an opposing Char deals its four-damage packet to a controlled Watchwolf and its two-damage self-packet; both are halved. A later Char targets the player, proving the replacement also applies to player damage. Combat damage from a controlled Watchwolf is likewise halved. The game reaches a seat-zero terminal win and replays deterministically. Cost, characteristics, missing-binding, and wrong-replacement mutations are checked in-game.

Materialize with `materialize_ghosts_of_the_innocent_task.py --workspace PATH --manifest PATH`; verify with the same arguments and `--verify`. The scorer checks the trusted authority report outside the candidate process. This is local diagnostic evidence only: no provider trial, card credit, set completion, or Harbor reward is awarded.
