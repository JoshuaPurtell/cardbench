# Implement Boros Fury-Shield

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_fury_shield.rs` as a literal-only module with `definition()`. Preserve its `{2}{W}` white instant identity and its one targeted effect: it prevents all combat damage the targeted creature would deal through end of turn; if red mana was spent to cast it, it deals damage equal to that creature's power to that creature's controller.

The reference authority pays `{W}{R}{R}` and `{W}{G}{G}` in separate shuffled two-player full games. It checks cast/payment/resolution/graveyard ordering, the actual targeted attacker, the red damage and no-red branch, the prevention record, and an actual combat damage packet prevented for seat zero. The report is diagnostic only and credits no set coverage or reward.
