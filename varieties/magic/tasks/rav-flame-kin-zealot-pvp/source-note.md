# Flame-Kin Zealot source note

The compatibility target is the original Ravnica: City of Guilds printing, collector #206. The RAV card scan shows mana cost `{1}{R}{R}{W}`, a red-white 2/2 Creature — Elemental Berserker, and a mandatory ability that pumps the controller's creatures and gives them Haste for the turn. The card-list page identifies the RAV printing and links its scan: <https://www.mtg.wtf/card/rav/206/Flame-Kin-Zealot>.

The current `CardDefinition` model exposes the creature card type but has no subtype field. The task preserves Elemental and Berserker as printed identity metadata (`features.toml`) and does not claim that runtime state asserts either subtype.

The current Oracle presentation says “When this creature enters,” where the RAV printing says “When Flame-Kin Zealot comes into play.” This is the legacy wording update for the same battlefield-entry event; it does not change the trigger, affected controller, +1/+1 modifier, Haste, or duration. The current Oracle ruling clarifies that creatures controlled when the trigger resolves are affected, including Flame-Kin Zealot itself; creatures controlled or made creatures later in the turn are not. That ruling is used to interpret resolution-time membership, while the printed RAV identity remains the compatibility target.

Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective 2026-08-07, using the shared RAV source contract. No card image, scan, full prose corpus, or upstream response payload is stored in this task.
