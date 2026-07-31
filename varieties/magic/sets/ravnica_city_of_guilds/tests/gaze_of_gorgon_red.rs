//! Red regression for a false executable semantic slice.
//!
//! This records CardBench-authored semantic facts from the public RAV audit;
//! it does not retain card rules text or artwork.

use cardbench_magic_engine::{Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn gaze_of_gorgon_must_not_be_modelled_as_a_temporary_power_toughness_modifier() {
    let gaze = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GAZE-OF-THE-GORGON")
        .expect("red regression starts from the currently executable definition");

    assert_eq!(
        gaze.mana_cost,
        ManaCost::with_hybrid(
            3,
            [],
            [HybridManaSymbol {
                first: Color::Black,
                second: Color::Green,
            }],
        ),
        "the public RAV audit rejects the current incorrect casting-cost fixture"
    );
    assert!(
        gaze.effects.is_empty(),
        "the engine has neither the regeneration replacement nor the delayed combat-history destruction required by this card"
    );
}
