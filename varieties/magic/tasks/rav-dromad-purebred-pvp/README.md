# Dromad Purebred single-card PvP pilot

This task qualifies Dromad Purebred (RAV collector #15) in a seeded legal 60-card game. Player zero pays `{4}{W}` and resolves the 1/5. Player one casts Char for `{2}{R}` targeting Dromad. The trace requires four damage to the permanent, Dromad's candidate-owned nonoptional trigger to reach and resolve from the stack, and exactly four life to be gained while Dromad remains on the battlefield. A Watchwolf cast for `{G}{W}` then attacks to a terminal player-zero win.

The authority replays the same match and compares complete event logs, terminal state, payments, live Dromad characteristics, and life totals. Its targeted semantic control replaces damage-sized life gain with a fixed one life; the compile-valid starter module uses that wrong effect and must fail the live event check. The task uses the Wizards Comprehensive Rules effective August 7, 2026 and the RAV printing source contract.

The report retains the full 291-identity / 306-printing RAV denominator. This is a single-card pilot with no whole-set credit or Harbor reward. Harbor qualification and independent review remain separate gates.

Materialize with `materialize_dromad_purebred_task.py --workspace PATH --manifest PATH`; verify with `--verify` added to the same command. The only candidate-writable file is `cards/dromad_purebred.rs`.
