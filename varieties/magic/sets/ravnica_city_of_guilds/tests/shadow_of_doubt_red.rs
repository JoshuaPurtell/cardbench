use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn shadow_of_doubt_requires_turn_scoped_library_search_prevention_and_a_draw() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SHADOW-OF-DOUBT")
        .expect("Shadow of Doubt definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(definition.colors, [Color::Blue, Color::Black].into());
    assert_eq!(definition.card_types, [CardType::Instant].into());
    assert!(
        definition
            .supported_rules
            .contains(&"library-search-prevention")
    );
    assert!(definition.supported_rules.contains(&"draw"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
