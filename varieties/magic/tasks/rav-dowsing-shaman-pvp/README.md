# Dowsing Shaman full-game card task

Implement RAV Dowsing Shaman (#159) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/dowsing_shaman.rs`. It is a green `{4}{G}` 3/4 creature with the activated ability `{2}{G}, {T}: Return target enchantment card from your graveyard to your hand`.

The deterministic 60-card duel casts Dowsing Shaman and Fists of Ironwood, attaches the Aura to Elves of Deep Shadow, and attacks with that exact Elf into an available Watchwolf. The Elf dies and puts its attached Aura into its controller's graveyard. Dowsing Shaman then pays `{2}{G}` and taps to target that exact Aura card; the authority inspects the activated ability's stack target, confirms the Aura began in its controller's graveyard, and confirms it moved to hand on resolution. The game continues to a terminal seat-zero win.

The authority validates the declarative candidate module, exact RAV registry values, cast and activation costs, target scope and identity, zone transitions, full-game result, deterministic replay, and six semantic mutations. This is a local diagnostic pilot: it records one identity among 291 RAV names and 306 printings, with zero card credit and zero reward.

Materialize with `materialize_dowsing_shaman_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
