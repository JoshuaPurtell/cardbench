# RAV Dogpile single-card diagnostic

This task targets Dogpile (RAV collector #120), a red instant costing `{3}{R}`. In a legal shuffled 60-card PvP game, two controlled Watchwolves attack and Dogpile is cast after attackers are declared. The trace verifies that Dogpile selects the opponent and deals exactly two damage, matching the number of attacking creatures when the spell resolves.

The authority checks the printed identity and effect, the exact colored casting payment, ordered combat/cast/damage/resolution receipts, deterministic full-game replay, and mutations removing the damage effect, restricting the target to creatures, or changing the mana cost. Reports keep `pilot_reward`, `harbor_reward`, and `cards_credited` at zero. This diagnostic does not complete set coverage or grant qualification credit.

The only editable candidate file is `src/cards/dogpile.rs`, exposing `definition()`. The task bundle's compile-valid stub preserves the printed card identity and damage effect but incorrectly limits its target to creatures, so the trusted authority must reject it.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, with an authority cutoff of September 24, 2026. A candidate trial and independent review remain separate qualification steps.
