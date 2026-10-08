# CardBench Magic single-card task: Bottled Cloister

Implement Bottled Cloister (RAV #256) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs`. Keep the module to the card definition and its candidate-owned triggered-ability bindings.

The verifier runs a shuffled 60-card two-player match. It requires the current hand to be exiled face down on an opponent's upkeep and linked to that exact Bottled Cloister incarnation. At the controller's upkeep, the same linked objects must return to their owner's hand before one fresh library card is drawn and before the ability resolves. The trace checks the opponent cannot see the hidden cards, reaches a terminal game result, replays deterministically, and rejects missing or reordered effects. This diagnostic task grants no card credit or Harbor reward.
