# Thundersong Trumpeter single-card RAV PvP trace

This diagnostic task covers RAV Thundersong Trumpeter (#235): the red-white 2/1 creature and its zero-mana tap activation that temporarily prevents a target creature from participating in combat. A shuffled 60-card two-player match casts Trumpeter and a legal opposing creature, pays its colored cost, activates the ability through priority, then checks the targeted restriction during a later blocker declaration and records the terminal seat-zero win.

The rules baseline is the Comprehensive Rules effective 2026-08-07, with a 2026-09-23 source cutoff. The typed event projection, post-resolution characteristics, tap state, target legality, blocker state, terminal result, same-seed replay, and card-specific gameplay mutations are checked by the trusted authority. Results remain diagnostic against the 291-name, 306-printing RAV catalog; no card coverage or rewards are credited.

The candidate boundary is limited to `src/cards/thundersong_trumpeter.rs`. Materialization removes card task authorities and tooling from the public workspace and installs a compile-valid stub for the trusted bin. No provider-backed candidate trial is part of this task.
