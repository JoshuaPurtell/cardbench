//! Event-log contract for Crown of Convergence's controller-scoped static rules.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_static_library_top_reveal_bindings,
};

fn game() -> Game {
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV game builds with Crown bindings");
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .expect("RAV top-library bindings register");
    game
}

#[test]
fn crown_reveals_only_its_controllers_top_and_rotates_the_live_color_layer() {
    let mut game = game();
    let crown = game
        .put_on_battlefield(PlayerId(0), "RAV-CROWN-OF-CONVERGENCE")
        .expect("Crown setup");
    let shared_color = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("green-white recipient setup");
    let off_color = game
        .put_on_battlefield(PlayerId(0), "RAV-SNAPPING-DRAKE")
        .expect("blue nonrecipient setup");
    let opponent_shared_color = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent recipient setup");
    let bottom = game
        .add_card(PlayerId(0), "RAV-BLOCKBUSTER", Zone::Library)
        .expect("noncreature controller library bottom");
    let top = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("creature controller library top");
    let opponent_top = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Library)
        .expect("opponent library top remains private");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("green source setup");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("white source setup");
    game.begin_game().expect("game begins");

    assert_eq!(
        game.characteristics(shared_color)
            .expect("shared-color characteristics")
            .power,
        Some(4),
        "Crown buffs the 3/3 green-white creature"
    );
    assert_eq!(
        game.characteristics(off_color)
            .expect("off-color characteristics")
            .power,
        Some(3),
        "Crown never buffs a blue-only controlled creature"
    );
    assert_eq!(
        game.characteristics(opponent_shared_color)
            .expect("opponent characteristics")
            .power,
        Some(3),
        "Crown never reaches an opponent's same-color creature"
    );
    let visible = game
        .view_for_player(PlayerId(1))
        .expect("opponent public view")
        .revealed_library_tops;
    assert_eq!(visible.len(), 1, "only Crown's controller top is public");
    assert_eq!(visible[0].owner, PlayerId(0));
    assert_eq!(visible[0].card.id, top);
    assert!(visible.iter().all(|view| view.card.id != opponent_top));

    game.activate_mana_ability(PlayerId(0), forest, cardbench_magic_engine::Color::Green)
        .expect("Forest produces green");
    game.activate_mana_ability(PlayerId(0), plains, cardbench_magic_engine::Color::White)
        .expect("Plains produces white");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: crown,
            ability_id: "green-white-rotate-controller-library-top-to-bottom",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Crown activation is stack-backed");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert_eq!(game.players[0].library, vec![top, bottom]);
    assert_eq!(
        game.characteristics(shared_color)
            .expect("static layer after rotation")
            .power,
        Some(3),
        "the noncreature new top removes Crown's modifier"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryTopMovedToBottom { player: PlayerId(0), card } if *card == top
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == crown
                && *ability == "green-white-rotate-controller-library-top-to-bottom"
    )));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CROWN-OF-CONVERGENCE"));
    eprintln!(
        "crown_of_convergence_trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Crown controller-library lifecycle remains invariant-valid");
}
