# Implement Viashino Fangtail

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/viashino_fangtail.rs` as a literal-only module that exports `definition()` and `tap_deal_one_ability()`. Preserve the RAV #148 identity, `{2}{R}{R}` cost, red 3/3 creature characteristics, and stack-backed tap ability with a player-or-creature target and one damage. Keep both the card definition and the activated-ability binding in this candidate-owned module.

The reference authority plays an ordinary shuffled two-player game. It casts Fangtail, waits until the next turn so the creature can legally pay its tap cost, then targets the opponent’s Elves of Deep Shadow. It verifies the source taps, damage reaches that creature, lethal damage causes a state-based action and graveyard move, and Fangtail remains a 3/3. Fangtail later attacks in the same match and the game reaches a seat-zero win. Reference reports grant no RAV credit or Harbor reward.
