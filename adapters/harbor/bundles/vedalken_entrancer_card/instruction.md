Implement the candidate-owned Vedalken Entrancer module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_entrancer.rs`.

Vedalken Entrancer is RAV #74, a blue 1/4 Creature with Defender costing `{3}{U}`. Define one activated ability with the canonical ID `tap-blue-mill-two`: `{U}`, tap this creature: target player mills two cards. The ability is instant-speed.

The isolated authority checks the exact spell cost and characteristics; activation of the same Entrancer object with blue and tap costs on the opponent's turn; the opponent as target; and the identity, ownership, and graveyard movement of exactly the two top library objects. It then continues Watchwolf combat to a terminal win, replays deterministically, and checks five semantic mutations. Keep the module declarative and limited to the printed card definition and activation. Do not add rules prose, art, scans, or flavor text.
