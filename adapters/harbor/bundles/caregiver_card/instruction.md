Implement Caregiver in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/caregiver.rs` by providing `definition()` and `activated_ability()`.

Caregiver (#6) is a white 1/1 creature that costs `{W}`. Its ability costs `{W}`, sacrifices Caregiver, and prevents the next 1 damage that would be dealt to target creature or player this turn. The diagnostic casts Caregiver, uses its ability to protect its controller from Elves of Deep Shadow's attack, then casts and attacks with Watchwolf to win.

Change only the candidate module. The reference checks live characteristics, exact white payment for both costs, source sacrifice and graveyard movement, shield creation and consumption, ordered typed combat events, deterministic replay, and four compiled semantic mutants. The task remains diagnostic-only at 1/291, with zero card credit and zero reward.
