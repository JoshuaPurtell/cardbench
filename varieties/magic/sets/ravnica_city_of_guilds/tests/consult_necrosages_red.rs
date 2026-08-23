//! Compatibility contract for Consult the Necrosages' cast-selected modes.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game, GameEvent,
    ManaCost, PlayerId, PolicyAction, PolicyMoveKind, RulesError, Step, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins");
    while game.step != Step::PrecombatMain {
        if game
            .view_for_player(game.active_player)
            .expect("active player view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(game.active_player, None)
                .expect("ordinary draw resolves");
        } else {
            pass_pair(game);
        }
    }
}

fn cast_request(card: cardbench_magic_engine::ObjectId, target: PlayerId) -> CastRequest {
    CastRequest {
        card,
        targets: vec![Target::Player(target)],
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn give_consult_mana(game: &mut Game) {
    for color in [Color::Blue, Color::Black, Color::Colorless] {
        game.add_mana_from_action(PlayerId(0), color, 1)
            .expect("Consult payment mana");
    }
}

#[test]
fn consult_the_necrosages_requires_a_cast_selected_modal_target_player_effect() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONSULT-THE-NECROSAGES")
        .expect("Consult the Necrosages definition exists");

    assert_eq!(definition.name, "Consult the Necrosages");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Black])
    );
    assert_eq!(
        definition.card_types,
        std::collections::BTreeSet::from([CardType::Sorcery])
    );
    assert_eq!(
        definition.effects,
        [Effect::ChooseOneOf(vec![
            vec![Effect::DrawTargetPlayerCards { count: 2 }],
            vec![Effect::DiscardTargetPlayer { count: 2 }],
        ])]
    );
    assert!(
        definition
            .supported_rules
            .contains(&"caster-selected-modal-target-player-draw-or-discard")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"target-player-discard-two-private-recipient-selection")
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the target-player discard branch is full fidelity with its recipient-private card selection"
    );
}

#[test]
fn consult_requires_a_policy_submitted_branch_and_rejects_an_out_of_range_mode() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let consult = game
        .add_card(PlayerId(0), "RAV-CONSULT-THE-NECROSAGES", Zone::Hand)
        .expect("Consult begins in hand");
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("opening draw card exists");
    advance_to_precombat_main(&mut game);
    give_consult_mana(&mut game);

    let before_events = game.event_log.clone();
    let before_pool = game
        .player(PlayerId(0))
        .expect("caster exists")
        .mana_pool
        .clone();
    assert_eq!(
        game.cast_spell(PlayerId(0), cast_request(consult, PlayerId(1))),
        Err(RulesError::IllegalAction(
            "a modal spell requires one explicit selected branch"
        ))
    );
    assert_eq!(game.zone_of(consult), Some(Zone::Hand));
    assert_eq!(
        game.player(PlayerId(0)).expect("caster exists").mana_pool,
        before_pool
    );
    assert_eq!(game.event_log, before_events);

    assert_eq!(
        game.cast_spell_with_mode(PlayerId(0), cast_request(consult, PlayerId(1)), 2),
        Err(RulesError::IllegalAction(
            "the selected modal branch is outside the card definition"
        ))
    );
    assert_eq!(game.zone_of(consult), Some(Zone::Hand));
    assert_eq!(game.event_log, before_events);
}

#[test]
#[allow(clippy::too_many_lines)] // Both policy-selected modal branches must retain their full event transcripts.
fn consult_policy_draw_and_discard_branches_retain_stack_and_event_provenance() {
    let mut draw_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let consult = draw_game
        .add_card(PlayerId(0), "RAV-CONSULT-THE-NECROSAGES", Zone::Hand)
        .expect("Consult begins in hand");
    draw_game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("opening draw card exists");
    let drawn_one = draw_game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("target library bottom");
    let drawn_two = draw_game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("target library top");
    advance_to_precombat_main(&mut draw_game);
    give_consult_mana(&mut draw_game);
    draw_game
        .submit_policy_move(
            PlayerId(0),
            "test.consult-necrosages.draw.v1",
            PolicyAction::CastWithMode {
                request: cast_request(consult, PlayerId(1)),
                mode: 0,
            },
        )
        .expect("policy submits the draw mode");
    assert_eq!(draw_game.stack.len(), 1);
    assert_eq!(draw_game.stack[0].chosen_modal_mode, Some(0));
    assert_eq!(
        draw_game.stack[0].effects,
        vec![Effect::DrawTargetPlayerCards { count: 2 }]
    );
    pass_pair(&mut draw_game);
    assert_eq!(draw_game.zone_of(consult), Some(Zone::Graveyard));
    assert_eq!(
        draw_game.players[1].hand,
        vec![drawn_two, drawn_one],
        "two ordinary draws preserve library-top order"
    );
    assert!(draw_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            kind: PolicyMoveKind::CastWithMode,
            ..
        }
    )));
    assert!(draw_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellModeChosen { card, mode: 0, .. } if *card == consult
    )));

    let mut discard_game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let consult = discard_game
        .add_card(PlayerId(0), "RAV-CONSULT-THE-NECROSAGES", Zone::Hand)
        .expect("Consult begins in hand");
    discard_game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("opening draw card exists");
    let first = discard_game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Hand)
        .expect("first target hand card");
    let second = discard_game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .expect("second target hand card");
    let retained = discard_game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Hand)
        .expect("third target hand card");
    advance_to_precombat_main(&mut discard_game);
    give_consult_mana(&mut discard_game);
    discard_game
        .submit_policy_move(
            PlayerId(0),
            "test.consult-necrosages.discard.v1",
            PolicyAction::CastWithMode {
                request: cast_request(consult, PlayerId(1)),
                mode: 1,
            },
        )
        .expect("policy submits the discard mode");
    assert_eq!(discard_game.stack[0].chosen_modal_mode, Some(1));
    assert_eq!(
        discard_game.stack[0].effects,
        vec![Effect::DiscardTargetPlayer { count: 2 }]
    );
    pass_pair(&mut discard_game);
    let decision = discard_game
        .view_for_player(PlayerId(1))
        .expect("target recipient view")
        .pending_decision
        .expect("discard target receives private hand selection");
    assert_eq!(decision.kind, DecisionKind::ConditionalPrivateDiscard);
    discard_game
        .submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::Objects(vec![first, second]),
        )
        .expect("target selects the two cards to discard");
    assert_eq!(discard_game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(discard_game.zone_of(second), Some(Zone::Graveyard));
    assert_eq!(discard_game.players[1].hand, vec![retained]);
    assert!(discard_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellModeChosen { card, mode: 1, .. } if *card == consult
    )));
    assert!(discard_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened {
            player: PlayerId(1),
            kind: DecisionKind::ConditionalPrivateDiscard,
            ..
        }
    )));
    println!("consult_draw_trace={:#?}", draw_game.canonical_event_log());
    println!(
        "consult_discard_trace={:#?}",
        discard_game.canonical_event_log()
    );
    draw_game
        .validate_invariants()
        .expect("draw branch preserves modal stack provenance");
    discard_game
        .validate_invariants()
        .expect("discard branch preserves modal stack provenance");
}
