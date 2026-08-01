//! Red regression for the unported Clinging Darkness Aura.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn clinging_darkness_has_its_exact_persistent_aura_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CLINGING-DARKNESS")
        .expect("Clinging Darkness definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Clinging Darkness");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into_iter().collect());
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(definition.keywords.is_empty());
    assert_eq!(
        definition.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: -3,
            toughness: -1,
        }]
    );
}

#[test]
fn clinging_darkness_attaches_and_retains_its_exact_negative_modifier() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let aura = game
        .add_card(PlayerId(0), "RAV-CLINGING-DARKNESS", Zone::Hand)
        .expect("Aura setup");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLIATH-SPIDER", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 3)
        .expect("Aura mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");

    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura remains live").attached_to,
        Some(target)
    );
    let characteristics = game.characteristics(target).expect("target remains live");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(5));
    game.validate_invariants()
        .expect("negative Aura modifier remains invariant-safe");
}
