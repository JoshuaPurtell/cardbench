# RAV Ursapine single-card diagnostic

This task targets Ursapine (RAV collector #188), a green 3/3 creature costing `{3}{G}{G}`. In a legal shuffled 60-card PvP game, Ursapine is cast and its `{G}` activated ability targets a controlled Watchwolf. The trace verifies that Watchwolf becomes 4/4, attacks for four combat damage, and returns to 3/3 when the end-of-turn effect expires.

The authority checks the printed identity and candidate-owned ability binding, deterministic full-game replay, ordered payment/stack/combat/terminal receipts, and in-game mutations to the pump amount, ability cost, and binding. Reports keep `pilot_reward`, `harbor_reward`, and `cards_credited` at zero. This diagnostic does not complete set coverage or grant qualification credit.

The only editable candidate file is `src/cards/ursapine.rs`, whose API exposes `definition()` and `activated_ability()`. The task bundle's compile-valid stub preserves identity and cost but gives the target no power, so the trusted authority must reject it.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, with an authority cutoff of September 24, 2026. A candidate trial and independent review remain separate qualification steps.
