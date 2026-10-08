# Ordruun Commando RAV card task

Implement the candidate-owned Ordruun Commando definition and its `{W}` activated prevention ability for RAV #137. The reference authority shuffles two legal 60-card libraries, casts the red 4/1 Commando for `{3}{R}`, and later pays `{W}` during an opposing attack. The ability resolves before a live Elves of Deep Shadow attacks into the Commando; the Commando blocks it, the shield prevents the Elf's one combat damage, and the Commando deals lethal combat damage back.

The trace checks spell payment, cast and resolution, entry, the white activation payment, stack resolution, blocker assignment, the actual prevented-damage receipt, lethal damage and graveyard movement for the Elf, and a later Commando attack that ends the ordinary two-player game in seat zero's win. Mutations to shield amount, activation color, tap cost, and red spell cost must fail. The diagnostic reference awards no RAV credit or Harbor reward.
