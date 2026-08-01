//! Regression for Vigor Mortis's spent-green graveyard return.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, ManaCost, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn vigor_mortis_has_its_exact_targeted_graveyard_return_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VIGOR-MORTIS")
        .expect("Vigor Mortis definition exists");
    assert_eq!(definition.name, "Vigor Mortis");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"return-target-creature-card-from-graveyard-to-battlefield")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"spent-green-plus-one-plus-one-counter")
    );
}

fn resolve_with_generic_spend(generic: [Color; 2]) -> (Game, cardbench_magic_engine::ObjectId) {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-VIGOR-MORTIS", Zone::Hand)
        .expect("spell setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("creature target setup");
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black symbols");
    for color in generic {
        game.grant_mana(PlayerId(0), color, 1)
            .expect("generic payment mana");
    }

    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: generic.to_vec(),
            hybrid: vec![],
        },
    )
    .expect("Vigor Mortis casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    (game, target)
}

#[test]
fn vigor_mortis_returns_its_owners_creature_and_uses_spent_green_for_counter() {
    let (game, target) = resolve_with_generic_spend([Color::Green, Color::Green]);
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .power,
        Some(4)
    );
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .toughness,
        Some(4)
    );
    game.validate_invariants()
        .expect("spent-green counter return stays valid");
}

#[test]
fn vigor_mortis_without_spent_green_returns_without_a_counter() {
    let (game, target) = resolve_with_generic_spend([Color::Black, Color::Black]);
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(target)
            .expect("returned creature characteristics")
            .power,
        Some(3)
    );
    game.validate_invariants()
        .expect("non-green return stays valid");
}
