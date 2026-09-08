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

// Real-card rules regression, not a benchmark deck or grading workload.
#[test]
fn cloister_commander_return_is_optional_and_precedes_every_linked_move() {
    use cardbench_magic_engine::{DecisionKind, DecisionSelection};
    for accept in [false, true] {
        let owner = PlayerId(0);
        let mut game = rav_game();
        game.configure_commander_format(40, 21).unwrap();
        let cloister = game.put_on_battlefield(owner, "RAV-BOTTLED-CLOISTER").unwrap();
        let commander = game.put_on_battlefield(owner, "RAV-TOLSIMIR-WOLFBLOOD").unwrap();
        game.designate_commander(owner, commander).unwrap();
        let bounce = game.add_card(owner, "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Hand).unwrap();
        let mut mana = Vec::new();
        for (land, color) in [("RAV-ISLAND", Color::Blue), ("RAV-ISLAND", Color::Blue),
            ("RAV-SWAMP", Color::Black), ("RAV-SWAMP", Color::Black)] {
            mana.push((game.put_on_battlefield(owner, land).unwrap(), color));
        }
        let ordinary = game.add_card(owner, "RAV-WATCHWOLF", Zone::Hand).unwrap();
        add_library_buffers(&mut game);
        game.begin_game().unwrap();
        pass_pair(&mut game);
        for (land, color) in mana { game.activate_mana_ability(owner, land, color).unwrap(); }
        game.cast_spell(owner, CastRequest { card: bounce, targets: vec![Target::Permanent(commander)],
            convoke: vec![], payment_mana_abilities: vec![] }).unwrap();
        pass_pair(&mut game);
        let bounce_choice = game.view_for_player(owner).unwrap().pending_decision.unwrap();
        game.submit_decision(owner, bounce_choice.id, DecisionSelection::Objects(vec![])).unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Hand));
        advance_to_upkeep(&mut game, 2, PlayerId(1));
        pass_pair(&mut game);
        let arrival = game.view_for_player(owner).unwrap().pending_decision.unwrap();
        assert_eq!(arrival.kind, DecisionKind::CommanderReturn);
        game.submit_decision(owner, arrival.id, DecisionSelection::Objects(vec![])).unwrap();
        advance_to_upkeep(&mut game, 3, owner);
        let before = game.event_log.len();
        pass_pair(&mut game);
        let replacement = game.view_for_player(owner).unwrap().pending_decision.unwrap();
        assert_eq!(replacement.kind, DecisionKind::CommanderZoneReplacement);
        assert_eq!(game.zone_of(commander), Some(Zone::Exile));
        assert_eq!(game.zone_of(ordinary), Some(Zone::Exile));
        assert!(!game.event_log[before..].iter().any(|event| matches!(event,
            GameEvent::CardMoved { to: Zone::Hand, .. })));
        game.submit_decision(owner, replacement.id,
            DecisionSelection::Objects(if accept { vec![commander] } else { vec![] })).unwrap();
        assert_eq!(game.zone_of(commander), Some(if accept { Zone::Command } else { Zone::Hand }));
        assert_eq!(game.zone_of(ordinary), Some(Zone::Hand));
        let (return_at, returned) = game.event_log.iter().enumerate().skip(before).find_map(|(index, event)| {
            if let GameEvent::LinkedHandExileReturned { source, cards, .. } = event {
                (*source == cloister).then_some((index, cards))
            } else { None }
        }).unwrap();
        assert_eq!(returned.contains(&commander), !accept);
        assert!(returned.contains(&ordinary));
        assert!(game.event_log[return_at + 1..].iter().any(|event| matches!(event,
            GameEvent::CardMoved { card, to: Zone::Hand } if !returned.contains(card))));
        assert!(game.submit_decision(owner, replacement.id, DecisionSelection::Objects(vec![])).is_err());
        game.validate_invariants().unwrap();
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
    let third_mountain = game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN").unwrap();
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
    game.activate_mana_ability(PlayerId(1), third_mountain, Color::Red).unwrap();
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

#[test]
fn cloister_control_change_preserves_prior_owner_hand_returns_and_new_controller_draw() {
    let mut game = rav_game();
    let cloister = game
        .put_on_battlefield(PlayerId(0), "RAV-BOTTLED-CLOISTER")
        .expect("Cloister begins on battlefield");
    game.add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Hand)
        .expect("controller hand card enters setup");
    let leash = game
        .add_card(PlayerId(1), "RAV-DREAM-LEASH", Zone::Hand)
        .expect("effect-created Aura enters setup");
    add_library_buffers(&mut game);

    game.begin_game().expect("game begins");
    pass_pair(&mut game);
    advance_to_upkeep(&mut game, 2, PlayerId(1));
    let prior_controller_cards = game
        .player(PlayerId(0))
        .expect("former controller exists")
        .hand
        .clone();
    pass_pair(&mut game);
    assert!(
        prior_controller_cards
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Exile))
    );

    // This is the engine's public effect-created Aura-entry primitive; it
    // installs Dream Leash's ordinary layer-two control effect without using
    // a post-start setup mutator.
    game.enter_attachment_without_cast(leash, cloister)
        .expect("Dream Leash changes control of Cloister");
    assert_eq!(game.controller_of(cloister), Ok(PlayerId(1)));

    advance_to_upkeep(&mut game, 4, PlayerId(1));
    pass_pair(&mut game);

    println!(
        "Bottled Cloister control-change trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(
        prior_controller_cards
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Hand))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LinkedHandExileReturned { controller, source, cards, .. }
            if *controller == PlayerId(1)
                && *source == cloister
                && prior_controller_cards.iter().all(|card| cards.contains(card))
    )));
    game.validate_invariants()
        .expect("control-changed source preserves linked hand provenance");
}
