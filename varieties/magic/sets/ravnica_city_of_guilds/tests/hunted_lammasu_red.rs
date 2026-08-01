//! Red discovery contract for the White source-lane Hunted Lammasu path.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, CreatureSubtype, Game, GameEvent, Keyword, ManaCost, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn hunted_lammasu_requires_flying_and_a_targeted_opponent_horror_etb() {
    let lammasu = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-LAMMASU")
        .expect("Hunted Lammasu definition exists");
    assert_eq!(
        lammasu.mana_cost,
        ManaCost::with_colors(2, [Color::White, Color::White])
    );
    assert_eq!(lammasu.card_types, [CardType::Creature].into());
    assert_eq!((lammasu.power, lammasu.toughness), (Some(5), Some(5)));
    assert_eq!(lammasu.keywords, [Keyword::Flying]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&lammasu.id));
    assert!(
        lammasu
            .supported_rules
            .contains(&"etb-targeted-opponent-horror-token")
    );
}

#[test]
fn hunted_lammasu_etb_creates_one_black_horror_for_its_targeted_opponent() {
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
    let lammasu = game
        .add_card(PlayerId(0), "RAV-HUNTED-LAMMASU", Zone::Hand)
        .expect("Hunted Lammasu enters hand");
    let plains = [
        game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
            .expect("first Plains enters"),
        game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
            .expect("second Plains enters"),
        game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
            .expect("third Plains enters"),
        game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Battlefield)
            .expect("fourth Plains enters"),
    ];
    game.begin_game().expect("fixture starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward main phase");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward main phase");
    }
    for plain in plains {
        game.activate_mana_ability(PlayerId(0), plain, Color::White)
            .expect("Plains produces white mana");
    }
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: lammasu,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Hunted Lammasu casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes creature spell");
    game.pass_priority(PlayerId(1))
        .expect("creature resolves and ETB trigger stacks");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Player(PlayerId(1))]),
        "the trigger targets exactly one opponent"
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("Horror trigger resolves");

    let opponent_battlefield = &game
        .player(PlayerId(1))
        .expect("opponent exists")
        .battlefield;
    assert_eq!(opponent_battlefield.len(), 1);
    let horror = opponent_battlefield[0];
    let characteristics = game
        .characteristics(horror)
        .expect("Horror has characteristics");
    assert_eq!(characteristics.colors, [Color::Black].into_iter().collect());
    assert_eq!(characteristics.card_types, [CardType::Creature].into());
    assert!(
        characteristics
            .creature_subtypes
            .contains(&CreatureSubtype::Horror)
    );
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(4))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == lammasu && *ability == "etb-opponent-horror"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, token } if *player == PlayerId(1) && *token == horror
    )));
    println!("hunted_lammasu_event_log={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Hunted Lammasu trace preserves invariants");
}
