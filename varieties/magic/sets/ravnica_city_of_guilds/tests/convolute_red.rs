//! Red discovery probe for Convolute's resolution-time payment choice.

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn convolute_has_an_exact_nonautomatic_counter_unless_payment_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONVOLUTE")
        .expect("Convolute definition exists");

    assert_eq!(definition.mana_cost, ManaCost::with_colors(2, [Color::Blue]));
    assert!(definition.card_types.contains(&CardType::Instant));
    assert!(definition
        .supported_rules
        .contains(&"counter-target-spell-unless-controller-pays-4"));
    assert!(definition.effects.iter().any(|effect| {
        format!("{effect:?}").contains("CounterTargetSpellUnlessControllerPays")
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
