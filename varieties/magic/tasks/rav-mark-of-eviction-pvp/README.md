# Mark of Eviction RAV card task

Implement the literal RAV #58 definition and upkeep ability in the candidate-owned module. The task checks a blue `{U}` Aura attached to a creature, with an upkeep trigger that returns the enchanted creature and every Aura attached to it to their respective owners' hands at the beginning of Mark's controller's upkeep.

The shuffled reference casts Mark on the opponent's War-Torch Goblin, lets the opponent's upkeep pass without triggering it, then observes the Mark trigger on its controller's later upkeep. Both the Goblin and Mark return to their own owners' hands, with a separate owner-view receipt for each. The duel continues to a terminal seat-zero win and must replay identically. Gameplay mutations remove the attachment or trigger, change the upkeep condition or return-group instruction, and change the cost. Diagnostic reports award no card credit or Harbor reward.
