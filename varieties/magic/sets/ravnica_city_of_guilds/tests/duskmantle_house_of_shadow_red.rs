//! Red discovery contract for Duskmantle, House of Shadow's mill activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardType, Color, Effect, Game, GameEvent, ManaCost,
    PlayerId, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn duskmantle_requires_exact_land_and_target_player_mill_activation() {
    let duskmantle = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DUSKMANTLE-HOUSE-OF-SHADOW")
        .expect("Duskmantle, House of Shadow definition exists");
    assert_eq!(duskmantle.name, "Duskmantle, House of Shadow");
    assert_eq!(duskmantle.mana_cost, ManaCost::new(0));
    assert_eq!(duskmantle.colors, BTreeSet::<Color>::new());
    assert_eq!(duskmantle.card_types, BTreeSet::from([CardType::Land]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&duskmantle.id));
    assert!(
        duskmantle
            .supported_rules
            .contains(&"tap-target-player-mill-one")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == duskmantle.id
            && binding.ability
                == ActivatedAbility {
                    id: "tap-target-player-mill-one",
                    mana_cost: ManaCost::new(0),
                    tap_cost: true,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![TargetRequirement::Player],
                    effects: vec![Effect::MillTargetPlayer { count: 1 }],
                }
    }));
}

#[test]
fn duskmantle_catalog_mapping_names_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(277),
        Ok("RAV-DUSKMANTLE-HOUSE-OF-SHADOW")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-DUSKMANTLE-HOUSE-OF-SHADOW"),
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
fn duskmantle_taps_to_mill_only_the_selected_player() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let duskmantle = game
        .put_on_battlefield(PlayerId(0), "RAV-DUSKMANTLE-HOUSE-OF-SHADOW")
        .expect("Duskmantle begins on battlefield");
    let opponent_top = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("opponent library fixture exists");
    game.begin_game().expect("fixture starts game");
    pass_pair(&mut game);
    pass_pair(&mut game);
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: duskmantle,
            ability_id: "tap-target-player-mill-one",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Duskmantle accepts one selected player");
    pass_pair(&mut game);

    println!("Duskmantle trace: {:#?}", game.canonical_event_log());
    assert!(game.object(duskmantle).expect("land persists").tapped);
    assert!(game.players[1].library.is_empty());
    assert_eq!(game.players[1].graveyard, vec![opponent_top]);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == opponent_top
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == duskmantle && *ability == "tap-target-player-mill-one"
    )));
    game.validate_invariants()
        .expect("targeted mill activation preserves invariant state");
}
