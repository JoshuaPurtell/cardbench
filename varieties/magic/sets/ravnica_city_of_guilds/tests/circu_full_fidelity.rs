//! Full-fidelity behavioral contracts for Circu's colored-cast triggers.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastPermissionPayment, CastRequest, CastTiming, Color, DecisionKind,
    DecisionSelection, Effect, Game, GameEvent, ManaCost, PlayerId, RulesError, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

fn test_instant(
    id: &'static str,
    name: &'static str,
    colors: BTreeSet<Color>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["test-fixture"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn game_with_circu_and_fixture_spells() -> Game {
    let mut definitions = card_definitions()
        .into_iter()
        .filter(|definition| definition.id == "RAV-CIRCU-DIMIR-LOBOTOMIST")
        .collect::<Vec<_>>();
    definitions.extend([
        test_instant(
            "TST-BLUE-RELAY",
            "fixture blue instant",
            BTreeSet::from([Color::Blue]),
            vec![Effect::GainLifeController { amount: 1 }],
        ),
        test_instant(
            "TST-BLACK-RELAY",
            "fixture black instant",
            BTreeSet::from([Color::Black]),
            vec![Effect::GainLifeController { amount: 1 }],
        ),
        test_instant(
            "TST-BLUE-BLACK-RELAY",
            "fixture blue black instant",
            BTreeSet::from([Color::Blue, Color::Black]),
            vec![Effect::GainLifeController { amount: 1 }],
        ),
        test_instant(
            "TST-EXILED-RELAY",
            "fixture exiled instant",
            BTreeSet::from([Color::Black]),
            vec![Effect::GainLifeController { amount: 1 }],
        ),
        test_instant(
            "TST-EXILE-PERMISSION",
            "fixture exile permission",
            BTreeSet::from([Color::Blue]),
            vec![Effect::GrantExileCastPermissionUntilEndOfTurn {
                payment: CastPermissionPayment::PayManaCost,
                timing: CastTiming::Normal,
            }],
        ),
    ]);
    let triggers: Vec<_> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == "RAV-CIRCU-DIMIR-LOBOTOMIST")
        .collect();
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        vec![],
        vec![],
        vec![],
        vec![],
        triggers,
    )
    .expect("Circu fixture builds")
}

fn cast(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    targets: Vec<Target>,
) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("fixture spell casts");
}

fn choose_opponent_for_trigger(game: &mut Game) {
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Circu trigger requires a target choice");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityTargets);
    assert_eq!(
        decision.target_candidates,
        vec![Target::Player(PlayerId(1))]
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Player(PlayerId(1))]),
    )
    .expect("controller chooses the opponent");
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn circu_colored_casts_exile_the_opponents_top_card_and_block_permission_casts() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game_with_circu_and_fixture_spells();
    let circu = game
        .put_on_battlefield(controller, "RAV-CIRCU-DIMIR-LOBOTOMIST")
        .expect("Circu setup");
    let blue = game
        .add_card(controller, "TST-BLUE-RELAY", Zone::Hand)
        .expect("blue spell setup");
    let black = game
        .add_card(controller, "TST-BLACK-RELAY", Zone::Hand)
        .expect("black spell setup");
    let multicolor = game
        .add_card(controller, "TST-BLUE-BLACK-RELAY", Zone::Hand)
        .expect("multicolor spell setup");
    let black_exiled = game
        .add_card(opponent, "TST-EXILED-RELAY", Zone::Library)
        .expect("lower library card setup");
    let blue_exiled = game
        .add_card(opponent, "TST-EXILED-RELAY", Zone::Library)
        .expect("top library card setup");
    let permission = game
        .add_card(opponent, "TST-EXILE-PERMISSION", Zone::Hand)
        .expect("permission spell setup");
    game.begin_game().expect("fixture begins");

    cast(&mut game, controller, blue, vec![]);
    choose_opponent_for_trigger(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.zone_of(blue_exiled), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == circu && *ability == "controller-casts-blue-spell-exile-opponent-library-top"
    )));

    game.pass_priority(controller)
        .expect("controller passes for the permission response");
    cast(
        &mut game,
        opponent,
        permission,
        vec![Target::Permanent(blue_exiled)],
    );
    pass_pair(&mut game);
    game.pass_priority(controller)
        .expect("controller passes to the permitted opponent");
    let restricted_cast = game.cast_spell(
        opponent,
        CastRequest {
            card: blue_exiled,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(
        matches!(
            restricted_cast,
            Err(RulesError::IllegalAction(
                "an opponent-owned card exiled by a live source cannot be cast"
            ))
        ),
        "unexpected exile cast result: {restricted_cast:?}"
    );

    game.pass_priority(opponent)
        .expect("opponent returns priority after the rejected cast");
    cast(&mut game, controller, black, vec![]);
    choose_opponent_for_trigger(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.zone_of(black_exiled), Some(Zone::Exile));

    cast(&mut game, controller, multicolor, vec![]);
    let ordering = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("multicolor spell creates two Circu triggers");
    assert_eq!(ordering.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(ordering.trigger_candidates.len(), 2);
    assert!(ordering.trigger_candidates.iter().any(|entry| {
        entry.source == circu
            && entry.ability == "controller-casts-blue-spell-exile-opponent-library-top"
    }));
    assert!(ordering.trigger_candidates.iter().any(|entry| {
        entry.source == circu
            && entry.ability == "controller-casts-black-spell-exile-opponent-library-top"
    }));

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CIRCU-DIMIR-LOBOTOMIST"));
    game.validate_invariants()
        .expect("Circu cast and restriction state preserves invariants");
}
