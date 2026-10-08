# Implement Darkblast

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/darkblast.rs` as a literal-only candidate module with `definition()`. Preserve the RAV #82 identity and printed behavior: black `{B}` instant, target creature gets -1/-1 until end of turn, and Dredge 3.

The legal PvP authority casts Darkblast at two opposing Watchwolves, observes each become exactly 2/2, has each block the pilot's Watchwolf and die to its combat damage, then replaces one draw with Dredge 3 and recasts the same Darkblast object at the second target. The game must finish normally with deterministic replay. The normalized source and AST are checked before candidate code can run.
