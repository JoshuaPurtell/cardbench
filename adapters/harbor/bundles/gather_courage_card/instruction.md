# Implement Gather Courage

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/gather_courage.rs` as a literal-only module with `definition()`. Preserve its RAV #165 identity, `{G}` green instant cost, Convoke keyword, and targeted +2/+2-until-end-of-turn effect.

The reference authority traces a shuffled two-player game in which an Elves of Deep Shadow pays Gather Courage's green symbol through Convoke after a Watchwolf attacks. It checks the target-bound resolution and 5/5 live state before a real Watchwolf block; five damage kills the blocker while three damage leaves the attacker alive. The diagnostic report credits no RAV coverage or Harbor reward.
