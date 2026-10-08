# Flight of Fancy full-game card task

Implement the candidate-owned RAV Flight of Fancy definition and ETB trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/flight_of_fancy.rs`. It is a blue Aura that attaches to a creature, gives that creature flying, and draws two cards when it enters the battlefield.

The isolated authority must cast and attach Flight of Fancy to a controlled creature in a shuffled 60-card PvP game, resolve its draw-two trigger, prove the creature's live flying characteristic, and continue through combat to a terminal result. It must check exact Aura and creature identities, ordered payment/cast/attachment/trigger/draw events, deterministic replay, and mutations for missing flying, a missing trigger, and the wrong draw count. This diagnostic reports one identity among 291 RAV names and 306 printings with zero card credit and zero reward.
