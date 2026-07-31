//! Red probe for Hunted Horror's targeted opponent token ETB ability.

use cardbench_magic_engine::{
    CastRequest, Color, CreatureSubtype, Game, GameEvent, Keyword, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn hunted_horror_etb_creates_two_target_opponent_tokens() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let horror = game.add_card(PlayerId(0), "RAV-HUNTED-HORROR", Zone::Hand);
    println!(
        "Hunted Horror setup result: {horror:?}; events={:?}",
        game.canonical_event_log()
    );
    let horror = horror.expect("Hunted Horror must be executable");
    let swamps = [
        game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
            .expect("first Swamp enters"),
        game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
            .expect("second Swamp enters"),
    ];
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Swamp produces black mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: horror,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Hunted Horror casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the creature spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes the creature spell");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(1))]),
        "the trigger selects exactly one opponent"
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("token trigger resolves");
    println!(
        "Hunted Horror green trace: {:?}",
        game.canonical_event_log()
    );
    let battlefield = &game
        .player(PlayerId(1))
        .expect("opponent exists")
        .battlefield;
    assert_eq!(battlefield.len(), 2, "exactly two Centaurs are created");
    for token in battlefield {
        let characteristics = game.characteristics(*token).expect("token characteristics");
        assert_eq!(characteristics.colors, [Color::Green].into_iter().collect());
        assert_eq!(characteristics.power, Some(3));
        assert_eq!(characteristics.toughness, Some(3));
        assert!(
            characteristics
                .creature_subtypes
                .contains(&CreatureSubtype::Centaur)
        );
        assert!(!characteristics.keywords.contains(&Keyword::FirstStrike));
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(1)))
            .count(),
        2
    );
}
