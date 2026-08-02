//! Full-fidelity regression coverage for Stoneshaker Shaman's end-step trigger.

use cardbench_magic_engine::{Color, DecisionSelection, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

const ABILITY: &str = "each-end-step-active-player-sacrifice-untapped-land";

fn game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_end_step(game: &mut Game, player: PlayerId) {
    for _ in 0..96 {
        if game.active_player == player && game.step == Step::End {
            return;
        }
        let next_player = game.next_policy_player();
        if game
            .view_for_player(next_player)
            .expect("public game view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(next_player, None)
                .expect("ordinary draw is legal");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("active-player view")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attackers are legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(PlayerId(1 - game.active_player.0))
                    .expect("defending-player view")
                    .blockers_declared =>
            {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty blockers are legal");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority)
                    .expect("ordinary priority advances the turn");
            }
        }
    }
    panic!("fixture did not reach requested end step");
}

#[test]
fn stoneshaker_shaman_captures_each_end_step_player_and_offers_only_untapped_lands() {
    let mut game = game();
    let shaman = game
        .put_on_battlefield(PlayerId(0), "RAV-STONESHAKER-SHAMAN")
        .expect("Shaman starts on the battlefield");
    let controller_untapped_land = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("controller has an untapped land");
    let controller_tapped_land = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("controller has a tapped land");
    let opponent_untapped_land = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("opponent has an untapped land");
    game.add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Library)
        .expect("opponent library supports its ordinary draw step");
    game.begin_game().expect("fixture begins");
    game.activate_mana_ability(PlayerId(0), controller_tapped_land, Color::Red)
        .expect("controller spends the land before their end step");

    advance_to_end_step(&mut game, PlayerId(0));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == shaman && *ability == ABILITY
    )));
    pass_pair(&mut game);
    let first_decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("end-step trigger opens the active player's choice");
    assert_eq!(
        first_decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![controller_untapped_land],
        "tapped and opponent lands must never enter the decision boundary"
    );
    assert!(
        game.submit_decision(
            PlayerId(1),
            first_decision.id,
            DecisionSelection::Objects(vec![controller_untapped_land]),
        )
        .is_err()
    );
    assert!(
        game.submit_decision(
            PlayerId(0),
            first_decision.id,
            DecisionSelection::Objects(vec![controller_tapped_land]),
        )
        .is_err()
    );
    game.submit_decision(
        PlayerId(0),
        first_decision.id,
        DecisionSelection::Objects(vec![controller_untapped_land]),
    )
    .expect("active end-step player sacrifices their own untapped land");
    assert_eq!(
        game.zone_of(controller_untapped_land),
        Some(Zone::Graveyard)
    );
    assert_eq!(
        game.zone_of(controller_tapped_land),
        Some(Zone::Battlefield)
    );

    advance_to_end_step(&mut game, PlayerId(1));
    pass_pair(&mut game);
    let second_decision = game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .expect("opponent end step opens that opponent's choice");
    assert_eq!(
        second_decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![opponent_untapped_land]
    );
    assert!(
        game.submit_decision(
            PlayerId(0),
            second_decision.id,
            DecisionSelection::Objects(vec![opponent_untapped_land]),
        )
        .is_err()
    );
    game.submit_decision(
        PlayerId(1),
        second_decision.id,
        DecisionSelection::Objects(vec![opponent_untapped_land]),
    )
    .expect("captured opponent end-step player sacrifices their own untapped land");
    assert_eq!(game.zone_of(opponent_untapped_land), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, player, permanent }
            if *source == shaman && *player == PlayerId(1) && *permanent == opponent_untapped_land
    )));
    println!(
        "Stoneshaker Shaman trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("captured end-step land sacrifices preserve invariants");
}

#[test]
fn stoneshaker_shaman_noops_when_the_active_player_has_no_untapped_land() {
    let mut game = game();
    let shaman = game
        .put_on_battlefield(PlayerId(0), "RAV-STONESHAKER-SHAMAN")
        .expect("Shaman starts on the battlefield");
    let tapped_land = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("controller has only a tapped land");
    game.begin_game().expect("fixture begins");
    game.activate_mana_ability(PlayerId(0), tapped_land, Color::Red)
        .expect("controller spends their only land before the end step");

    advance_to_end_step(&mut game, PlayerId(0));
    pass_pair(&mut game);
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_none(),
        "no legal untapped land must not fabricate a choice"
    );
    assert_eq!(game.zone_of(tapped_land), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == shaman && *ability == ABILITY
    )));
    game.validate_invariants()
        .expect("end-step no-op preserves the state-machine invariants");
}
