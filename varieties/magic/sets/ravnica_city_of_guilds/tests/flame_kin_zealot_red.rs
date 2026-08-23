use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn flame_kin_zealot_is_executable_with_its_enter_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLAME-KIN-ZEALOT")
        .expect("Flame-Kin Zealot exists");
    assert_eq!(definition.name, "Flame-Kin Zealot");
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(definition.supported_rules.contains(&"etb-team-pump-haste"));
}

#[test]
fn flame_kin_zealot_enters_and_resolves_its_team_trigger() {
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
    let zealot = game
        .add_card(PlayerId(0), "RAV-FLAME-KIN-ZEALOT", Zone::Hand)
        .expect("Zealot enters hand");
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("existing creature enters");
    game.set_entered_turn_for_setup(watchwolf, 0)
        .expect("old creature");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain enters")
        })
        .collect::<Vec<_>>();
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains enters");
    game.begin_game().expect("game starts");
    while game.step != Step::PrecombatMain {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to main phase");
        game.pass_priority(PlayerId(1 - priority.0))
            .expect("pass turn priority");
    }
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("red mana");
    }
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("white mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: zealot,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Zealot casts");
    game.pass_priority(PlayerId(0))
        .expect("spell caster passes");
    game.pass_priority(PlayerId(1)).expect("Zealot resolves");
    assert_eq!(game.zone_of(zealot), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == zealot && *ability == "etb-team-pump-haste"
    )));
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("team trigger resolves");
    let zealot_characteristics = game
        .characteristics(zealot)
        .expect("Zealot characteristics");
    assert_eq!(
        (
            zealot_characteristics.power,
            zealot_characteristics.toughness
        ),
        (Some(3), Some(3))
    );
    assert!(zealot_characteristics.keywords.contains(&Keyword::Haste));
    let watchwolf_characteristics = game
        .characteristics(watchwolf)
        .expect("Watchwolf characteristics");
    assert_eq!(
        (
            watchwolf_characteristics.power,
            watchwolf_characteristics.toughness
        ),
        (Some(4), Some(4))
    );
    assert!(watchwolf_characteristics.keywords.contains(&Keyword::Haste));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == zealot && *ability == "etb-team-pump-haste"
    )));
}
