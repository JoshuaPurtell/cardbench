# RAV Flow of Ideas single-card PvP diagnostic

This task evaluates Flow of Ideas (RAV #50) through a complete, legal two-player
60-card game. The controller reaches six Islands, then casts the `{5}{U}` blue
sorcery while the opponent controls five Islands. The authority requires six
distinct cards to move into the controller's hand between the spell's cast and
resolution receipts, proving that the effect counts only Islands its
controller controls. The match then casts a green-white Watchwolf and reaches
a terminal win through combat.

The authority checks the live registered definition against the candidate's
source, pays exactly six blue mana, verifies the spell's stack and graveyard
event ordering, records the controlled/opponent Island snapshot at cast time,
and repeats the same seed to check determinism. Compiled semantic mutants alter
the exact mana cost, color, land type, and draw quantity; all must be rejected.
The report is diagnostic only: it records 1 of 291 unique RAV names against
306 printings, grants zero card credit, zero reward, and no set coverage.

Rules are pinned to the Comprehensive Rules effective August 7, 2026, at the
September 24, 2026 authority cutoff. The rules cross-check covers colored mana
and costs, priority and timing, basic land types, spell casting and resolution,
and drawing cards. No provider-backed trial is part of this package.

Materialize with `materialize_flow_of_ideas_task.py --workspace PATH --manifest
PATH`; verify with the same arguments and `--verify`. The scorer accepts a
materialized workspace and validates the authority report outside the
candidate process. The candidate-editable target is only
`src/cards/flow_of_ideas.rs`.
