# Greater Mossdog single-card task

This diagnostic task covers Ravnica: City of Guilds #169 through one shuffled 60-card PvP game. The candidate-owned module contains the printed creature definition and its Dredge keyword. The trace casts Greater Mossdog, blocks a Watchwolf and dies, chooses its own object as the draw replacement, mills exactly three cards, returns that same object to hand, casts it again, and wins through later combat damage.

The card has no creature-becomes-target trigger. The task authority checks that the shared trigger registry contains no Greater Mossdog binding and includes a mutation probe showing that restoring the unprinted trigger changes the target-trigger event stream. The reference scorer, authority, materializer, and task metadata remain trusted. Only `varieties/magic/sets/ravnica_city_of_guilds/src/cards/greater_mossdog.rs` is candidate-editable.

All reported values are diagnostic only: card credit, coverage completion, full reward, Harbor reward, and pilot reward remain zero.
