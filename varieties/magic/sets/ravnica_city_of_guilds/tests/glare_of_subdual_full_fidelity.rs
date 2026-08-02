//! Full stack and combat contracts for Glare of Subdual.

use cardbench_magic_engine::{
    AbilityActivation, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, "RAV-PLAINS", Zone::Library)
            .expect("library fixture card exists");
    }
}

fn advance_to_opponent_declare_attackers(game: &mut Game) {
    while game.active_player != PlayerId(1) || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

#[test]
fn glare_taps_a_selected_controlled_creature_before_tapping_its_target() {
    let mut game = game_with_rav_bindings();
    let glare = game
        .put_on_battlefield(PlayerId(0), "RAV-GLARE-OF-SUBDUAL")
        .expect("Glare setup");
    let cost_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled cost creature setup");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature setup");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: glare,
            ability_id: "tap-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![cost_creature],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Glare tap ability stacks");
    assert!(game.object(cost_creature).expect("cost creature remains").tapped);
    assert!(!game.object(glare).expect("Glare remains").tapped);
    assert!(!game.object(target).expect("target remains").tapped);

    pass_pair(&mut game);
    assert!(game.object(target).expect("target remains").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AdditionalCreatureTappedAsAbilityCost { source, permanent, .. }
            if *source == glare && *permanent == cost_creature
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentTapped { source, card } if *source == glare && *card == target
    )));
    eprintln!("glare_target_tap_trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Glare target-tap trace preserves invariants");
}

#[test]
#[allow(clippy::too_many_lines)] // The complete combat window validates the global replacement lifetime.
fn glare_prevents_all_combat_damage_after_its_selected_creature_cost() {
    let mut game = game_with_rav_bindings();
    add_opening_library(&mut game, PlayerId(0));
    add_opening_library(&mut game, PlayerId(1));
    let glare = game
        .put_on_battlefield(PlayerId(0), "RAV-GLARE-OF-SUBDUAL")
        .expect("Glare setup");
    let cost_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled cost creature setup");
    let attacker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("attacker setup");
    for creature in [cost_creature, attacker] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("fixture creature predates measured turn");
    }
    game.begin_game().expect("game starts");
    advance_to_opponent_declare_attackers(&mut game);
    game.clear_event_log();

    game.declare_attackers(PlayerId(1), &[attacker])
        .expect("attacker declares");
    game.pass_priority(PlayerId(1))
        .expect("attacker controller passes to Glare controller");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: glare,
            ability_id: "prevent-all-combat-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![cost_creature],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Glare prevention ability stacks");
    assert!(game.object(cost_creature).expect("cost creature remains").tapped);
    assert!(!game.object(glare).expect("Glare remains").tapped);

    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::GlobalCombatDamagePreventionCreated { source, expires_turn }
            if *source == glare && *expires_turn == game.turn
    )));
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(0), &[])
        .expect("no blockers declared");
    pass_pair(&mut game);

    assert_eq!(game.player(PlayerId(0)).expect("player remains").life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePrevented {
            source,
            prevented_by,
            target: Target::Player(player),
            amount: 3,
        } if *source == attacker && *prevented_by == glare && *player == PlayerId(0)
    )));
    while game.turn == 2 {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::GlobalCombatDamagePreventionExpired { source } if *source == glare
    )));
    eprintln!("glare_global_prevention_trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Glare global combat-prevention trace preserves invariants");
}
