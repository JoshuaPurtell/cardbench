//! Red regression for Hunted Horror's Centaur token protection.

use cardbench_magic_engine::{Effect, Keyword, TokenSpec, TriggerCondition};
use cardbench_magic_rav::rav_triggered_ability_bindings;

#[test]
fn hunted_horror_creates_green_protection_from_black_centaurs() {
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-HUNTED-HORROR")
        .expect("Hunted Horror ETB binding exists")
        .ability;
    assert_eq!(ability.condition, TriggerCondition::EntersBattlefield);
    let [Effect::CreateTokenForTargetPlayer { token, count }] = ability.effects.as_slice() else {
        panic!("Hunted Horror must create one typed token effect");
    };
    assert_eq!(*count, 2);
    assert_eq!(
        token,
        &TokenSpec::hunted_centaur(),
        "the Centaurs must retain protection from black"
    );
    assert!(
        token
            .keywords
            .contains(&Keyword::Protection(cardbench_magic_engine::Color::Black))
    );
}
