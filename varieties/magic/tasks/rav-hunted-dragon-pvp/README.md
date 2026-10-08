# CardBench Magic single-card task: Hunted Dragon

Implement the candidate-owned Hunted Dragon definition and enters-the-battlefield ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_dragon.rs`.

The trusted diagnostic authority must parse and validate the exact candidate-source contract before invoking candidate functions, and must reject a compile-valid source mutation that attempts a filesystem side effect. It then runs a deterministic, legal 60-card duel. It should cast Hunted Dragon for exactly `{3}{R}{R}`, verify its 6/6 Flying and Haste characteristics, and resolve the mandatory ETB trigger with the opponent as its only legal target. The targeted opponent must receive exactly three white 2/2 Knight creature tokens with First Strike. Capture the tokens when the trigger resolves, while both players remain in the game, and verify the dragon's Haste through legal same-turn combat. Replay the complete game and detect semantic mutations to the body, mana cost, Flying, Haste, trigger condition, target restriction, token characteristics, and token count.

This is an uncredited diagnostic task against the 291 distinct RAV names and 306 printings. It reports zero card credit and zero reward. Candidate evaluation and credit are separate.
