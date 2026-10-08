# RAV Dryad's Caress single-card diagnostic

This task targets Dryad's Caress (RAV collector #160), a green instant costing `{4}{G}{G}`. In a shuffled 60-card PvP game, the pilot attacks with two controlled Watchwolves and casts Dryad's Caress after attackers are declared, paying White mana toward the generic cost. The trace verifies that its life gain equals the number of creatures on the battlefield as the spell resolves, both attacking Watchwolves untap, and the opponent's tapped creature remains tapped.

The authority checks the printed identity and both ordered effects, exact mana payment, combat/cast/life/untap/resolution/terminal receipts, deterministic full-game replay, and mutations removing life gain, removing conditional untap, changing its required mana color, or changing the mana cost. Reports keep `pilot_reward`, `harbor_reward`, and `cards_credited` at zero. This diagnostic does not complete set coverage or grant qualification credit.

The only editable candidate file is `src/cards/dryads_caress.rs`, exposing `definition()`. The task bundle's compile-valid stub preserves the printed identity and life-gain effect but incorrectly makes the untap condition depend on Green mana, so the trusted authority must reject it.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, with an authority cutoff of September 24, 2026. A candidate trial and independent review remain separate qualification steps.
