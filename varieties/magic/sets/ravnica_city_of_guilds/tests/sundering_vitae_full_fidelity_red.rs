use cardbench_magic_engine::{CardType, Color, Game, PlayerId, Target, Zone};
use cardbench_magic_rav::{card_definitions, SET_CODE};

#[test]
fn sundering_vitae_is_cataloged_and_destroys_an_artifact_or_enchantment() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUNDERING-VITAE")
        .expect("Sundering Vitae must be present in the executable catalog");
    assert_eq!(definition.set_code, SET_CODE);
    assert_eq!(definition.colors, std::collections::BTreeSet::from([Color::Green]));
    assert_eq!(definition.card_types, std::collections::BTreeSet::from([CardType::Instant]));

    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-SUNDERING-VITAE", Zone::Hand)
        .expect("card is executable");
    let artifact = game
        .add_card(PlayerId(1), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("target exists");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("test mana");
    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(artifact)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast succeeds");
    assert_eq!(game.zone_of(artifact), Some(Zone::Battlefield));
}
