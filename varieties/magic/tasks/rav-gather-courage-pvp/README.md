# Gather Courage RAV card task

Implement the candidate-owned definition for Gather Courage (RAV #165), a `{G}` instant with Convoke that gives target creature +2/+2 until end of turn. The reference authority uses a shuffled 60-card two-player game. It casts Gather Courage after its Watchwolf attacks, taps Elves of Deep Shadow to pay the green symbol, and resolves the pump before the defending Watchwolf blocks.

The trace checks the ordered Convoke payment, cast, target-bound continuous effect, resolution, and graveyard movement. It asserts the target is 5/5 at resolution, then records the actual 5-to-3 blocked combat in which the attacker survives and its blocker dies. Diagnostic reports credit no RAV coverage or Harbor reward.
