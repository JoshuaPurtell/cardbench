# Cerulean Sphinx full-game card task

Implement RAV Cerulean Sphinx in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cerulean_sphinx.rs`. The task checks its exact `{4}{U}{U}` cost, 5/5 blue Creature characteristics, flying, and `{U}: shuffle Cerulean Sphinx into its owner's library` activation.

The private authority runs a legal shuffled 60-card duel. It casts Cerulean Sphinx, changes control with Flash Conscription, activates the Sphinx while the opponent controls it, and verifies that the exact object moves to and shuffles its owner's library. It then completes combat to a terminal PvP result, checks deterministic replay, and detects mutations to flying, the source-shuffle effect, and the activation cost.

This is a diagnostic card task within the 291-name / 306-printing RAV denominator. It grants no card credit or reward.
