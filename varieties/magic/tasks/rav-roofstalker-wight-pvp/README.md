# Roofstalker Wight single-card PvP pilot

This task qualifies Roofstalker Wight (RAV collector #102) in a legal, seeded two-player constructed game. The trace pays `{1}{B}` for a real Roofstalker Wight and `{G}{W}` for a real opposing Watchwolf. It then pays `{1}{U}` for Wight's candidate-owned activated ability, resolves Flying on Wight, and verifies that a live ground Watchwolf cannot block it. The rejected block must leave the event log unchanged. Combat proceeds, the temporary Flying effect expires at cleanup, and a later activation demonstrates that Flying can be granted again before the game reaches a terminal result.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, using the RAV printing identity and the RAV source contract. The task module owns the definition and activated ability binding. A targeted mutation that grants Reach instead of Flying, and the compile-valid Reach stub, must both reach the block challenge and fail the same Flying gate. The deterministic replay checks ordered casts, mana and ability receipts, combat, cleanup expiry, reactivation, object state, and terminal outcome.

The report retains the full 291-card / 306-printing RAV denominator. This pilot pass does not award whole-set coverage, card credit, or Harbor reward. The reference lane is diagnostic; independent review and a real candidate trial remain separate gates.

Materialize with `materialize_roofstalker_wight_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`. The scorer accepts `--candidate-kind reference|agent` and validates a candidate in the bounded offline worker.
