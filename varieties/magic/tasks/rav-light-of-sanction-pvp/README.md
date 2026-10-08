# Light of Sanction single-card PvP task

Implement Light of Sanction (RAV #24) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs` as a white `{1}{W}{W}` enchantment. Its static effect prevents damage to creatures its controller controls when the damage source is also controlled by that player.

The trusted authority plays a shuffled 60-card two-player match. It casts Light of Sanction, a friendly Char at a controlled Watchwolf, and an opposing Char at that same Watchwolf. The friendly four damage is prevented, while Char's separate two damage to its controller still occurs; the opposing Char then deals four damage and kills the Watchwolf while Light remains on the battlefield. Skyknight Legionnaire finishes the game. The authority checks the ordered projected events and post-event states, deterministically replays the game, and detects removal, source-scope, and prevention-keyword mutations.

Materialize and verify a public workspace with `materialize_light_of_sanction_task.py --workspace PATH --manifest PATH` and the same command with `--verify`. The scorer validates the single-card authority report in an isolated worker. This is unreviewed diagnostic evidence: it awards no card credit or Harbor reward, and RAV coverage remains incomplete.

The candidate module must remain declarative: `definition()` and `static_continuous_effect()` return the card definition and its registered static binding.
