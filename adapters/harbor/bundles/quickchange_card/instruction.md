Implement the candidate-owned Quickchange module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/quickchange.rs`.

Quickchange is a blue `{1}{U}` Instant. It lets its caster choose a color, makes the target creature only that color until end of turn, then draws a card.

Keep the module declarative and limited to its card definition. The isolated authority checks the policy-submitted color, exact Watchwolf object, color replacement and end-of-turn reversion, one drawn card, later combat, the terminal duel, deterministic replay, and semantic mutants. Do not add rules prose, art, scans, or flavor text.
