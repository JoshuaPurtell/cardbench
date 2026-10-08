# Implement Rain of Embers

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/rain_of_embers.rs` as a literal-only module with `definition()`. Preserve Rain of Embers' RAV #138 identity, `{1}{R}` red sorcery cost, and one effect that deals one damage to each creature and each player.

The reference authority casts the card in a shuffled two-player game with an Elves of Deep Shadow and a Watchwolf on each side. It checks the exact red and generic payment, every creature and player damage packet, and the ordered state-based deaths after all packets. Both players finish the batch at 19 life; both Elves die; both Watchwolves remain on the battlefield with one marked damage. The diagnostic report credits no RAV coverage or Harbor reward.
