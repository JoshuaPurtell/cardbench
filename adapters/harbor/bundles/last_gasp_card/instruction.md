# Implement Last Gasp

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/last_gasp.rs` as a literal-only module with `definition()`. Preserve the RAV #93 identity, `{1}{B}` black instant characteristics, and a targeted -3/-3 modifier that lasts until end of turn.

The shuffled PvP reference casts Last Gasp after the opponent pays `{G}{W}` for Watchwolf. It must resolve its target-specific modifier and send the 3/3 to its owner's graveyard through the toughness-zero state-based action before continuing to a terminal pilot win. Preserve deterministic replay and make a source-level -2/-2 mutation fail this gameplay oracle.
