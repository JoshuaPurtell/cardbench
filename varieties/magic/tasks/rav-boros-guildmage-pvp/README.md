# Boros Guildmage single-card RAV PvP trace

This task covers Boros Guildmage (RAV collector #242): a red-white hybrid `{R/W}{R/W}` 2/2 creature with two candidate-owned activated abilities. Its `{R}` ability gives a target creature Haste until end of turn; its `{W}` ability gives a target creature First Strike until end of turn.

The reference authority uses a shuffled, legal two-player game. After Guildmage enters, it targets itself with both abilities, attacks that turn, and is blocked by an actual 1/1 Elf. The event trace verifies each colored payment, activation, resolution, and live target characteristics. First Strike damage kills the blocker before normal combat damage, and the blocker never deals damage back. Separate mutation controls remove each keyword effect and raise each colored activation cost.

The task boundary is one literal declarative card module containing the definition and both ability bindings. The authority separately changes each grant and increases each colored activation cost; all four mutations must be detected in the trace. The materializer uses the shared fail-closed RAV workspace helper; trusted authority bins, scorers, materializers, and task metadata are excluded from the candidate workspace. Results are diagnostic-only against the 291-name/306-printing RAV catalog and award no set coverage or Harbor reward.
