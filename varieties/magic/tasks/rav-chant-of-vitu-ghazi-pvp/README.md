# RAV Chant of Vitu-Ghazi single-card PvP diagnostic

This task evaluates Chant of Vitu-Ghazi (RAV #7) in a complete legal shuffled
duel. The spell's exact `{6}{W}{W}` cost is paid with Convoke, then it prevents
damage from creature sources through the turn and grants life only for damage
actually prevented by that effect. The trace uses creature combat damage and a
creature-sourced damage event in a live game, then continues to a terminal
result.

The authority validates that the candidate module owns the card definition and
the prevention/life-gain effect. It checks typed ordered payment, Convoke,
casting, resolution, prevention, life gain, damage, and game-end events, repeats
the same shuffle seed, and detects compiled semantic mutants. The result is
diagnostic-only: one of 291 unique RAV names among 306 printings, with zero card
credit and zero reward.

The rules snapshot is effective August 7, 2026, at the September 24, 2026
authority cutoff. No provider-backed trial is part of this package.

Materialize with `materialize_chant_of_vitu_ghazi_task.py --workspace PATH
--manifest PATH`; verify with the same arguments and `--verify`. The only
candidate-editable file is `src/cards/chant_of_vitu_ghazi.rs`.
