//! Event-log contracts for Quickchange's selected color replacement and draw.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_one_policy_action(game: &mut Game) {
    let active = game.active_player;
    if game
        .view_for_player(active)
        .expect("active player view")
        .draw_replacement_pending
    {
        game.draw_card(active, None)
            .expect("ordinary draw resolves before priority");
        return;
    }
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(active)
            .expect("active player combat view")
            .attackers_declared
    {
        game.declare_attackers(active, &[])
            .expect("empty attacker declaration");
        return;
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("defender combat view")
            .blockers_declared
    {
        let defender = game.next_policy_player();
        game.declare_blockers(defender, &[])
            .expect("empty blocker declaration");
        return;
    }
    let player = game.priority;
    game.pass_priority(player)
        .expect("priority holder advances turn structure");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..64 {
        if game.turn == turn && game.step == step {
            return;
        }
        advance_one_policy_action(game);
        game.validate_invariants()
            .expect("ordinary turn transition preserves invariants");
    }
    panic!(
        "turn machine did not reach turn {turn} step {step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

#[test]
fn quickchange_replaces_all_target_colors_then_draws_and_expires_at_cleanup() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let target = game
        .put_on_battlefield(opponent, "RAV-WATCHWOLF")
        .expect("multicolor creature target setup");
    let quickchange = game
        .add_card(caster, "RAV-QUICKCHANGE", Zone::Hand)
        .expect("Quickchange setup");
    let drawn = game
        .add_card(caster, "RAV-PLAINS", Zone::Library)
        .expect("draw card setup");
    let island = game
        .put_on_battlefield(caster, "RAV-ISLAND")
        .expect("blue source setup");
    let printed_cost_generic = game.put_on_battlefield(caster, "RAV-ISLAND").unwrap();

    game.begin_game().expect("game begins");
    advance_to(&mut game, 1, Step::PrecombatMain);
    game.activate_mana_ability(caster, island, Color::Blue)
        .expect("blue mana payment");
    game.activate_mana_ability(caster, printed_cost_generic, Color::Blue).unwrap();
    game.submit_policy_move(
        caster,
        "rav-quickchange-full-fidelity.v1",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: quickchange,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Red,
        },
    )
    .expect("policy-selected red color cast");
    pass_pair(&mut game);

    assert_eq!(
        game.characteristics(target)
            .expect("target survives")
            .colors,
        BTreeSet::from([Color::Red]),
        "the selected color replaces rather than adds to Watchwolf's colors"
    );
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    let created = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, target: effect_target, .. }
                    if *source == quickchange && *effect_target == target
            )
        })
        .expect("layer-five color replacement receipt");
    let drawn_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn
            )
        })
        .expect("controller draw receipt");
    let resolved = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::SpellResolved { card } if *card == quickchange),
        )
        .expect("Quickchange resolution receipt");
    assert!(
        created < drawn_index && drawn_index < resolved,
        "color replacement must precede the ordinary controller draw and terminal resolution receipt"
    );

    advance_to(&mut game, 2, Step::Upkeep);
    assert_eq!(
        game.characteristics(target).expect("target remains").colors,
        BTreeSet::from([Color::Green, Color::White]),
        "the original multicolor characteristics return after end-of-turn cleanup"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target: effect_target, layer }
            if *source == quickchange && *effect_target == target && *layer == cardbench_magic_engine::Layer::Color
    )));
    println!("quickchange_event_log={:#?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Quickchange trace preserves stack, choice, layer, draw, and cleanup invariants");
}

#[test]
fn quickchange_rejects_colorless_choice_without_mutating_the_cast_transaction() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target setup");
    let quickchange = game
        .add_card(PlayerId(0), "RAV-QUICKCHANGE", Zone::Hand)
        .expect("Quickchange setup");
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("pre-game mana setup");
    let before_events = game.event_log.clone();

    game.submit_policy_move(
        PlayerId(0),
        "rav-quickchange-invalid-color.v1",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: quickchange,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Colorless,
        },
    )
    .expect_err("colorless cannot be submitted as a card-color choice");

    assert_eq!(game.zone_of(quickchange), Some(Zone::Hand));
    assert_eq!(game.stack.len(), 0, "rejected cast never reaches the stack");
    assert_eq!(
        game.player(PlayerId(0))
            .expect("caster remains")
            .mana_pool
            .amount(Color::Blue),
        2,
        "invalid color choice cannot consume payment mana"
    );
    assert_eq!(
        game.characteristics(target).expect("target remains").colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(game.event_log, before_events, "rejection writes no receipt");
    game.validate_invariants()
        .expect("rejected Quickchange choice preserves invariants");
}

#[test]
fn quickchange_with_a_stale_creature_target_is_countered_by_rules_without_a_draw_or_layer_effect() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let target = game
        .put_on_battlefield(opponent, "RAV-WATCHWOLF")
        .expect("target setup");
    let quickchange = game
        .add_card(caster, "RAV-QUICKCHANGE", Zone::Hand)
        .expect("Quickchange setup");
    let removal = game
        .add_card(opponent, "RAV-LAST-GASP", Zone::Hand)
        .expect("response setup");
    let undrawn = game
        .add_card(caster, "RAV-PLAINS", Zone::Library)
        .expect("would-be draw setup");
    game.grant_mana(caster, Color::Blue, 2)
        .expect("Quickchange mana setup");
    game.grant_mana(opponent, Color::Black, 2)
        .expect("response mana setup");

    game.submit_policy_move(
        caster,
        "rav-quickchange-stale-target.v1",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: quickchange,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Red,
        },
    )
    .expect("Quickchange cast");
    game.pass_priority(caster)
        .expect("caster gives opponent the response window");
    game.cast_spell(
        opponent,
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response removes the target");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    pass_pair(&mut game);

    assert_eq!(game.zone_of(quickchange), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(undrawn), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCounteredByRules { card } if *card == quickchange
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, .. } if *source == quickchange
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Hand } if *card == undrawn
    )));
    println!(
        "quickchange_stale_target_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("stale Quickchange target preserves stack and zone invariants");
}

#[test]
fn quickchange_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-QUICKCHANGE"));
}

#[test]
fn quickchange_public_scenario_exposes_the_selected_color_and_draw_receipts() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_quickchange_color_replacement")
        .expect("Quickchange public scenario exists");
    assert_eq!(trace.digest, "fnv1a64:c25b45b7a2757fb0");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("SpellColorChosen") && event.contains("Red"))
    );
    assert!(
        trace.event_log.iter().any(
            |event| event.contains("ContinuousEffectCreated") && event.contains("layer: Color")
        )
    );
    println!(
        "quickchange_public_scenario_event_log={:#?}",
        trace.event_log
    );
}
