# Oathsworn Giant single-card PvP pilot

This task qualifies Oathsworn Giant (RAV collector #27) in a seeded, legal two-player constructed game. The active player casts Watchwolf for `{G}{W}` first and records its base 3/3 characteristics. After paying `{4}{W}{W}` for Oathsworn Giant, the same Watchwolf must become 3/5 and gain vigilance, while the Giant remains 3/4 with its printed vigilance. Both attack together, deal damage, and remain untapped after combat. The game reaches a terminal result and must replay deterministically.

The authority also mutates the Giant's toughness bonus, granted vigilance, and printed vigilance independently. Each mutation must be detected by live characteristics or the full-game combat trace. The candidate source is restricted to declarative definitions and the two candidate-owned static bindings.

The report keeps the full 291-card / 306-printing RAV denominator. This is a single-card pilot: it grants no card credit, whole-set coverage, or Harbor reward. Independent review and a candidate trial remain separate gates.

Materialize with `materialize_oathsworn_giant_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`. The scorer runs the candidate in the bounded offline worker.
