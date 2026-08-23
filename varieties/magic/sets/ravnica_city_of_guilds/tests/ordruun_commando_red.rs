//! Red milestone for Ordruun Commando's damage-prevention activation.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn ordruun_commando_is_full_fidelity_and_declares_prevention() {
    let commando = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ORDRUUN-COMMANDO")
        .expect("Ordruun Commando definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&commando.id));
    assert!(
        commando
            .supported_rules
            .contains(&"activated-prevent-one-damage-to-self")
    );
}

#[test]
fn ordruun_commando_prevents_the_next_damage_and_logs_the_shield() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let commando = game
        .put_on_battlefield(PlayerId(0), "RAV-ORDRUUN-COMMANDO")
        .expect("Ordruun enters");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char enters hand");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains enters");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain enters")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(commando, 0)
        .expect("fixture makes Ordruun long-controlled");
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1))
            .expect("early response pass");
    }
    assert_eq!(game.step, Step::PrecombatMain);

    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("white mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: commando,
            ability_id: "prevent-one-damage-to-self",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("prevention ability activates");
    game.pass_priority(PlayerId(0))
        .expect("ability controller passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(
        game.object(commando).expect("Ordruun exists").damage_shield,
        1
    );

    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Char mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(commando)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char casts at Ordruun");
    game.pass_priority(PlayerId(0))
        .expect("Char controller passes");
    game.pass_priority(PlayerId(1)).expect("Char resolves");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(target),
            amount: 1,
        } if *source == char && *target == commando
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent {
            source,
            permanent,
            amount: 3,
        } if *source == char && *permanent == commando
    )));
    assert_eq!(game.zone_of(commando), Some(Zone::Graveyard));
    game.validate_invariants().expect("Ordruun trace is valid");
}
