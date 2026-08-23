//! Red regression for Woebringer Demon's each-upkeep sacrifice trigger.
//!
//! The creature is controlled by player zero, but the effect belongs to the
//! player whose upkeep began.  The trigger must therefore be placed on the
//! ordinary stack before that upkeep's first priority instead of being
//! approximated as a controller-only static ability.

use cardbench_magic_engine::{DecisionSelection, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

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

fn advance_to_upkeep(game: &mut Game, player: PlayerId) {
    for _ in 0..64 {
        if game.active_player == player && game.step == Step::Upkeep {
            return;
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
    panic!("fixture did not reach requested upkeep");
}

#[test]
fn woebringer_demon_stacks_an_active_player_sacrifice_on_its_controllers_upkeep() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon definition exists");
    let mut game = game();
    let demon = game
        .put_on_battlefield(PlayerId(0), definition.id)
        .expect("demon starts on the battlefield");
    let controller_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("active player has a legal creature to sacrifice");
    let opponent_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("opponent has a legal creature to sacrifice later");
    game.begin_game().expect("fixture enters first upkeep");

    println!("Woebringer red trace: {:#?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == demon && *ability == "each-upkeep-active-player-sacrifice-creature"
    )));
    pass_pair(&mut game);
    let first_decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("resolving trigger opens the active upkeep player's choice");
    assert!(
        game.submit_decision(
            PlayerId(1),
            first_decision.id,
            DecisionSelection::Objects(vec![controller_creature]),
        )
        .is_err(),
        "an opponent may not submit the controller-upkeep sacrifice choice"
    );
    game.submit_decision(
        PlayerId(0),
        first_decision.id,
        DecisionSelection::Objects(vec![controller_creature]),
    )
    .expect("controller submits their own mandatory sacrifice");
    assert_eq!(game.zone_of(controller_creature), Some(Zone::Graveyard));

    advance_to_upkeep(&mut game, PlayerId(1));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == demon && *ability == "each-upkeep-active-player-sacrifice-creature"
    )));
    pass_pair(&mut game);
    let second_decision = game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .expect("opponent upkeep opens the opponent's mandatory sacrifice choice");
    assert!(
        game.submit_decision(
            PlayerId(0),
            second_decision.id,
            DecisionSelection::Objects(vec![opponent_creature]),
        )
        .is_err(),
        "the Demon controller may not submit the opponent-upkeep sacrifice choice"
    );
    game.submit_decision(
        PlayerId(1),
        second_decision.id,
        DecisionSelection::Objects(vec![opponent_creature]),
    )
    .expect("opponent submits their own mandatory sacrifice");
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Graveyard));
    println!("Woebringer green trace: {:#?}", game.canonical_event_log());
    assert!(
        definition
            .supported_rules
            .contains(&"each-upkeep-active-player-sacrifice-creature"),
        "the card definition must not claim that a controller-only sacrifice is enough"
    );
    game.validate_invariants()
        .expect("each-upkeep sacrifice continuations preserve invariants");
}
