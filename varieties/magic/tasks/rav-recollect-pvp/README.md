# Recollect RAV full-game task

Implement Recollect (#178) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/recollect.rs`. Keep its candidate-owned definition in this module. It is a green `{2}{G}` sorcery that returns target card from your graveyard to your hand.

The deterministic shuffled 60-card PvP trace resolves Gather Courage, observes that instant enter the pilot's graveyard, then casts Recollect targeting that exact pilot-owned card. It verifies that the target is in the graveyard when targeted and moves to the pilot's hand during resolution, before Recollect itself reaches its owner's graveyard. Watchwolf then continues repeated combat to finish the full game.

The authority checks the declarative candidate-module boundary, registered-definition delegation, deterministic replay, a wrong-player graveyard target submission, a creature-only target-scope mutant, and wrong-cost and wrong-color mutants. This is a diagnostic task; it awards no card credit or Harbor reward.
