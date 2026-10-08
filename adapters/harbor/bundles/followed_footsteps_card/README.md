# Followed Footsteps card task

Implement the candidate-owned RAV #51 module in the only writable source file. Followed Footsteps costs `{3}{U}{U}`, is a blue Enchantment — Aura, and enchants a creature. Its original RAV text triggers at the beginning of the Aura controller's upkeep and creates a token copy of the enchanted creature. The Aura controller controls that trigger and token even when another player controls the enchanted creature.

The reference duel attaches the Aura to an opponent's Watchwolf, generates multiple copies on the Aura controller's upkeeps, and ends with a P0 win. The trusted scorer checks full-game event ordering, attachment/copy incarnations, token controller and characteristics, replay, semantic mutations, and source validation. The task issues no reward or card credit.
