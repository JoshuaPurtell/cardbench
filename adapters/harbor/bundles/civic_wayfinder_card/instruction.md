# CardBench Magic single-card task: Civic Wayfinder

Implement Civic Wayfinder in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/civic_wayfinder.rs`. The module must provide `definition()` and `triggered_ability()` so the RAV registry can delegate to the candidate-owned card behavior.

Civic Wayfinder is a green 2/2 Creature with mana cost `{2}{G}`. Its mandatory enter-the-battlefield trigger searches its controller's library for a basic land card, may fail to find, reveals any selected land, puts it into that player's hand, then shuffles.

The trusted authority runs two shuffled 60-card full games. The first selects and reveals a Plains from among multiple basic land types; the second declines while legal candidates remain. Each trace checks ordered payment, cast, resolution, battlefield entry, private search decision, search result, shuffle, and combat-to-end events. Deterministic replay and compiled trigger, quality-restriction, and reveal mutants are also checked. The task reports one passed card of 291 unique names and 306 printings with zero card credit and zero reward.
