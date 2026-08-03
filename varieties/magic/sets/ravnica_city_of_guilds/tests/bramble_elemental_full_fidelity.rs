//! Green contract for Bramble Elemental's optional Aura-entry token trigger.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player)
            .expect("fixture priority passes toward main");
    }
    assert_eq!(game.priority, PlayerId(0));
}

fn advance_to_precombat_main_for(game: &mut Game, player: PlayerId) {
    game.begin_game().expect("fixture begins game");
    while game.active_player != player || game.step != Step::PrecombatMain {
        if game.step == Step::Draw
            && game
                .view_for_player(game.active_player)
                .expect("active-player view")
                .draw_replacement_pending
        {
            game.resolve_pending_draw(game.active_player, None)
                .expect("fixture resolves ordinary draw");
            continue;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.active_player)
                .expect("active-player view")
                .attackers_declared
        {
            game.declare_attackers(game.active_player, &[])
                .expect("fixture declares no attackers");
            continue;
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.active_player)
                .expect("active-player view")
                .blockers_declared
        {
            let defender = PlayerId((game.active_player.0 + 1) % 2);
            game.declare_blockers(defender, &[])
                .expect("fixture declares no blockers");
            continue;
        }
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("fixture priority passes toward requested main");
    }
    assert_eq!(game.priority, player);
}

fn aura_entry_fixture() -> (Game, cardbench_magic_engine::ObjectId) {
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
    let bramble = game
        .put_on_battlefield(PlayerId(0), "RAV-BRAMBLE-ELEMENTAL")
        .expect("Bramble setup");
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("Aura target setup");
    let aura = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Aura hand setup");
    let forests = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-FOREST")
                .expect("Forest setup")
        })
        .collect::<Vec<_>>();
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, "RAV-WATCHWOLF", Zone::Library)
            .expect("draw fixture setup");
    }
    advance_to_first_main(&mut game);
    for forest in forests {
        game.activate_mana_ability(PlayerId(0), forest, Color::Green)
            .expect("Forest pays Moldervine Cloak");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Moldervine Cloak casts at controlled creature");
    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == bramble && *ability == "controlled-aura-enters-create-saproling"
    )));
    (game, bramble)
}

#[test]
fn bramble_elemental_accepts_its_controller_aura_entry_and_creates_one_saproling() {
    let (mut game, bramble) = aura_entry_fixture();
    pass_pair(&mut game);
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("optional trigger choice is controller-visible");
    assert_eq!(choice.source, bramble);
    assert_eq!(choice.ability, "controlled-aura-enters-create-saproling");
    assert!(choice.can_pay, "zero-mana may-trigger is always acceptable");
    assert!(choice.conditional_targets.is_empty());
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .optional_triggered_ability_choice
            .is_none(),
        "the opponent has no actionable choice"
    );

    game.submit_policy_move(
        PlayerId(0),
        "bramble-elemental-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: bramble,
            ability: "controlled-aura-enters-create-saproling",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts Bramble trigger");
    let saprolings = game.players[PlayerId(0).0]
        .battlefield
        .iter()
        .filter(|card| {
            game.object(**card)
                .expect("battlefield object")
                .token
                .as_ref()
                .is_some_and(|token| token == &cardbench_magic_engine::TokenSpec::saproling())
        })
        .count();
    assert_eq!(saprolings, 1);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("accepted Aura-entry trigger preserves engine invariants");
    eprintln!("Bramble accept trace={:?}", game.canonical_event_log());
}

#[test]
fn bramble_elemental_decline_creates_no_token() {
    let (mut game, bramble) = aura_entry_fixture();
    pass_pair(&mut game);
    game.submit_policy_move(
        PlayerId(0),
        "bramble-elemental-test.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: bramble,
            ability: "controlled-aura-enters-create-saproling",
            pay: false,
            target: None,
        },
    )
    .expect("controller declines Bramble trigger");
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("declined Aura-entry trigger preserves engine invariants");
    eprintln!("Bramble decline trace={:?}", game.canonical_event_log());
}

#[test]
fn bramble_elemental_does_not_observe_an_opponents_aura_entry() {
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
    let bramble = game
        .put_on_battlefield(PlayerId(0), "RAV-BRAMBLE-ELEMENTAL")
        .expect("Bramble setup");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent Aura target setup");
    let aura = game
        .add_card(PlayerId(1), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("opponent Aura hand setup");
    let forests = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-FOREST")
                .expect("opponent Forest setup")
        })
        .collect::<Vec<_>>();
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, "RAV-WATCHWOLF", Zone::Library)
            .expect("draw fixture setup");
    }
    advance_to_precombat_main_for(&mut game, PlayerId(1));
    for forest in forests {
        game.activate_mana_ability(PlayerId(1), forest, Color::Green)
            .expect("opponent Forest pays Moldervine Cloak");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent Aura casts");
    pass_pair(&mut game);

    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == bramble && *ability == "controlled-aura-enters-create-saproling"
    )));
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("opponent Aura entry preserves controller-scoped trigger invariants");
}
