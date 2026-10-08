Implement Golgari Rotwurm in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rotwurm.rs` by providing `definition()` and `activated_ability()`.

Golgari Rotwurm (#210) is a black-green 5/4 creature that costs `{3}{B}{G}`. Its activated ability costs `{B}`, sacrifices another creature, and targets a player to lose 1 life. The full-game reference casts Rotwurm and Elves of Deep Shadow with explicit colored payments, pays black for the ability, sacrifices the Elf, verifies the opponent loses one life, and attacks with Rotwurm to a terminal win.

Change only the candidate module. The reference checks live characteristics, ordered typed cast/payment/sacrifice/life-loss/combat events, deterministic replay, and three compiled semantic mutants. The report is diagnostic-only at 1/291, with zero card credit and zero reward.
