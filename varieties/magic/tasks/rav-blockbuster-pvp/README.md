# Blockbuster RAV card task

Implement Blockbuster (RAV #115) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/blockbuster.rs` with `definition()` and `activated_ability()`. The card is a red `{3}{R}{R}` enchantment with `{1}{R}, sacrifice Blockbuster: it deals 3 damage to each tapped creature and each player.` The activation has no targets.

The full-game authority uses two legal shuffled 60-card decks. Its trace verifies exact casting and activation payments, source sacrifice before the activation enters the stack, a targetless damage batch for every tapped creature and both players, no damage to untapped creatures, state-based deaths for the tapped 3/3 attackers, deterministic replay, a clean terminal win, and cost/sacrifice/effect mutations. The diagnostic report covers 1 of 291 RAV card identities across 306 printings, credits zero cards, and awards zero reward.

The task preserves RAV printed semantics. Current Oracle wording says “this enchantment” in place of the printed self-reference; behavior is unchanged. See `source-note.md` for the source audit and the legacy catalog label issue.
