Implement the candidate-owned Incite Hysteria definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/incite_hysteria.rs`.

Incite Hysteria is RAV #132, a red `{2}{R}` sorcery. Its target creature and each other creature sharing one of the target's colors cannot block until end of turn.

The isolated authority runs a shuffled full-game duel from ordinary 60-card decks. The reference checks the exact payment and the target plus a second red creature as Radiance recipients. A green creature remains able to block. The authority attempts a block with the targeted red creature, verifies the rejection leaves state unchanged, then observes a legal block by the green creature. The match must reach a terminal win with deterministic replay and gameplay mutations detected.

Keep the module declarative and limited to the card definition. Do not add card rules prose, art, scans, or flavor text.
