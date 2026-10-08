# Implement Fiery Conclusion

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/fiery_conclusion.rs` as a literal-only module containing `definition()` and `additional_spell_cost_binding()`. Preserve Fiery Conclusion's RAV #122 identity, `{1}{R}` red instant characteristics, and the required sacrifice-controlled-creature cost binding.

The definition deals 5 damage to a target creature. The reference casts it during the opponent's Watchwolf attack, pays one red plus one generic mana, sacrifices its own Elves of Deep Shadow, and removes the attacker before blockers and combat damage. The report credits no RAV coverage or Harbor reward.
