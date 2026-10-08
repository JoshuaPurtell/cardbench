# Devouring Light single-card PvP pilot

This task qualifies Devouring Light (RAV collector #13) in a seeded, legal two-player constructed game. The opponent casts and attacks with a real Watchwolf. Only after the `AttackersDeclared` receipt, the defending player casts Devouring Light at instant speed, pays its `{1}{W}{W}` with two white creatures through Convoke and one generic mana, and exiles the still-attacking target before blockers. The full game then continues to a terminal result and must replay deterministically.

The authority checks the attacking target at the exact cast decision, its exile zone change during spell resolution (before the `SpellResolved` receipt), two distinct creatures tapped for white Convoke, and the remaining mana payment. It also checks that removing Convoke prevents the cast and broadening the target to any creature improperly permits a precombat target.

The report keeps the full 291-card / 306-printing RAV denominator. This is a single-card pilot: it grants no card credit, whole-set coverage, or Harbor reward. Independent review and candidate trial remain separate gates.

Materialize with `materialize_devouring_light_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`. The scorer runs the candidate in the bounded offline worker.
