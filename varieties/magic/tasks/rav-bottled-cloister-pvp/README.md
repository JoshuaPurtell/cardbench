# RAV Bottled Cloister single-card PvP task

Implement Bottled Cloister (RAV #256) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs` as a colorless `{4}` artifact with two linked-upkeep abilities. On an opponent's upkeep, exile the controller's current hand face down and link that group to this source incarnation. On the controller's upkeep, return exactly that linked group to its owner's hand, then draw a card.

The trusted authority runs a shuffled 60-card duel. It checks the four-mana cast, the opponent-upkeep trigger and complete hand exile, the opponent's inability to see the hidden hand, the exact source object and incarnation on both linked-hand receipts, return of the same card objects, and a new library card drawn only after the group returns and before the trigger resolves. A Watchwolf is then cast and attacks to reach a terminal PvP result. The trace must replay deterministically and detect trigger removal, incorrect return/draw order, omitted draw, and the wrong upkeep condition.

Materialize with `materialize_bottled_cloister_task.py --workspace PATH --manifest PATH`; verify the files with the same arguments and `--verify`. The scorer validates the trusted report outside the candidate process. This is diagnostic reference evidence only and awards no card credit or Harbor reward.
