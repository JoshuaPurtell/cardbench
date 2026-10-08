# Boros Fury-Shield RAV card task

Implement the candidate-owned definition and conditional combat-prevention effect for Boros Fury-Shield. The full-game authority traces both legal generic-payment branches: `{W}{R}{R}` creates a prevention effect and deals 3 damage to the targeted Watchwolf's controller; `{W}{G}{G}` creates the same prevention effect without controller damage. Both traces use a shuffled legal 60-card two-player match and require a terminal result.

The trusted trace checks exact payment, target identity, the created prevention state, the damage or no-damage branch, a real combat-damage packet prevented for seat zero, and its unchanged life total from that packet. Diagnostic reports credit no RAV coverage or Harbor reward.
