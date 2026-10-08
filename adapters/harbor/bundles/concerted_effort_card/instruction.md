# Implement Concerted Effort

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/concerted_effort.rs` with candidate-owned `definition()` and `upkeep_trigger()` literal functions. Keep its RAV #8 identity, white `{2}{W}{W}` enchantment cost, mandatory beginning-of-upkeep timing, and the six supported `SharedKeywordFamily` values: Flying, FirstStrike, DoubleStrike, Landwalk, Protection, and Trample.

The task authority verifies the full-game cast, other-creature keyword sharing, temporary gameplay effect, deterministic replay, and mutation sensitivity. Preserve exact typed Protection and Landwalk instances and the controller boundary covered by the existing focused regression. This diagnostic task awards no card credit or Harbor reward.
