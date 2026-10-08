# CardBench Magic single-card task: Benevolent Ancestor

Implement Benevolent Ancestor in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/benevolent_ancestor.rs`. Keep its printed definition and activated prevention ability in that candidate-owned module. Only this module is an allowed candidate edit.

The card is a white `{2}{W}` 0/4 creature with Defender. Its `{T}` activated ability has no mana cost, targets a creature or player, and prevents the next 1 damage that would be dealt to that target this turn. The trace casts the card, activates its ability targeting seat zero during an opposing Elves of Deep Shadow attack, and verifies the exact damage is prevented and not dealt. Watchwolf combat carries the same legal shuffled game through a seat-zero win.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, the scorer, materializer, and task specification files. Do not add other files to the candidate submission.
