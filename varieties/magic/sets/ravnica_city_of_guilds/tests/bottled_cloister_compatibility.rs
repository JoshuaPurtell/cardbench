//! Full lifecycle contract for Bottled Cloister's source-linked hand exile.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
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

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_upkeep(game: &mut Game, turn: u32, player: PlayerId) {
    for _ in 0..128 {
        if game.turn == turn && game.active_player == player && game.step == Step::Upkeep {
            return;
        }
        if game
            .view_for_player(game.active_player)
            .expect("active player view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(game.active_player, None)
                .expect("ordinary draw resolves");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("attacker view")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attack is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(PlayerId(1 - game.active_player.0))
                    .expect("defender view")
                    .blockers_declared =>
            {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty blocks are legal");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority)
                    .expect("ordinary priority advances the state machine");
            }
        }
    }
    panic!("fixture never reached requested upkeep");
}

fn add_library_buffers(game: &mut Game) {
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..8 {
            game.add_card(player, "RAV-GLASS-GOLEM", Zone::Library)
                .expect("library buffer enters setup");
        }
    }
}

#[test]
fn cloister_exiles_its_current_hand_then_returns_it_before_the_controller_draws() {
    let mut game = rav_game();
    let cloister = game
        .put_on_battlefield(PlayerId(0), "RAV-BOTTLED-CLOISTER")
        .expect("Cloister begins on battlefield");
    let first = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("first hand card enters setup");
    let second = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("second hand card enters setup");
    add_library_buffers(&mut game);

    game.begin_game().expect("game begins at controller upkeep");
    pass_pair(&mut game); // Empty first controller-upkeep return trigger resolves.
    advance_to_upkeep(&mut game, 2, PlayerId(1));
    let hand_to_exile = game
        .player(PlayerId(0))
        .expect("controller exists")
        .hand
        .clone();
    assert!(hand_to_exile.contains(&first));
    assert!(hand_to_exile.contains(&second));

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == cloister && *ability == "opponent-upkeep-exile-controller-hand-linked"
    )));
    pass_pair(&mut game);

    assert!(
        hand_to_exile
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Exile))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::HandExiledWithSource { controller, source, cards, .. }
            if *controller == PlayerId(0) && *source == cloister && cards == &hand_to_exile
    )));
    game.validate_invariants()
        .expect("exiled hand state is valid");

    advance_to_upkeep(&mut game, 3, PlayerId(0));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == cloister && *ability == "controller-upkeep-return-linked-hand-then-draw"
    )));
    pass_pair(&mut game);

    let returned_at = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::LinkedHandExileReturned { source, cards, .. }
                    if *source == cloister && cards == &hand_to_exile
            )
        })
        .expect("source-linked cards return");
    let draw_at = game
        .event_log
        .iter()
        .enumerate()
        .skip(returned_at + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if !hand_to_exile.contains(card))
                .then_some(index)
        })
        .expect("following draw moves another library card to hand");
    println!(
        "Bottled Cloister lifecycle trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(returned_at < draw_at, "return must precede the upkeep draw");
    assert!(
        hand_to_exile
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Hand))
    );
    game.validate_invariants()
        .expect("returned hand state is valid");
}

#[test]
fn cloister_empty_hand_branch_has_no_private_group_or_synthetic_move_receipt() {
    let mut game = rav_game();
    let cloister = game
        .put_on_battlefield(PlayerId(1), "RAV-BOTTLED-CLOISTER")
        .expect("Cloister begins on battlefield");
    for _ in 0..8 {
        game.add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Library)
            .expect("active-player buffer enters setup");
    }

    game.begin_game()
        .expect("game begins at the opponent upkeep");
    assert_eq!(game.step, Step::Upkeep);
    assert_eq!(game.active_player, PlayerId(0));
    assert_eq!(game.stack.len(), 1, "opponent-upkeep trigger stacks");
    pass_pair(&mut game);

    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::HandExiledWithSource { source, .. } if *source == cloister
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == cloister && *ability == "opponent-upkeep-exile-controller-hand-linked"
    )));
    game.validate_invariants()
        .expect("empty-hand trigger leaves no stale private group");
}

#[test]
fn cloister_departure_expires_an_unreturnable_private_hand_group() {
    let mut game = rav_game();
    let cloister = game
        .put_on_battlefield(PlayerId(0), "RAV-BOTTLED-CLOISTER")
        .expect("Cloister begins on battlefield");
    game.add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("controller hand card enters setup");
    let smash = game
        .add_card(PlayerId(1), "RAV-SMASH", Zone::Hand)
        .expect("response spell enters setup");
    let mountain = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("red source enters setup");
    let second_mountain = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("second red source enters setup");
    add_library_buffers(&mut game);

    game.begin_game().expect("game begins");
    pass_pair(&mut game);
    advance_to_upkeep(&mut game, 2, PlayerId(1));
    let exiled_cards = game
        .player(PlayerId(0))
        .expect("controller exists")
        .hand
        .clone();
    pass_pair(&mut game);
    assert!(
        exiled_cards
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Exile))
    );

    game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
        .expect("opponent produces red mana");
    game.activate_mana_ability(PlayerId(1), second_mountain, Color::Red)
        .expect("opponent produces generic payment mana");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: smash,
            targets: vec![Target::Permanent(cloister)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent targets Cloister");
    pass_pair(&mut game);

    println!(
        "Bottled Cloister departure trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(cloister), Some(Zone::Graveyard));
    assert!(
        exiled_cards
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Exile))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LinkedHandExileExpired { source, cards, .. }
            if *source == cloister && cards == &exiled_cards
    )));
    game.validate_invariants()
        .expect("departed source leaves no unreachable private group");
}
