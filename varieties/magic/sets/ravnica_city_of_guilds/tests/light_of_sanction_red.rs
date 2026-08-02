//! Red discovery contract for Light of Sanction's static friendly-fire prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_static_continuous_effect_bindings,
};

#[test]
fn light_of_sanction_has_its_static_controller_relative_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LIGHT-OF-SANCTION")
        .expect("Light of Sanction definition exists");
    assert_eq!(definition.name, "Light of Sanction");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"static-prevent-friendly-source-damage")
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn light_of_sanction_prevents_only_friendly_source_damage_to_controller_creatures() {
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        [],
        [],
        [],
        [],
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV fixture builds");
    game.put_on_battlefield(PlayerId(0), "RAV-LIGHT-OF-SANCTION")
        .expect("Light of Sanction setup");
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("friendly creature setup");
    let friendly_char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("friendly source setup");
    let opposing_char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("opposing source setup");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("friendly pre-game payment setup");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("opposing pre-game payment setup");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: friendly_char,
            targets: vec![Target::Permanent(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("friendly Char casts");
    pass_pair(&mut game);
    assert_eq!(
        game.object(watchwolf)
            .expect("protected creature remains")
            .damage,
        0
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(target),
            amount: 4,
        } if *source == friendly_char && *target == watchwolf
    )));

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: opposing_char,
            targets: vec![Target::Permanent(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opposing Char casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(watchwolf), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent {
            source,
            permanent,
            amount: 4,
        } if *source == opposing_char && *permanent == watchwolf
    )));
    eprintln!("Light of Sanction trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Light of Sanction preserves invariant state");
}
