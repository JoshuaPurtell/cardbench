# Shadow of Doubt RAV card task

Implement the candidate-owned module `varieties/magic/sets/ravnica_city_of_guilds/src/cards/shadow_of_doubt.rs` as RAV #253: a blue-black instant with two `{U/B}` hybrid symbols. On resolution, prevent all players from searching libraries for the remainder of the current turn, then have its controller draw one card.

The diagnostic authority plays a normal shuffled 60-card duel. It responds to a resolving Farseek with Shadow of Doubt, checks the prevention and exactly one draw in printed order, verifies the same-turn search fails and a later-turn search succeeds, and continues the game through combat to a terminal seat-zero win. The later Farseek privately selects a Mountain from legal options. Deterministic replay and gameplay mutations cover the prevention, draw count, and effect order.

This single-card diagnostic reports one passed identity among the RAV catalog. It awards no card credit, pilot reward, or Harbor reward.
