//! Full-fidelity contract for Ethereal Usher's two independent activations.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, DecisionKind, DecisionSelection, Game,
    GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn ethereal_usher_definition_has_exact_static_and_activated_scope() {
    let usher = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ETHEREAL-USHER")
        .expect("Ethereal Usher definition exists");

    assert_eq!(usher.name, "Ethereal Usher");
    assert_eq!(usher.mana_cost, ManaCost::with_colors(5, [Color::Blue]));
    assert_eq!(usher.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(usher.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((usher.power, usher.toughness), (Some(2), Some(3)));
    assert_eq!(
        usher.keywords,
        [Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))]
    );
    assert_eq!(
        usher.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-target-unblockable-until-end-of-turn",
            "transmute",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&usher.id));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-ETHEREAL-USHER"
            && binding.ability.id == "tap-target-unblockable"
    }));
}

#[test]
fn ethereal_usher_tap_ability_leaves_its_target_unblockable_this_turn() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let usher = game
        .put_on_battlefield(player, "RAV-ETHEREAL-USHER")
        .expect("Usher enters");
    let attacker = game
        .put_on_battlefield(player, "RAV-WATCHWOLF")
        .expect("attack target enters");
    let blocker = game
        .put_on_battlefield(opponent, "RAV-COURIER-HAWK")
        .expect("blocker enters");
    let island = game
        .put_on_battlefield(player, "RAV-ISLAND")
        .expect("Island enters");
    for object in [usher, attacker, blocker, island] {
        game.set_entered_turn_for_setup(object, 0)
            .expect("fixture objects have prior-turn provenance");
    }
    game.begin_game().expect("game begins");
    for player in [player, opponent, player, opponent] {
        game.pass_priority(player)
            .expect("advance to precombat main");
    }
    game.activate_mana_ability(player, island, Color::Blue)
        .expect("activation mana");
    game.activate_ability(
        player,
        AbilityActivation {
            source: usher,
            ability_id: "tap-target-unblockable",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(attacker)],
        },
    )
    .expect("Usher targets the attacker");
    game.pass_priority(player).expect("controller passes");
    game.pass_priority(opponent).expect("ability resolves");

    assert!(game.object(usher).expect("Usher remains live").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == usher && *target == attacker
    )));

    for player in [player, opponent, player, opponent] {
        game.pass_priority(player)
            .expect("advance through beginning of combat");
    }
    game.declare_attackers(player, &[attacker])
        .expect("the target attacks");
    game.pass_priority(player).expect("attacker passes");
    game.pass_priority(opponent).expect("enter blockers");
    assert!(
        game.declare_blockers(opponent, &[CombatBlock { attacker, blocker }],)
            .is_err(),
        "the chosen creature cannot be blocked this turn"
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
    println!(
        "ethereal_usher_evasion_trace={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("unblockable declaration rejection preserves invariants");
}

#[test]
fn ethereal_usher_transmute_uses_the_stack_backed_private_library_search() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV typed game builds");
    let usher = game
        .add_card(player, "RAV-ETHEREAL-USHER", Zone::Hand)
        .expect("Usher begins in hand");
    let matching_value = game
        .add_card(player, "RAV-ETHEREAL-USHER", Zone::Library)
        .expect("matching library card");
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(player, "RAV-ISLAND")
                .expect("Transmute mana source")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("advance first priority");
        let second = game.priority;
        game.pass_priority(second).expect("advance second priority");
    }
    for island in islands {
        game.activate_mana_ability(player, island, Color::Blue)
            .expect("Transmute mana");
    }

    game.activate_transmute(player, usher)
        .expect("stack-backed Transmute activates");
    game.pass_priority(player).expect("controller passes");
    game.pass_priority(opponent)
        .expect("resolution opens the private library decision");
    let decision = game
        .view_for_player(player)
        .expect("controller view")
        .pending_decision
        .expect("private Transmute choice");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert!(
        game.view_for_player(opponent)
            .expect("opponent view")
            .pending_decision
            .is_none()
    );
    game.submit_decision(
        player,
        decision.id,
        DecisionSelection::Objects(vec![matching_value]),
    )
    .expect("controller selects matching mana value");

    println!(
        "ethereal_usher_transmute_trace={:#?}",
        game.canonical_event_log()
    );
    assert!(game.stack.is_empty());
    assert_eq!(game.zone_of(usher), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(matching_value), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { discarded, found: Some(found), .. }
            if *discarded == usher && *found == matching_value
    )));
    game.validate_invariants()
        .expect("stack-backed Transmute preserves invariants");
}
