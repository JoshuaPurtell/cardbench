# Stasis Cell source reconciliation

RAV #66 is Stasis Cell, a blue `{4}{U}` Enchantment — Aura. The original Ravnica: City of Guilds printing says that the enchanted creature can't attack or block, and gives the Aura an activated ability costing `{3}{U}` to attach Stasis Cell to a target creature. The current Oracle text preserves those same gameplay operations; the wording difference is only the current templating of the Aura line and ability.

The supported task semantics follow the printed card: attaching the Aura adds only the `CannotAttackOrBlock` restriction, and activating its reattachment ability pays three generic mana and one blue mana before attaching it to the targeted creature. The enchanted creature's other nonmana activated abilities remain available. The preexisting inline RAV registry model was not source-faithful: it also suppressed nonmana activated abilities and represented reattachment as colorless `{3}`. The candidate module corrects both fields. No broader ability behavior is inferred from Oracle text.

Public identity and current Oracle source: <https://scryfall.com/card/rav/66/stasis-cell> and <https://www.mtg.wtf/card/rav/66/Stasis-Cell>. The project catalog records collector number 66 and `RAV-STASIS-CELL`. The semantic summary retains no art, scan, flavor text, or rules-text corpus.
