//! Red coverage probe for Keening Banshee's ETB creature modifier.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn keening_banshee_exposes_its_complete_etb_slice() {
    let banshee = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-KEENING-BANSHEE")
        .expect("Keening Banshee definition exists");
    assert_eq!(
        banshee.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
    assert_eq!(
        banshee.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((banshee.power, banshee.toughness), (Some(2), Some(2)));
    assert!(banshee.keywords.contains(&Keyword::Flying));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&banshee.id));
    assert!(
        banshee
            .supported_rules
            .contains(&"enter-battlefield-targeted-minus-two-minus-two")
    );
}

#[test]
fn keening_banshee_stacks_and_resolves_its_targeted_etb_modifier() {
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
    let banshee = game
        .add_card(PlayerId(0), "RAV-KEENING-BANSHEE", Zone::Hand)
        .expect("Banshee enters hand");
    let target = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("opponent creature enters battlefield");
    let swamps = (0..4)
        .map(|_| {
            game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Battlefield)
                .expect("Swamp enters battlefield")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to precombat main");
    }
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Swamp produces black mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: banshee,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Keening Banshee");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Banshee resolves");
    assert_eq!(
        game.stack.last().map(|object| object.targets.clone()),
        Some(vec![Target::Permanent(target)])
    );
    game.pass_priority(PlayerId(0))
        .expect("trigger controller passes");
    game.pass_priority(PlayerId(1))
        .expect("ETB trigger resolves");

    println!("Keening Banshee trace: {:?}", game.canonical_event_log());
    assert_eq!(
        game.characteristics(target).expect("target exists").power,
        Some(1)
    );
    assert_eq!(
        game.characteristics(target)
            .expect("target exists")
            .toughness,
        Some(1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == banshee && *ability == "etb-target-minus-two"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { target: effect_target, .. }
            if *effect_target == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == banshee && *ability == "etb-target-minus-two"
    )));
}
