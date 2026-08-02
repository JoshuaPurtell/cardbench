use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, ManaAbilityActivation, ManaCost, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};

const BOUNCE_LANDS: [(&str, &str, &str); 4] = [
    ("RAV-BOROS-GARRISON", "Boros Garrison", "boros-garrison-rw"),
    ("RAV-DIMIR-AQUEDUCT", "Dimir Aqueduct", "dimir-aqueduct-ub"),
    (
        "RAV-GOLGARI-ROT-FARM",
        "Golgari Rot Farm",
        "golgari-rot-farm-bg",
    ),
    (
        "RAV-SELESNYA-SANCTUARY",
        "Selesnya Sanctuary",
        "selesnya-sanctuary-gw",
    ),
];

#[test]
fn guild_bounce_lands_require_tapped_entry_return_trigger_and_two_mana_binding() {
    let definitions = card_definitions();
    let mana = rav_mana_ability_bindings();
    let triggers = rav_triggered_ability_bindings();
    for (id, name, mana_ability) in BOUNCE_LANDS {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("{name} definition exists"));
        assert_eq!(definition.mana_cost, ManaCost::new(0), "{name}");
        assert_eq!(definition.card_types, [CardType::Land].into(), "{name}");
        assert!(
            mana.iter().any(|binding| {
                binding.card_definition == id && binding.ability.id == mana_ability
            }),
            "{name} has its two-mana activation"
        );
        assert!(
            triggers.iter().any(|binding| {
                binding.card_definition == id && binding.ability.id == "return-controlled-land"
            }),
            "{name} has a stack-backed return trigger"
        );
    }
}

fn bounce_game() -> Game {
    Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .expect("RAV bounce-land fixture constructs")
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("game begins");
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn choose_bounce_target(game: &mut Game, source: cardbench_magic_engine::ObjectId, target: Target) {
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("entry trigger requires a live target choice");
    assert_eq!(choice.source, source);
    assert_eq!(choice.ability, "return-controlled-land");
    assert!(
        choice
            .target_options
            .first()
            .is_some_and(|options| options.contains(&target))
    );
    game.submit_policy_move(
        PlayerId(0),
        "test.guild-bounce-land-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source,
            ability: "return-controlled-land",
            targets: vec![target],
        },
    )
    .expect("controller chooses the return target");
}

#[test]
fn guild_bounce_land_entry_uses_a_real_etb_stack_window() {
    let mut game = bounce_game();
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("setup Plains");
    let garrison = game
        .add_card(PlayerId(0), "RAV-BOROS-GARRISON", Zone::Hand)
        .expect("setup Boros Garrison");

    advance_to_precombat_main(&mut game);
    game.play_land(PlayerId(0), garrison)
        .expect("play Boros Garrison");
    assert_eq!(game.zone_of(garrison), Some(Zone::Battlefield));
    assert!(game.object(garrison).expect("Garrison object").tapped);
    assert_eq!(
        game.stack.len(),
        0,
        "targeted ETB waits for a legal decision"
    );
    choose_bounce_target(&mut game, garrison, Target::Permanent(plains));
    assert_eq!(game.stack.len(), 1, "chosen ETB trigger is on the stack");
    assert_eq!(game.stack[0].targets, vec![Target::Permanent(plains)]);
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == garrison && *ability == "return-controlled-land"
        )
    }));

    game.pass_priority(PlayerId(0)).expect("first pass");
    game.pass_priority(PlayerId(1))
        .expect("second pass resolves ETB");
    assert_eq!(game.zone_of(plains), Some(Zone::Hand));
    assert_eq!(game.zone_of(garrison), Some(Zone::Battlefield));
    assert!(game.object(garrison).expect("Garrison object").tapped);
    println!(
        "guild_bounce_land_entry_event_log={:#?}",
        game.canonical_event_log()
    );
    let trigger_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == garrison && *ability == "return-controlled-land"
            )
        })
        .expect("trigger stack receipt");
    let move_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Hand } if *card == plains
            )
        })
        .expect("controlled-land return receipt");
    let resolved_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == garrison && *ability == "return-controlled-land"
            )
        })
        .expect("trigger resolution receipt");
    assert!(trigger_index < move_index && move_index < resolved_index);
    game.validate_invariants()
        .expect("entry-trigger resolution preserves invariants");
}

#[test]
fn guild_bounce_land_can_return_itself_when_it_is_the_only_land() {
    let mut game = bounce_game();
    let garrison = game
        .add_card(PlayerId(0), "RAV-BOROS-GARRISON", Zone::Hand)
        .expect("setup Boros Garrison");

    advance_to_precombat_main(&mut game);
    game.play_land(PlayerId(0), garrison)
        .expect("play Boros Garrison");
    choose_bounce_target(&mut game, garrison, Target::Permanent(garrison));
    assert_eq!(game.stack[0].targets, vec![Target::Permanent(garrison)]);
    game.pass_priority(PlayerId(0)).expect("first pass");
    game.pass_priority(PlayerId(1))
        .expect("second pass resolves ETB");
    assert_eq!(game.zone_of(garrison), Some(Zone::Hand));
    game.validate_invariants()
        .expect("self-return trigger preserves invariants");
}

#[test]
fn guild_bounce_trigger_never_selects_an_opponent_land() {
    let mut game = bounce_game();
    let opponent_land = game
        .put_on_battlefield(PlayerId(1), "RAV-ISLAND")
        .expect("setup opponent Island");
    let garrison = game
        .add_card(PlayerId(0), "RAV-BOROS-GARRISON", Zone::Hand)
        .expect("setup Boros Garrison");

    advance_to_precombat_main(&mut game);
    game.play_land(PlayerId(0), garrison)
        .expect("play Boros Garrison");
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("entry trigger requires a target choice");
    assert!(choice.target_options[0].contains(&Target::Permanent(garrison)));
    assert!(!choice.target_options[0].contains(&Target::Permanent(opponent_land)));
    game.validate_invariants()
        .expect("controller-relative target selection preserves invariants");
}

#[test]
fn guild_bounce_land_produces_its_fixed_two_color_bundle() {
    let mut game = bounce_game();
    let garrison = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-GARRISON")
        .expect("setup Boros Garrison");

    advance_to_precombat_main(&mut game);
    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: garrison,
            ability_id: "boros-garrison-rw",
            chosen_color: None,
        },
    )
    .expect("activate Boros Garrison");
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player")
            .mana_pool
            .amount(Color::Red),
        1
    );
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player")
            .mana_pool
            .amount(Color::White),
        1
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::BoundManaAbilityFreeBundleActivated { source, ability, .. }
                if *source == garrison && *ability == "boros-garrison-rw"
        )
    }));
    game.validate_invariants()
        .expect("free mana-bundle activation preserves invariants");
}
