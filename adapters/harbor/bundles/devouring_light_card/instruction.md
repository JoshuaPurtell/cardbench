Implement the candidate-owned RAV Devouring Light definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/devouring_light.rs`.

Keep the module declarative. It owns the white instant definition with printed cost `{1}{W}{W}`, Convoke, and an effect that exiles a creature that is attacking or blocking. Do not add rules prose, art, scans, or flavor text.

The isolated evaluator plays a seeded full game. The defending player must wait until an opposing Watchwolf is declared attacking, then cast Devouring Light while that creature is still attacking and before blockers. The caster taps two white creatures to pay the two white symbols through Convoke and spends one mana for the generic symbol. The spell resolves and exiles that exact attacker; the match continues to a terminal result and replays deterministically. The evaluator also checks mutations that remove Convoke or allow targeting any creature.
