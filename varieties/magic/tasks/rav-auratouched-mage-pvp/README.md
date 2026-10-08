# Auratouched Mage full-game card task

Implement the candidate-owned RAV Auratouched Mage definition and ETB trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/auratouched_mage.rs`. Its triggered ability searches its controller's library for an Aura card that can legally attach to the Mage, permits a private choice to find nothing, and attaches any selected Aura to the Mage.

The isolated diagnostic authority must exercise a selected compatible Aura and a declined search in separate shuffled 60-card PvP games. Each game continues after the ETB decision to a terminal result and checks ordered cast, trigger, private search, attachment, combat, and game-end events. It must also check deterministic replay and mutations for a missing trigger, an incompatible Aura entering the choice set, and a missing attachment. This task is diagnostic only: it reports one identity among 291 RAV names and 306 printings and grants zero card credit or reward.
