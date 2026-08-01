//! Red regression for Moldervine Cloak's persistent attachment slice.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn moldervine_cloak_has_exact_cost_dredge_and_attachment_support() {
    let cloak = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOLDERVINE-CLOAK")
        .expect("Moldervine Cloak definition exists");
    assert_eq!(cloak.name, "Moldervine Cloak");
    assert_eq!(cloak.mana_cost, ManaCost::with_colors(2, [Color::Green]));
    assert_eq!(
        cloak.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(cloak.keywords.contains(&Keyword::Dredge(2)));
    assert!(cloak.supported_rules.contains(&"aura-attach-and-static-pt"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&cloak.id));
}

#[test]
fn moldervine_cloak_resolves_as_a_persistent_plus_three_plus_three_attachment() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Moldervine Cloak setup");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Battlefield)
        .expect("target creature setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("Cloak mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Moldervine Cloak casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Cloak resolves");

    assert_eq!(game.zone_of(cloak), Some(Zone::Battlefield));
    let characteristics = game.characteristics(target).expect("target remains live");
    assert_eq!(characteristics.power, Some(5));
    assert_eq!(characteristics.toughness, Some(6));
    game.validate_invariants()
        .expect("persistent attachment preserves invariant-safe layers");
}
