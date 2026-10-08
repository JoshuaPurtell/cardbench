Implement the candidate-owned RAV Oathsworn Giant module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/oathsworn_giant.rs`.

Keep the module declarative. It owns the white 3/4 creature definition with printed `{4}{W}{W}` and vigilance, plus two static bindings that grant +0/+2 and vigilance to other creatures you control. Oathsworn Giant must not receive its own static effects. Do not add rules prose, art, scans, or flavor text.

The isolated evaluator plays a seeded full game. It casts a real Watchwolf for green and white first and records its 3/3 baseline. After the Giant resolves, the same Watchwolf must be 3/5 with vigilance; the Giant stays 3/4 with vigilance. Both attack, deal combat damage, and remain untapped. The trace must reach a terminal result, replay deterministically, and detect separate wrong-toughness, missing-granted-vigilance, and missing-printed-vigilance mutations.
