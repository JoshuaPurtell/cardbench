//! Red milestone for Indentured Oaf's source-color damage prevention.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn indentured_oaf_declares_red_damage_prevention() {
    let oaf = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INDENTURED-OAF")
        .expect("Indentured Oaf definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&oaf.id));
    assert!(
        oaf.supported_rules
            .contains(&"prevent-damage-from-red-sources")
    );
}

#[test]
fn indentured_oaf_prevents_all_damage_from_red_char() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let oaf = game
        .put_on_battlefield(PlayerId(0), "RAV-INDENTURED-OAF")
        .expect("Oaf enters");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char enters hand");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1))
            .expect("early response pass");
    }
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Char mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(oaf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char casts at Oaf");
    game.pass_priority(PlayerId(0))
        .expect("Char controller passes");
    game.pass_priority(PlayerId(1)).expect("Char resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(target),
            amount: 4,
        } if *source == char && *target == oaf
    )));
    assert_eq!(game.object(oaf).expect("Oaf exists").damage, 0);
    assert_eq!(game.zone_of(oaf), Some(Zone::Battlefield));
    game.validate_invariants().expect("Oaf trace is valid");
}
