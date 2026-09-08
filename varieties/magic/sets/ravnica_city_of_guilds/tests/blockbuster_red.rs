//! Red discovery contract for Blockbuster's absent stack-backed activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardType, Color, Effect, Game, GameEvent, ManaCost,
    PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn blockbuster_requires_printed_enchantment_and_sacrifice_activation() {
    let blockbuster = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLOCKBUSTER")
        .expect("Blockbuster definition exists");
    assert_eq!(blockbuster.name, "Blockbuster");
    assert_eq!(blockbuster.mana_cost, ManaCost::with_colors(3, [Color::Red, Color::Red]));
    assert_eq!(blockbuster.colors, BTreeSet::from([Color::Red]));
    assert_eq!(blockbuster.card_types, BTreeSet::from([CardType::Enchantment]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&blockbuster.id));
    assert!(
        blockbuster
            .supported_rules
            .contains(&"sacrifice-tapped-creature-and-player-damage")
    );

    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == blockbuster.id
            && binding.ability
                == ActivatedAbility {
                    id: "sacrifice-tapped-creature-and-player-damage",
                    mana_cost: ManaCost::with_colors(1, [Color::Red]),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: true,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![Effect::DealDamageToEachTappedCreatureAndPlayer { amount: 3 }],
                }
    }));
}

#[test]
fn blockbuster_catalog_mapping_names_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(115),
        Ok("RAV-BLOCKBUSTER")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-BLOCKBUSTER"),
        "catalog mapping identifies the complete typed definition"
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn blockbuster_sacrifice_hits_only_tapped_creatures_and_each_player() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let blockbuster = game
        .put_on_battlefield(PlayerId(0), "RAV-BLOCKBUSTER")
        .expect("Blockbuster begins on battlefield");
    let own_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("own creature begins on battlefield");
    let opposing_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opposing creature begins on battlefield");
    game.set_tapped_for_setup(opposing_creature, true).unwrap();
    game.begin_game().expect("fixture starts game");
    pass_pair(&mut game);
    pass_pair(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Red, 2)
        .expect("generic activation mana is a legal action");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: blockbuster,
            ability_id: "sacrifice-tapped-creature-and-player-damage",
            sacrifice_sources: vec![blockbuster],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Blockbuster activation succeeds");
    pass_pair(&mut game);

    println!("Blockbuster trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(blockbuster), Some(Zone::Graveyard));
    assert_eq!(game.players[0].life, 17);
    assert_eq!(game.players[1].life, 17);
    assert_eq!(game.zone_of(own_creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(opposing_creature), Some(Zone::Graveyard));
    for creature in [opposing_creature] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPermanent { source, permanent, amount }
                if *source == blockbuster && *permanent == creature && *amount == 3
        )));
    }
    for player in [PlayerId(0), PlayerId(1)] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPlayer { source, player: damaged, amount }
                if *source == blockbuster && *damaged == player && *amount == 3
        )));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == blockbuster && *ability == "sacrifice-tapped-creature-and-player-damage"
    )));
    game.validate_invariants()
        .expect("global activation preserves invariant state");
}
