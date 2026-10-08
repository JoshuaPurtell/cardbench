# Flickerform full-game card task

Implement the candidate-owned RAV Flickerform module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/flickerform.rs`. Flickerform is a white `{1}{W}` Enchantment Aura that enchants a creature. Its `{2}{W}{W}` instant-speed activated ability exiles the enchanted creature and all Auras attached to it, then returns those cards to the battlefield under their owners' control at the next end step.

The diagnostic authority casts Flickerform and Moldervine Cloak on the exact Watchwolf, activates Flickerform, and follows the three exact object IDs through exile and back. It verifies the scheduled end-step return, newer object incarnations, Aura reattachment, the Cloak's +3/+3, later Watchwolf combat damage, and a terminal PvP win. The shuffled 60-card duel must replay deterministically. Gameplay mutations remove the linked-exile effect, change the activation cost, or remove the creature attachment behavior.

This is a single-card diagnostic pilot. It reports one identity among 291 RAV names and 306 printings and grants zero card credit or reward. Materialize with `materialize_flickerform_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
