# Dizzy Spell full-game card task

Implement the candidate-owned Dizzy Spell module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/dizzy_spell.rs`. Dizzy Spell is RAV collector #43: a blue Instant costing `{U}` that gives target creature -3/-0 until end of turn and has Transmute `{1}{U}{U}`.

The local authority uses shuffled 60-card PvP decks. The pilot pays the exact Transmute cost, makes the controller-private search, selects a physical Dizzy Spell card, and casts that same object for `{U}` at priority after an opposing Watchwolf has been declared attacking. It checks the exact target and its controller/owner, 3/3 to 0/3 characteristics in the power/toughness layer, matching permanent incarnation, resolution and graveyard movement, and zero damage from the target in that combat. The duel continues to a terminal pilot win and deterministic replay. Five semantic mutants cover the effect's power and toughness, Transmute, its cost, and the printed Instant cost.

This is diagnostic evidence for one identity among 291 RAV card names and 306 printings. Card credit and reward remain zero. No Harbor/provider candidate trial is part of local validation. Materialize with `materialize_dizzy_spell_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
