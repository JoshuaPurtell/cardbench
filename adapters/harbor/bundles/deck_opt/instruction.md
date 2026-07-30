# CardBench Pokémon deck-choice task

Create `candidate/deck.json` by selecting one unchanged deck from
`varieties/pokemon/decks/`. The deck must contain exactly 60 cards, respect the
four-copy limit for non-basic-Energy cards, and contain Energy. Decks are
scored with the frozen `simple_heuristic_v1` code policy; do not create or
modify a play policy.
