use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn nightmare_void_requires_targeted_discard_and_dredge() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NIGHTMARE-VOID")
        .expect("Nightmare Void definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into());
    assert_eq!(definition.card_types, [CardType::Sorcery].into());
    assert!(definition.keywords.contains(&Keyword::Dredge(2)));
    assert!(definition.supported_rules.contains(&"targeted-discard"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
