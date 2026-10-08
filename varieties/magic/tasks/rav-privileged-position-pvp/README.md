# Privileged Position (RAV #251)

Implement the candidate-owned definition and static-effect binding in `cards/privileged_position.rs` for Privileged Position's single RAV printing. Preserve its hybrid `{2}{G/W}{G/W}{G/W}` Enchantment identity and its effect on other permanents you control.

The source-faithful game demonstrates both sides of the opponent-only targeting restriction. The opponent attempts to cast Putrefy at the pilot's protected Watchwolf; the game must reject that target before payment or stack/state mutation. The pilot can still cast Gather Courage targeting that same protected Watchwolf. The shuffled duel then reaches a normal seat-zero win with deterministic replay.

The trusted full-game authority is `rav-privileged-position-pvp`. It validates normalized candidate source and literal AST before candidate calls, rejects a compile-valid injected side-effect function, and detects removal or broadening of the targeting restriction, changes to the hybrid cost, and incorrect card characteristics. The materializer exposes only the Privileged Position module as writable and stubs trusted authority binaries. No card credit or reward is granted.

The original RAV text is opponent-only. The opponent's Putrefy attempt at the pilot's Watchwolf is rejected before payment or game-state mutation; the pilot can still cast Gather Courage at that same Watchwolf. This distinguishes the printed rule from Shroud. The duel then reaches a normal seat-zero win with deterministic replay.

See [source-note.md](source-note.md) for the RAV printing authority and Oracle reconciliation. The task follows `varieties/magic/tasks/rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md`: the RAV printing controls identity and semantics, and Oracle is used only to interpret terminology.
