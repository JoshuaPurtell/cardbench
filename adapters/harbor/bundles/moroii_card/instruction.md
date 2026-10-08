# Implement Moroii

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/moroii.rs` with literal candidate factories named `definition()` and `triggered_ability()`. Preserve RAV #216's blue-black `{2}{U}{B}` 4/4 creature with Flying, and the required `upkeep-lose-one-life` trigger at the beginning of its controller's upkeep.

The reference duel must show the exact four-mana cast receipt, an opponent upkeep without the trigger, and the Moroii controller's upkeep trigger stacked and resolved from the same source incarnation, causing exactly one life loss to that controller. Then attack through the opposing ground creature and finish a terminal pilot win. The trace must replay deterministically. Cost, stats, Flying, and trigger-removal mutations must fail; diagnostic output grants no card credit or reward.
