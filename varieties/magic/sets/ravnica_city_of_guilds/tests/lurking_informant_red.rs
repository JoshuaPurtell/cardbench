//! Red discovery contract for Lurking Informant's private top-library choice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DecisionKind, DecisionSelection, DecisionVisibility, Game,
    GameEvent, HybridManaSymbol, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn lurking_informant_requires_target_player_top_library_may_graveyard_activation() {
    let informant = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LURKING-INFORMANT")
        .expect("Lurking Informant definition exists");

    assert_eq!(informant.name, "Lurking Informant");
    assert_eq!(
        informant.mana_cost,
        ManaCost::with_hybrid(
            1,
            BTreeSet::new(),
            [HybridManaSymbol {
                first: Color::Blue,
                second: Color::Black,
            }],
        )
    );
    assert_eq!(
        informant.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(informant.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(informant.power, Some(1));
    assert_eq!(informant.toughness, Some(2));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&informant.id));
    assert!(
        informant
            .supported_rules
            .contains(&"tap-two-target-player-private-top-library-may-graveyard")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == informant.id
            && binding.ability.id == "two-tap-target-player-top-library-may-graveyard"
            && binding.ability.mana_cost == ManaCost::new(2)
            && binding.ability.tap_cost
            && binding.ability.targets == vec![TargetRequirement::Player]
    }));
}

#[test]
fn lurking_informant_catalog_mapping_requires_the_typed_definition() {
    assert_eq!(
        executable_definition_id_for_collector(249),
        Ok("RAV-LURKING-INFORMANT")
    );
}

fn game() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn activation(source: cardbench_magic_engine::ObjectId) -> AbilityActivation {
    AbilityActivation {
        source,
        ability_id: "two-tap-target-player-top-library-may-graveyard",
        sacrifice_sources: vec![],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![Target::Player(PlayerId(1))],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // Private snapshot, opponent pass, and mill decisions form one transcript.
fn lurking_informant_keeps_a_private_exact_top_snapshot_until_its_controller_mills() {
    let mut game = game();
    let informant = game
        .put_on_battlefield(PlayerId(0), "RAV-LURKING-INFORMANT")
        .expect("Informant enters fixture battlefield");
    game.set_entered_turn_for_setup(informant, 0)
        .expect("Informant entered on an earlier fixture turn");
    let top = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("target library top exists");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("first generic payment source");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("second generic payment source");
    game.begin_game().expect("fixture starts game");
    game.clear_event_log();
    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("first generic mana source activates");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("second generic mana source activates");

    game.activate_ability(PlayerId(0), activation(informant))
        .expect("Informant activation accepts target player");
    pass_pair(&mut game);

    let controller = game
        .view_for_player(PlayerId(0))
        .expect("controller receives private decision view");
    let decision = controller
        .pending_decision
        .expect("private may-choice opens");
    assert_eq!(
        decision.kind,
        DecisionKind::TargetPlayerLibraryTopMayGraveyard
    );
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![top],
        "only the activating controller receives the exact private top card"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("target player receives view")
            .pending_decision
            .is_none(),
        "the targeted player does not receive the controller's private snapshot"
    );
    assert!(game.stack.last().is_some_and(|item| item.card == informant));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardsLookedAt { cards, .. } if cards.contains(&top)
    )));

    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![top]),
    )
    .expect("controller elects to mill the inspected top card");

    println!(
        "lurking_informant_mill_trace={:#?}",
        game.canonical_event_log()
    );
    assert!(game.object(informant).expect("source persists").tapped);
    assert_eq!(game.zone_of(top), Some(Zone::Graveyard));
    assert!(game.players[1].library.is_empty());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateTargetPlayerLibraryTopChoiceOpened {
            decision: opened,
            controller: PlayerId(0),
            source,
            target: PlayerId(1),
            ..
        } if *opened == decision.id && *source == informant
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == top
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == informant
                && *ability == "two-tap-target-player-top-library-may-graveyard"
    )));
    game.validate_invariants()
        .expect("private top-library mill trace preserves state-machine invariants");
}

#[test]
fn lurking_informant_allows_the_private_retain_branch_without_a_zone_move() {
    let mut game = game();
    let informant = game
        .put_on_battlefield(PlayerId(0), "RAV-LURKING-INFORMANT")
        .expect("Informant enters fixture battlefield");
    game.set_entered_turn_for_setup(informant, 0)
        .expect("Informant entered on an earlier fixture turn");
    let top = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("target library top exists");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("first generic payment source");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("second generic payment source");
    game.begin_game().expect("fixture starts game");
    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("first generic mana source activates");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("second generic mana source activates");
    game.activate_ability(PlayerId(0), activation(informant))
        .expect("Informant activation accepts target player");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private may-choice opens");

    game.submit_decision(PlayerId(0), decision.id, DecisionSelection::Objects(vec![]))
        .expect("controller declines the optional graveyard move");

    assert_eq!(game.zone_of(top), Some(Zone::Library));
    assert_eq!(game.players[1].library, vec![top]);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == top
        )),
        "the declined may branch does not move the private top card"
    );
    game.validate_invariants()
        .expect("private retain trace preserves state-machine invariants");
}
