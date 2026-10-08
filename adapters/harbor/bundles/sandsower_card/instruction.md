Implement the candidate-owned Sandsower definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sandsower.rs`.

Sandsower is RAV #28, a white 1/3 creature costing `{3}{W}`. Its activated ability has no mana cost and reads: “Tap three untapped creatures you control: Tap target creature.” Do not add a tap cost to Sandsower itself.

The isolated authority plays a shuffled full-game duel from ordinary 60-card decks. The pilot casts Sandsower and three other creatures, then activates it at instant speed targeting an untapped opposing Watchwolf. The authority checks that exactly three distinct creatures are tapped as costs, Sandsower remains untapped, and the target taps only when the ability resolves. It also checks ordered cast and ability receipts, replay, combat, a terminal win, and gameplay mutations.

Keep the candidate module declarative and limited to the card definition and activated ability. Do not add rules prose, art, scans, or flavor text.
