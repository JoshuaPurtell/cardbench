# Followed Footsteps — RAV #51

This isolated PvP task implements the original Ravnica printing: a blue `{3}{U}{U}` Enchantment — Aura with “Enchant creature” and an ability that triggers at the beginning of its controller's upkeep to create a token copy of the enchanted creature. The trigger's controller and the token's controller are the Aura's controller. The enchanted creature can be controlled by another player without changing that upkeep scope.

The reference game has P0 cast Followed Footsteps and attach it to a Watchwolf cast by P1. It requires multiple triggers on P0's upkeeps, zero triggers on P1's upkeeps, tokens under P0's control, and each token's layer-one copy snapshot to name that exact Watchwolf incarnation. A Tattered Drake and the generated tokens carry the legal full-game duel to a P0 win. The task checks event ordering, payment, attachment incarnation, token values, replay, and mutations to cost, Aura binding, trigger, trigger scope, and token-copy effect.

Only the candidate card module is writable in the materialized workspace. The report grants zero reward and no card credit; RAV coverage remains incomplete. See [features.toml](features.toml), [source-note.md](source-note.md), and [harbor.toml](harbor.toml).
