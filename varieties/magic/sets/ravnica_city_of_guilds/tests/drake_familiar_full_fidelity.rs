//! Event-log contracts for Drake Familiar's Flying and enchantment-return ETB.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, Step, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

const DRAKE_FAMILIAR: &str = "RAV-DRAKE-FAMILIAR";

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
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Target choice, trigger stack, and bounce receipts are one transcript.
fn drake_familiar_returns_only_a_legal_enchantment_through_the_trigger_stack() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let target_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target Aura has an attached creature");
    let enchantment = game
        .add_card(PlayerId(1), "RAV-MARK-OF-EVICTION", Zone::Hand)
        .expect("target enchantment starts in hand");
    game.enter_attachment_without_cast(enchantment, target_creature)
        .expect("target Aura is live before the cast");
    let islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Island setup")
        })
        .collect::<Vec<_>>();
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, "RAV-FOREST", Zone::Library)
            .expect("library setup");
    }
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Drake Familiar mana");
    }

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: drake,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Drake Familiar casts");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("ETB opens target decision");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityTargets);
    assert_eq!((decision.min_selections, decision.max_selections), (1, 1));
    let events_before_bad_target = game.event_log.clone();
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Targets(vec![Target::Permanent(target_creature)]),
        )
        .is_err(),
        "a creature is not an enchantment target"
    );
    assert_eq!(game.event_log, events_before_bad_target);
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Permanent(enchantment)]),
    )
    .expect("live Aura is a legal target");
    assert_eq!(
        game.stack.last().expect("ETB is stacked").targets,
        vec![Target::Permanent(enchantment)]
    );
    pass_pair(&mut game);

    println!(
        "Drake Familiar target-return trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(enchantment), Some(Zone::Hand));
    let moved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == enchantment))
        .expect("owner-hand zone receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == drake && *ability == "etb-return-target-enchantment-owner-hand"
            )
        })
        .expect("ETB resolution receipt");
    assert!(
        moved < resolved,
        "the target moves during, not after, resolution"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&DRAKE_FAMILIAR));
    game.validate_invariants()
        .expect("Drake Familiar target-return trace is invariant-valid");
}
