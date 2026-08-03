//! Full-fidelity contract for Brightflame's X-dependent Radiance ledger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_damage_replacement_effect_bindings,
};

#[test]
fn brightflame_has_an_exact_chosen_x_radiance_damage_life_definition() {
    assert_eq!(
        executable_definition_id_for_collector(194),
        Ok("RAV-BRIGHTFLAME")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BRIGHTFLAME")
        .expect("Brightflame definition exists");

    assert_eq!(definition.name, "Brightflame");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Red, Color::Red, Color::White, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"radiance-chosen-x-damage-total-life-gain")
    );
}

#[test]
fn brightflame_gains_only_the_radiance_damage_that_was_actually_committed() {
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture builds");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("RAV damage replacements register");
    let brightflame = game
        .add_card(PlayerId(0), "RAV-BRIGHTFLAME", Zone::Hand)
        .expect("Brightflame setup");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target setup");
    let prevented_peer = game
        .put_on_battlefield(PlayerId(1), "RAV-PHYTOHYDRA")
        .expect("same-color prevention setup");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("red payment setup");
    game.grant_mana(PlayerId(0), Color::White, 3)
        .expect("white payment setup");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: brightflame,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Red, Color::White],
            hybrid: vec![],
        },
    )
    .expect("Brightflame X=2 casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    println!("brightflame_event_log={:?}", game.canonical_event_log());
    assert_eq!(game.players[0].life, 22);
    assert_eq!(
        game.object(target).expect("target remains live").damage,
        2,
        "the targeted Watchwolf receives the declared X"
    );
    assert_eq!(
        game.object(prevented_peer)
            .expect("Phytohydra remains live")
            .damage,
        0,
        "the same-color Phytohydra prevents its own packet"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
            if *source == brightflame && *permanent == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePreventedWithPlusOneCounters { source, permanent, amount: 2, .. }
            if *source == brightflame && *permanent == prevented_peer
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageBatchLifeGained { source, controller, amount: 2, .. }
            if *source == brightflame && *controller == PlayerId(0)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player, amount: 2 } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("damage-derived life ledger preserves invariants");
}
