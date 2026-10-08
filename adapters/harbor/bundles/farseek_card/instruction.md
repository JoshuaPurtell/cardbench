# CardBench Magic single-card task: Farseek

Implement the RAV Farseek module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/farseek.rs`.

Farseek is a green Sorcery with exact cost `{1}{G}`. Its controller may find a Plains, Island, Swamp, or Mountain card in their library, put that card onto the battlefield tapped, and shuffle. Forest is not an eligible search result. The selected library identity is private until the card enters the public battlefield; Farseek does not reveal it as part of the search.

The trusted authority plays a shuffled legal 60-card duel, selects a Plains, verifies private candidate visibility and tapped entry, then continues through combat to a terminal seat-zero win. Follow the task's CardBench-authored semantic summary and preserve the candidate module boundary. This diagnostic awards no RAV set credit or Harbor reward.
