# RAV Blazing Archon single-card PvP diagnostic

This task evaluates Blazing Archon (RAV #4) in a complete legal shuffled duel.
The player pays its exact `{6}{W}{W}{W}` cost, resolves a white 5/6 creature
with Flying, and faces an opposing Elves of Deep Shadow that attacks before
the Archon enters. While the live Archon remains on the battlefield, the
opponent must make an empty attacker declaration. The Archon then attacks for
five combat damage over a terminal win.

The authority validates that the candidate module owns both the card
definition and its static attack-restriction binding. It checks typed ordered
payment, cast, resolution, entry, attack declarations, combat damage, and game
end events, and repeats the same shuffle seed. Compiled mutants remove Flying
or remove the opponent attack restriction. The result is diagnostic-only: one
of 291 unique RAV names among 306 printings, with zero card credit and zero
reward.

The rules snapshot is effective August 7, 2026, at the September 24, 2026
authority cutoff. No provider-backed trial is part of this package.

Materialize with `materialize_blazing_archon_task.py --workspace PATH --manifest
PATH`; verify with the same arguments and `--verify`. The only candidate-editable
file is `src/cards/blazing_archon.rs`.
