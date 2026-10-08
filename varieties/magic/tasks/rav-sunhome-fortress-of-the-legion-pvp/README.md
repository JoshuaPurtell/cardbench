# RAV Sunhome, Fortress of the Legion single-card PvP task

This task qualifies Sunhome, Fortress of the Legion (RAV #282) through a full
two-player constructed match. The pilot plays Sunhome and taps it for one
colorless mana to cast a real Skyknight Legionnaire, paying `{1}{R}{W}` with
that colorless mana. On a later turn, Sunhome pays `{2}{R}{W}` and taps to
target that creature. The trace verifies the target gains Double Strike and
deals damage in both combat-damage steps before the match reaches a terminal
result.

The reference run replays the complete seeded event stream and checks semantic
mutations for Sunhome's colorless output, activation cost, tap cost, and granted
keyword. The task source is the candidate-owned
`cards/sunhome_fortress_of_the_legion.rs` module. Its authority permits only
the three literal-returning constructors and validates their exact identity
and binding shapes before the game uses them.

The rules snapshot is the Wizards Comprehensive Rules effective August 7,
2026, at the September 24, 2026 authority cutoff. The report preserves all 291
RAV names and 306 printings. This single-card reference pilot awards no card
credit, whole-set coverage, or Harbor reward; independent review and a
candidate trial remain separate qualification steps.

Run `materialize_sunhome_fortress_of_the_legion_task.py --workspace PATH
--manifest PATH`; verify with the same arguments plus `--verify`. The scorer
accepts `--candidate-kind reference|agent` and validates the trusted report
outside the candidate process.
