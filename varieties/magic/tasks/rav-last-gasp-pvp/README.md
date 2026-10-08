# Last Gasp RAV card task

Implement the candidate-owned module for Last Gasp, RAV #93. It is a `{1}{B}` black instant that gives target creature -3/-3 until end of turn.

The authority runs a shuffled, legal 60-card PvP game. The opponent casts a real `{G}{W}` Watchwolf; the pilot casts Last Gasp at instant speed, pays `{1}{B}`, and resolves the targeted modifier. The trace must show the -3/-3 continuous effect, the toughness-zero state-based action, the Watchwolf moving to its owner's graveyard, and a subsequent terminal win by the pilot's own Watchwolf. Replay is deterministic and the targeted -2/-2 gameplay mutation must fail because it leaves the opposing 3/3 alive. Reports award no set coverage or Harbor reward.
