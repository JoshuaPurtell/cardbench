Implement the candidate-owned Seeds of Strength module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seeds_of_strength.rs`.

Seeds of Strength is a green-white `{G}{W}` Instant. It gives up to three target creatures +1/+1 until end of turn.

Keep the module declarative and limited to the card definition. The isolated authority checks exact payment and object identities, three independent temporary modifiers, before/during/after power and toughness, cleanup expiration, later combat damage from all three targets, a terminal full game, deterministic replay, and semantic mutations. Do not add rules prose, art, scans, or flavor text.
