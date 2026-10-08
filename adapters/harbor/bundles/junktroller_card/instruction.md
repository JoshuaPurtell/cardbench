# Implement Junktroller

Edit only `varieties/magic/sets/ravnica_city_of_guilds/src/cards/junktroller.rs`.

Implement Junktroller's printed RAV identity and its `{T}` ability: choose a target card from any public graveyard and put that exact card on the bottom of its owner's library. Do not add a mana cost or shuffle; neither appears in this card instruction.

The authority runs ordinary play from shuffled 60-card decks. It checks the artifact creature's cost, characteristics and Defender keyword; waits through summoning sickness; taps Junktroller without paying mana; targets an opponent-owned graveyard card; checks the owner's library bottom and preserved order; continues to a terminal game result; and detects semantic changes. The source module must remain declarative and candidate-owned.
