# Caregiver card task

Implement the candidate-owned Caregiver definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/caregiver.rs`.

Caregiver (#6) is a white 1/1 creature costing `{W}`. Its activated ability costs `{W}`, sacrifices Caregiver as a cost, and creates a one-point shield for a target creature or player for the turn. The deterministic full-game diagnostic pays white to cast Caregiver and to activate it, sacrifices the source, targets its controller, and prevents the next point of combat damage from an attacking Elves of Deep Shadow. A Watchwolf provides terminal damage in that same duel.

The authority checks candidate-source structure and registry delegation, live characteristics, exact colored costs, ordered typed cast/resolve/entry/sacrifice/activation/prevention/combat events, deterministic replay, and four compiled semantic mutants. This diagnostic vertical covers 1 of 291 unique RAV card names from 306 printings, credits no cards, and awards zero reward. No provider trial is part of this task.

The materialized task workspace exposes only the candidate module. Its Harbor bundle runs the bounded scorer against the trusted authority.
