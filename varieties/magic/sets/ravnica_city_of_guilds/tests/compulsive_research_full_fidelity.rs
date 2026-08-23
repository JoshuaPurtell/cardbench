//! Stack, private-choice, and event-log contracts for Compulsive Research.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, RulesError,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_basic_land_type_bindings,
};

fn game() -> Game {
    Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
        .expect("RAV typed basic-land bindings initialize")
}

fn advance_to_first_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first priority pass");
        let second = game.priority;
        game.pass_priority(second).expect("second priority pass");
    }
}

fn open_recipient_choice(
    game: &mut Game,
) -> (
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    Vec<cardbench_magic_engine::ObjectId>,
) {
    let caster = PlayerId(0);
    let recipient = PlayerId(1);
    let research = game
        .add_card(caster, "RAV-COMPULSIVE-RESEARCH", Zone::Hand)
        .expect("Compulsive Research setup");
    let land = game
        .add_card(recipient, "RAV-FOREST", Zone::Hand)
        .expect("recipient land choice");
    let nonlands = ["RAV-WATCHWOLF", "RAV-GLASS-GOLEM"]
        .into_iter()
        .map(|definition| {
            game.add_card(recipient, definition, Zone::Hand)
                .expect("recipient nonland choice")
        })
        .collect::<Vec<_>>();
    for definition in ["RAV-ISLAND", "RAV-PLAINS", "RAV-SWAMP"] {
        game.add_card(recipient, definition, Zone::Library)
            .expect("recipient draw card");
    }
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(caster, "RAV-ISLAND")
                .expect("caster mana source")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins");
    advance_to_first_main(game);
    for island in islands {
        game.activate_mana_ability(caster, island, Color::Blue)
            .expect("cast mana");
    }
    game.cast_spell(
        caster,
        CastRequest {
            card: research,
            targets: vec![Target::Player(recipient)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Compulsive Research casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(recipient)
        .expect("resolution draws and opens recipient decision");
    (research, land, nonlands)
}

#[test]
fn recipient_privately_selects_one_land_after_the_three_draws() {
    let caster = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = game();
    let (research, land, nonlands) = open_recipient_choice(&mut game);
    let recipient_view = game.view_for_player(recipient).expect("recipient view");
    let decision = recipient_view
        .pending_decision
        .expect("recipient-only private decision");

    assert_eq!(decision.kind, DecisionKind::ConditionalPrivateDiscard);
    assert_eq!(decision.min_selections, 1);
    assert_eq!(decision.max_selections, 2);
    assert!(decision.candidates.iter().any(|card| card.id == land));
    assert!(
        decision
            .candidates
            .iter()
            .any(|card| card.id == nonlands[0])
    );
    assert!(
        game.view_for_player(caster)
            .expect("caster view")
            .pending_decision
            .is_none()
    );

    let events_before = game.event_log.clone();
    assert_eq!(
        game.submit_decision(caster, decision.id, DecisionSelection::Objects(vec![land])),
        Err(RulesError::IllegalAction(
            "only the decision player may submit this decision"
        ))
    );
    assert_eq!(game.event_log, events_before, "foreign response is atomic");
    assert_eq!(
        game.submit_decision(
            recipient,
            decision.id,
            DecisionSelection::Objects(vec![nonlands[0]]),
        ),
        Err(RulesError::IllegalAction(
            "conditional private discard requires one land or two distinct hand cards"
        ))
    );
    assert_eq!(game.event_log, events_before, "invalid branch is atomic");

    game.submit_decision(
        recipient,
        decision.id,
        DecisionSelection::Objects(vec![land]),
    )
    .expect("recipient discards one land");
    println!(
        "compulsive_research_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(research), Some(Zone::Graveyard));
    let decision_open = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionOpened {
                    kind: DecisionKind::ConditionalPrivateDiscard,
                    ..
                }
            )
        })
        .expect("private decision opened after draws");
    let drawn_cards = game
        .event_log
        .iter()
        .take(decision_open)
        .filter(|event| matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }))
        .count();
    assert_eq!(drawn_cards, 3, "all draws precede the private choice");
    let discard = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardDiscarded { player, card } if *player == recipient && *card == land))
        .expect("chosen discard receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == research))
        .expect("spell terminal resolution");
    assert!(decision_open < discard && discard < resolved);
    game.validate_invariants()
        .expect("private decision lifecycle preserves engine invariants");
}

#[test]
fn recipient_can_select_two_distinct_nonlands() {
    let recipient = PlayerId(1);
    let mut game = game();
    let (research, _land, nonlands) = open_recipient_choice(&mut game);
    let decision = game
        .view_for_player(recipient)
        .expect("recipient view")
        .pending_decision
        .expect("private decision")
        .id;
    game.submit_decision(
        recipient,
        decision,
        DecisionSelection::Objects(nonlands.clone()),
    )
    .expect("recipient discards two distinct nonlands");
    assert!(
        nonlands
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Graveyard))
    );
    assert_eq!(game.zone_of(research), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("two-card conditional discard preserves engine invariants");
}

#[test]
fn compulsive_research_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-COMPULSIVE-RESEARCH"));
}

#[test]
fn empty_library_recipient_loss_does_not_rollback_the_resolving_spell() {
    let caster = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = game();
    let research = game
        .add_card(caster, "RAV-COMPULSIVE-RESEARCH", Zone::Hand)
        .expect("Compulsive Research setup");
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(caster, "RAV-ISLAND")
                .expect("caster mana source")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins");
    advance_to_first_main(&mut game);
    for island in islands {
        game.activate_mana_ability(caster, island, Color::Blue)
            .expect("cast mana");
    }
    game.cast_spell(
        caster,
        CastRequest {
            card: research,
            targets: vec![Target::Player(recipient)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Compulsive Research casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(recipient)
        .expect("an empty-library loss completes, rather than rolls back, spell resolution");

    println!(
        "compulsive_research_empty_library_event_log={:#?}",
        game.canonical_event_log()
    );
    assert!(
        game.players[recipient.0].lost,
        "recipient lost drawing from empty library"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PlayerLost { player, reason: "attempted to draw from an empty library" }
            if *player == recipient
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == research
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::GameEnded { winner: Some(player) } if *player == caster
    )));
    assert!(game.stack.is_empty(), "terminal loss leaves no stack item");
    assert!(
        game.view_for_player(caster)
            .expect("surviving player view")
            .pending_decision
            .is_none()
    );
    game.validate_invariants()
        .expect("terminal loss branch preserves engine invariants");
}
