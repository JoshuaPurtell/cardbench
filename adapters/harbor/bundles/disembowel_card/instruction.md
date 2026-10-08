# Implement Disembowel

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/disembowel.rs` as a literal-only candidate module exposing `definition()`. Preserve the RAV #85 identity, black instant color/type, `{X}{B}` cost, and the effect that destroys a targeted creature only when its mana value equals the chosen X.

The shuffled reference duel pays `{1}{B}` using black mana, targets the opponent's War-Torch Goblin (mana value 1), resolves the destruction, and continues to a terminal pilot win. It must replay deterministically. The authority separately rejects an X=2 cast against that same Goblin and detects mutations that remove destruction, change exact equality to at-most, or alter the printed cost. Reports give no card credit or reward.
