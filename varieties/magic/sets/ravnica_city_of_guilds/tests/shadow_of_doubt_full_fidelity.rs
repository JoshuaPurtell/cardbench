use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn rav_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_without_attackers_until(game: &mut Game, target_turn: u32, target_player: PlayerId) {
    for _ in 0..160 {
        if game.turn == target_turn
            && game.active_player == target_player
            && game.step == Step::PrecombatMain
        {
            return;
        }
        if game.step == Step::Draw && !(game.turn == 1 && game.active_player == PlayerId(0)) {
            let player = game.active_player;
            game.resolve_pending_draw(player, None)
                .expect("the ordinary draw resolves");
            game.pass_priority(player)
                .expect("drawer passes after the required draw decision");
            let opponent = game.priority;
            game.pass_priority(opponent)
                .expect("opponent advances after the required draw decision");
        } else if game.step == Step::DeclareAttackers {
            let player = game.active_player;
            game.declare_attackers(player, &[])
                .expect("empty attack declaration");
            game.pass_priority(player)
                .expect("attacker passes after declaration");
            let opponent = game.priority;
            game.pass_priority(opponent)
                .expect("opponent advances empty combat");
        } else {
            let player = game.priority;
            game.pass_priority(player)
                .expect("priority pass advances turn");
        }
    }
    panic!("did not reach the requested main phase");
}

#[test]
fn shadow_of_doubt_prevents_transmute_only_for_its_current_turn_then_expires() {
    let controller = PlayerId(0);
    let mut game = rav_game();
    let shadow = game
        .add_card(controller, "RAV-SHADOW-OF-DOUBT", Zone::Hand)
        .expect("Shadow setup");
    let shred = game
        .add_card(controller, "RAV-SHRED-MEMORY", Zone::Hand)
        .expect("Transmute setup");
    game.add_card(controller, "RAV-PLAINS", Zone::Library)
        .expect("later controller draw setup");
    let shadow_draw = game
        .add_card(controller, "RAV-FOREST", Zone::Library)
        .expect("Shadow draw setup");
    game.add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent draw setup");
    let island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("blue source setup");
    let first_swamp = game
        .put_on_battlefield(controller, "RAV-SWAMP")
        .expect("first black source setup");
    let second_swamp = game
        .put_on_battlefield(controller, "RAV-SWAMP")
        .expect("second black source setup");
    let third_swamp = game
        .put_on_battlefield(controller, "RAV-SWAMP")
        .expect("generic payment source setup");

    game.begin_game().expect("game begins");
    advance_without_attackers_until(&mut game, 1, controller);
    game.activate_mana_ability(controller, island, Color::Blue)
        .expect("blue mana");
    game.activate_mana_ability(controller, first_swamp, Color::Black)
        .expect("black mana");
    game.cast_spell(
        controller,
        CastRequest {
            card: shadow,
            targets: Vec::<Target>::new(),
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Shadow cast");
    game.pass_priority(controller).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves Shadow");

    println!("Shadow of Doubt same-turn trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(shadow_draw), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchesPrevented { source, until_turn }
            if *source == shadow && *until_turn == 1
    )));
    let before_rejection = game.event_log.clone();
    assert_eq!(
        game.transmute(controller, shred, None),
        Err(cardbench_magic_engine::RulesError::IllegalAction(
            "library searches are prevented this turn"
        ))
    );
    assert_eq!(game.zone_of(shred), Some(Zone::Hand));
    assert_eq!(
        game.event_log, before_rejection,
        "rejected search is atomic"
    );

    advance_without_attackers_until(&mut game, 3, controller);
    game.activate_mana_ability(controller, first_swamp, Color::Black)
        .expect("first untapped black source");
    game.activate_mana_ability(controller, second_swamp, Color::Black)
        .expect("second untapped black source");
    game.activate_mana_ability(controller, third_swamp, Color::Black)
        .expect("generic untapped source");
    game.transmute(controller, shred, None)
        .expect("the prior-turn marker expires before the next controller turn");

    assert_eq!(game.zone_of(shred), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { player, discarded, found }
            if *player == controller && *discarded == shred && found.is_none()
    )));
    game.validate_invariants()
        .expect("prevention lifecycle preserves state-machine invariants");
}

#[test]
fn shadow_of_doubt_is_positive_manifest_with_the_exact_effect_order() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SHADOW-OF-DOUBT")
        .expect("Shadow definition");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.effects,
        vec![
            cardbench_magic_engine::Effect::PreventLibrarySearchUntilEndOfTurn,
            cardbench_magic_engine::Effect::DrawController,
        ]
    );
}
