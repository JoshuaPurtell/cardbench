# Fists of Ironwood full-game card task

Implement the candidate-owned RAV Fists of Ironwood definition and mandatory enter-the-battlefield trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/fists_of_ironwood.rs`. The green `{1}{G}` Enchantment Aura attaches to a Creature and grants that creature Trample. When Fists enters the battlefield, its mandatory trigger creates exactly two green 1/1 Saproling creatures.

The isolated diagnostic authority must resolve the Aura onto an exact Watchwolf object, resolve the trigger from the captured Fists incarnation, and verify both Saproling objects and their characteristics. In the same seeded full 60-card duel, the enchanted Watchwolf must attack and be blocked by an Elf; combat must deal lethal damage to that blocker and exactly two excess Trample damage to the defending player. The game continues to a terminal result and must replay deterministically. Mutations remove the ETB trigger, remove the Trample-granting attachment effect, and change the token count.

This is a single-card diagnostic pilot. It reports one identity among the 291 RAV names and 306 printings and awards zero card credit or reward. Materialize with `materialize_fists_of_ironwood_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
