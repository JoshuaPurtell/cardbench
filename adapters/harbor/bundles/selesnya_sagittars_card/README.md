# Selesnya Sagittars task bundle

This bundle evaluates RAV Selesnya Sagittars (#229): a green-white 2/5 creature with Reach and `{T}: deal 1 damage to target attacking or blocking creature`. Harbor reads `varieties/magic/tasks/rav-selesnya-sagittars-pvp/harbor.toml` for the isolated task route.

The trusted full-game trace pays the printed spell cost, blocks Courier Hawk through Reach, pings a simultaneously attacking Elf before combat damage, validates the combat exchange, then finishes the game through Watchwolf. The report stays diagnostic-only at 1/291 and zero reward.
