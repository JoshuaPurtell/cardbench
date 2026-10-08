# Veteran Armorer single-card PvP pilot

This task qualifies Veteran Armorer (RAV collector #34) in a legal seeded two-player constructed game. The active player casts a real Watchwolf for `{G}{W}` first and verifies its base 3/3 characteristics. The player then casts Veteran Armorer for `{1}{W}`. Its candidate-owned static binding must leave the Armorer itself at 2/2 while the other controlled Watchwolf becomes a live 3/4. The buffed creatures attack together, deal combat damage, and the match reaches a terminal result. The seeded match is replayed and its ordered receipts and terminal state must agree.

The rules snapshot is the Wizards Comprehensive Rules effective August 7, 2026, and the task follows the RAV printing and source contract at the September 24, 2026 authority cutoff. A targeted semantic mutation changes the static toughness bonus from +1 to +0; it must reach the live-state check and fail because Watchwolf remains 3/3. The compile-valid candidate stub has the same +0/+0 binding and must fail that gate.

The report preserves the full 291-card / 306-printing RAV denominator. This is a one-card pilot: it does not award card credit, whole-set coverage, or Harbor reward. Independent review and a real candidate trial remain separate qualification gates.

Materialize with `materialize_veteran_armorer_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`. The scorer accepts `--candidate-kind reference|agent` and validates candidates in the bounded offline worker.
