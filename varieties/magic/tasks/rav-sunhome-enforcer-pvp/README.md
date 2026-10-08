# RAV Sunhome Enforcer PvP task

Implement the candidate-owned definition and damage trigger for Sunhome Enforcer. The authority runs a legal 60-card PvP game, pays the actual red and white cast symbols plus two generic mana, resolves the 2/4, attacks with it, and checks ordered damage, trigger, life-gain, and ability-resolution events through a terminal game and deterministic replay.

The opponent begins at 20 life. The final lethal two-damage packet ends the game before its waiting trigger can resolve; the scorer therefore requires every nonlethal packet to produce exactly two life and checks the lethal terminal packet separately. A fixed one-life trigger is a targeted semantic mutant, and the compile-valid starter stub must fail the same live trace gate.

The public task workspace exposes only the candidate module. The scorer runs the bounded local Rust authority and Harbor bundle under the generic harness. It does not award set coverage or a Harbor reward by itself.
