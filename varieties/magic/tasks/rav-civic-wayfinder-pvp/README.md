# Civic Wayfinder full-game card task

Implement Civic Wayfinder in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/civic_wayfinder.rs` by returning its declarative definition and candidate-owned ETB trigger binding.

The diagnostic authority runs two shuffled 60-card duels. In one, Civic Wayfinder is cast for `{2}{G}`, its mandatory trigger opens a private basic-land search, and the controller selects a Plains, reveals it, puts it into hand, and shuffles. In the second, legal basic lands are available but the controller chooses to find nothing; no land is revealed or moved, and the library still shuffles. Both games continue through legal combat to a P0 win.

The authority checks ordered mana payment, spell cast and resolution, battlefield entry, trigger and private decision events, reveal and search receipts, combat, and game end. It also checks live 2/2 green characteristics, deterministic replay, and compiled controls for a missing trigger, a search that omits Plains, and a missing reveal. The diagnostic reports one passed card of 291 unique names and 306 printings, with zero card credit and zero reward.
