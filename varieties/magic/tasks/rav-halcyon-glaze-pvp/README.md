# Halcyon Glaze RAV card task

Implement Ravnica: City of Guilds #54, Halcyon Glaze: a blue `{1}{U}{U}` Enchantment. Its creature-spell trigger makes the enchantment a 4/4 Illusion creature with flying until end of turn while retaining its Enchantment type.

The authority runs a normal shuffled 60-card duel. It verifies that a P0 creature spell cast stacks the Glaze trigger above that creature spell, the trigger resolves first, and the Glaze gains the exact temporary characteristics before combat. P0 attacks with the flying Glaze, then observes the animation expire during Cleanup. Later controlled creature spells retrigger it. A P1 creature spell must not trigger P0's Glaze. The full game ends in a P0 win with five verified four-damage Glaze hits.

The printed RAV wording says “Whenever you play a creature spell, Halcyon Glaze becomes a 4/4 Illusion creature with flying until end of turn.” Current Oracle changes “play” to “cast,” uses “this enchantment,” and states that the creature type is added to its other types. These are wording clarifications for the same cast trigger and type-preserving animation.

This is a local diagnostic task. All card credits and rewards remain zero.
