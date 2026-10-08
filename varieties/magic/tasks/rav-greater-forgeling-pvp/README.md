# Greater Forgeling RAV full-game task

Implement Greater Forgeling (#129) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/greater_forgeling.rs`. Keep its printed definition and activated ability in this candidate-owned module. It is a red `{3}{R}{R}` 3/4 creature. Its `{1}{R}` ability gives itself +3/-3 until end of turn.

The deterministic shuffled 60-card PvP trace casts Greater Forgeling, pays the activated cost from Mountains, resolves the ability, and attacks as a 6/1. The match continues through repeated same-turn pump and combat turns until a seat-zero win. The trace confirms the temporary modification expires back to 3/4, records exact spell and ability payments, and preserves legal combat/event order.

The authority checks candidate-source shape, candidate-owned RAV registry delegation, deterministic replay, and gameplay mutations to pump power, pump toughness, activation cost, base power, and casting cost. This diagnostic task awards no card credit or Harbor reward.
