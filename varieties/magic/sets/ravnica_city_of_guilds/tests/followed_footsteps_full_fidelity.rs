//! Public attachment, upkeep, and layer-one token-copy contract for Followed Footsteps.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, Step, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_legendary_permanent_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture constructs");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV Aura bindings register");
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .expect("RAV legendary bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player)
            .expect("fixture priority advances toward main");
    }
}

fn advance_to_next_player_upkeep(game: &mut Game, player: PlayerId) {
    for _ in 0..96 {
        if game.active_player == player && game.step == Step::Upkeep {
            return;
        }
        match game.step {
            Step::Draw => {
                let active = game.active_player;
                if game
                    .view_for_player(active)
                    .expect("active-player view")
                    .draw_replacement_pending
                {
                    game.resolve_pending_draw(active, None)
                        .expect("ordinary draw-step replacement resolves");
                } else {
                    // In a two-player game, the starting player skips only
                    // their opening draw (CR 103.8a).
                    pass_pair(game);
                }
            }
            Step::DeclareAttackers => {
                let active = game.active_player;
                game.declare_attackers(active, &[])
                    .expect("empty attacker declaration");
                pass_pair(game);
            }
            Step::DeclareBlockers => {
                let defender = PlayerId(1 - game.active_player.0);
                game.declare_blockers(defender, &[])
                    .expect("empty blocker declaration");
                pass_pair(game);
            }
            _ => pass_pair(game),
        }
    }
    panic!("fixture failed to reach the requested upkeep");
}

#[test]
#[allow(clippy::too_many_lines)] // The complete entry/upkeep/token trace is one public contract.
fn followed_footsteps_triggers_on_its_own_upkeep_not_the_enchanted_creatures() {
    let aura_controller = PlayerId(0);
    let creature_controller = PlayerId(1);
    let mut game = game_with_rav_bindings();
    let creature = game
        .put_on_battlefield(creature_controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    let footsteps = game
        .add_card(aura_controller, "RAV-FOLLOWED-FOOTSTEPS", Zone::Hand)
        .expect("Aura setup");
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..3 {
            game.add_card(player, "RAV-FOREST", Zone::Library)
                .expect("draw-step library setup");
        }
    }
    let islands = (0..5)
        .map(|_| {
            game.put_on_battlefield(aura_controller, "RAV-ISLAND")
                .expect("blue mana source setup")
        })
        .collect::<Vec<_>>();

    advance_to_precombat_main(&mut game);
    for island in islands {
        game.activate_mana_ability(aura_controller, island, Color::Blue)
            .expect("Island produces blue mana");
    }
    game.cast_spell(
        aura_controller,
        CastRequest {
            card: footsteps,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts targeting the opponent's creature");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(footsteps), Some(Zone::Battlefield));
    assert_eq!(
        game.object(footsteps)
            .expect("Aura remains live")
            .attached_to,
        Some(creature)
    );
    assert_eq!(
        game.controller_of(creature).expect("creature controller"),
        creature_controller
    );

    game.clear_event_log();
    advance_to_next_player_upkeep(&mut game, aura_controller);
    let triggers = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { controller, source, ability, .. }
                    if *controller == aura_controller
                        && *source == footsteps
                        && *ability == "aura-controller-upkeep-token-copy"
            )
        })
        .count();
    assert_eq!(
        triggers, 1,
        "the Aura triggers on its own controller's upkeep, not the enchanted creature controller's"
    );
    pass_pair(&mut game);

    let token = game.players[aura_controller.0]
        .battlefield
        .iter()
        .copied()
        .find(|card| {
            *card != creature
                && game
                    .object(*card)
                    .is_ok_and(|object| object.token.is_some())
        })
        .expect("upkeep trigger creates one token under the Aura controller's control");
    assert_eq!(
        game.object(token)
            .expect("token remains live")
            .effective_definition(),
        Some("RAV-WATCHWOLF"),
        "the token retains layer-one definition values rather than the Aura's identity"
    );
    assert_eq!(
        game.controller_of(token).expect("token controller"),
        aura_controller
    );
    assert_eq!(
        game.characteristics(token)
            .expect("token characteristics")
            .power,
        Some(3)
    );
    let created = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::TokenCreated { player, token: created } if *player == aura_controller && *created == token))
        .expect("token creation receipt");
    let copied = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::PermanentCopied { source, target, .. } if *source == creature && *target == token))
        .expect("copiable-value provenance receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == footsteps && *ability == "aura-controller-upkeep-token-copy"))
        .expect("upkeep ability resolution receipt");
    assert!(created < copied && copied < resolved);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-FOLLOWED-FOOTSTEPS"));
    game.validate_invariants()
        .expect("copied-token entry preserves state-machine invariants");
    eprintln!("followed_footsteps_trace={:?}", game.canonical_event_log());
}

#[test]
#[allow(clippy::too_many_lines)] // This keeps the token-copy and no-priority legend SBA trace contiguous.
fn followed_footsteps_copied_legend_requires_controller_retention_choice() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_bindings();
    let original = game
        .put_on_battlefield(controller, "RAV-TOLSIMIR-WOLFBLOOD")
        .expect("legendary creature setup");
    let footsteps = game
        .add_card(controller, "RAV-FOLLOWED-FOOTSTEPS", Zone::Hand)
        .expect("Aura setup");
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..3 {
            game.add_card(player, "RAV-FOREST", Zone::Library)
                .expect("draw-step library setup");
        }
    }
    let islands = (0..5)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-ISLAND")
                .expect("blue mana source setup")
        })
        .collect::<Vec<_>>();

    advance_to_precombat_main(&mut game);
    for island in islands {
        game.activate_mana_ability(controller, island, Color::Blue)
            .expect("Island produces blue mana");
    }
    game.cast_spell(
        controller,
        CastRequest {
            card: footsteps,
            targets: vec![Target::Permanent(original)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts targeting the legendary creature");
    pass_pair(&mut game);

    game.clear_event_log();
    advance_to_next_player_upkeep(&mut game, controller);
    pass_pair(&mut game);
    let copied = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("Followed Footsteps creates the copied legendary token first");
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("legend rule opens a public no-priority retention decision");
    assert_eq!(decision.kind, DecisionKind::LegendRule);
    assert_eq!(decision.min_selections, 1);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(decision.candidates.len(), 2);
    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![original]),
    )
    .expect("controller keeps the original legendary permanent");

    assert_eq!(game.zone_of(original), Some(Zone::Battlefield));
    assert!(
        game.object(copied).is_err(),
        "the unretained token ceases to exist"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::StateBasedAction { card, reason }
            if *card == copied && *reason == "legend rule"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCeasedToExist { token } if *token == copied
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionCompleted { decision: completed, kind, .. }
            if *completed == decision.id && *kind == DecisionKind::LegendRule
    )));
    game.validate_invariants()
        .expect("legend-rule selection closes the copied-token SBA boundary");
    eprintln!(
        "followed_footsteps_legend_trace={:?}",
        game.canonical_event_log()
    );
}
