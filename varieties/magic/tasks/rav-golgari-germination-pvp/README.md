# Golgari Germination RAV full-game task

Implement Golgari Germination (#209) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_germination.rs`. It is a black-green enchantment costing `{1}{B}{G}`. Whenever a nontoken creature you control dies, its ability creates one green 1/1 Saproling creature token.

The shuffled 60-card duel casts Golgari Germination, attacks with a controlled nontoken creature into an available blocker, and follows its death trigger through the stack to the token entering the battlefield. The legal game continues to a seat-zero terminal win. Its trace checks the source identity, trigger provenance and ordering, the created token's live characteristics, deterministic replay, and gameplay mutations to the trigger, token, and printed cost.

This is an uncredited local diagnostic. It runs no provider trial and awards no card credit, set completion, or Harbor reward.
