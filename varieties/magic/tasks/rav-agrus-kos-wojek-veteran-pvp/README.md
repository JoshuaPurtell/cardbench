# Agrus Kos, Wojek Veteran RAV card task

Implement Ravnica: City of Guilds #190, Agrus Kos, Wojek Veteran. Its printed identity is a legendary red-white `{3}{R}{W}` 3/3 Human Soldier. Whenever Agrus attacks, attacking red creatures get +2/+0 and attacking white creatures get +0/+2 until end of turn.

The isolated shuffled PvP authority checks that the attack trigger stacks and resolves, independently applies both effects to red-white attackers (including Agrus), gives red-only and white-only attackers only their matching modifier, and leaves a nonattacking red creature unchanged. Modifiers expire at Cleanup; the duel must reach a terminal seat-zero win with a deterministic replay and mutation checks.

The original Ravnica FAQ states that a red-white attacker gets both bonuses. This task awards no card credit or reward; independent qualification remains pending.
