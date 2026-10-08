# Gaze of the Gorgon RAV card task

Implement the candidate-owned definition module for Gaze of the Gorgon, RAV
#246. It is a `{3}{B/G}` black-green instant. Its targeted creature receives a
regeneration shield, then the spell schedules destruction at this turn's next
end of combat for every creature that blocked or was blocked by that target
this turn.

The reference authority runs a shuffled, legal 60-card two-player game. It
casts Gaze after the target attacker has been blocked, records the actual
hybrid-symbol payment, observes the regeneration shield save the target during
combat damage, and observes the blocker destroyed by the delayed action at end
of combat. The same game continues to a terminal seat-zero win. Deterministic
replay and a mechanic-removal mutation are required. Reports grant no set
coverage credit or reward.

The public candidate workspace contains only the Gaze card module as its
editable file. The trusted reference authority, task tooling, and Harbor
scoring remain outside that workspace.
