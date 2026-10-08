# Primordial Sage RAV card task

Implement RAV #177, Primordial Sage: a green `{4}{G}{G}` 4/5 Creature — Spirit. A creature spell cast by its controller creates an optional trigger for that player; acceptance draws one card, while declining leaves the library unchanged.

The PvP authority casts Sage from a shuffled 60-card deck, then tests later creature spells with both accepted and declined optional draw triggers. It checks that the trigger belongs to the Sage's controller, resolves above the creature spell that caused it, draws the exact top card only when accepted, and leaves the library unchanged when declined. Both branches continue to a normal terminal match; deterministic replay and semantic mutations are checked. This diagnostic awards no card credit or reward.
