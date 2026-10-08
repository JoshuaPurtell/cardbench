# Sparkmage Apprentice RAV card task

Implement candidate-owned Sparkmage Apprentice for RAV #144: a `{1}{R}` red 1/1 creature and a mandatory enters-the-battlefield trigger that deals 1 damage to a target player or creature. Keep both the card definition and `etb-deal-one` trigger binding in the candidate module.

The authority uses an ordinary shuffled two-player game. Seat one casts Elves of Deep Shadow as its only creature. Seat zero casts Sparkmage Apprentice, selects that Elf as the trigger target, and checks the trigger stack object, one damage, the lethal-damage state-based action, and the Elf's graveyard move. Seat zero then attacks with the cleared board lane; the opponent has no blocker and seat zero wins the full game. The report remains diagnostic and grants no RAV coverage or Harbor reward.
