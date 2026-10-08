# Assignment: implement Stasis Cell

Implement RAV #66 in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/stasis_cell.rs`.

Required behavior:

- Define the blue `{4}{U}` Enchantment — Aura and attach it to a target creature as it resolves.
- While attached, the creature cannot attack or block. Its nonmana activated abilities still work.
- Allow its controller to pay `{3}{U}` to attach Stasis Cell to a target creature. The activated ability has no sorcery-speed restriction.
- Preserve the target and Aura incarnations through attachment and reattachment.

Use the trusted authority and scorer to validate the shuffled 60-card duel, typed payments, combat declarations, a nonmana activated ability used while enchanted, the reattachment, replay, mutations, and terminal result. The authority checks the exact candidate source before invoking candidate functions; keep candidate behavior in the required module API.
