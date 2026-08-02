//! Full-fidelity contract for Gaze of the Gorgon's delayed combat provenance.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, GameEvent, PlayerId, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

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

fn advance_to_declare_attackers(game: &mut Game) {
    while game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        } else {
            pass_pair(game);
        }
    }
}

#[test]
fn gaze_has_exact_hybrid_targeted_delayed_combat_definition() {
    let gaze = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GAZE-OF-THE-GORGON")
        .expect("Gaze definition exists");
    assert_eq!(
        executable_definition_id_for_collector(246),
        Ok("RAV-GAZE-OF-THE-GORGON")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&gaze.id));
    assert_eq!(gaze.card_types, [CardType::Instant].into_iter().collect());
    assert_eq!(
        gaze.effects,
        [Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction]
    );
    assert_eq!(
        gaze.effects[0].target_requirement(),
        Some(TargetRequirement::Creature)
    );
    assert_eq!(
        gaze.colors,
        [Color::Black, Color::Green].into_iter().collect()
    );
    assert_eq!(gaze.mana_cost.generic, 3);
    assert_eq!(gaze.mana_cost.hybrid.len(), 1);
}

#[test]
#[allow(clippy::similar_names, clippy::too_many_lines)] // The full turn trace is the contract.
fn gaze_regenerates_target_then_uses_exact_block_history_at_end_of_combat() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    add_opening_library(&mut game, PlayerId(0));
    add_opening_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-THUG")
        .expect("attacker begins on battlefield");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker is long-controlled");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker begins on battlefield");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker is long-controlled");
    let gaze = game
        .add_card(PlayerId(0), "RAV-GAZE-OF-THE-GORGON", Zone::Hand)
        .expect("Gaze begins in hand");
    let swamps = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Swamp begins on battlefield")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("fixture starts game");
    advance_to_declare_attackers(&mut game);

    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Thug attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(
        PlayerId(1),
        &[cardbench_magic_engine::CombatBlock { attacker, blocker }],
    )
    .expect("Watchwolf blocks Thug");
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Swamp pays Gaze");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: gaze,
            targets: vec![Target::Permanent(attacker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Gaze targets the attacking creature");
    pass_pair(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == gaze && *target == attacker
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedCombatDestructionScheduled {
            source,
            target,
            due_turn,
            ..
        } if *source == gaze && *target == attacker && *due_turn == game.turn
    )));

    pass_pair(&mut game);
    assert_eq!(game.step, Step::CombatDamage);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { target, .. } if *target == attacker
    )));
    assert_eq!(game.zone_of(attacker), Some(Zone::Battlefield));

    pass_pair(&mut game);
    assert_eq!(game.step, Step::EndOfCombat);
    assert_eq!(
        game.stack.len(),
        1,
        "due instruction uses the ordinary stack"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedCombatDestructionStacked {
            source,
            target,
            participants,
            ..
        } if *source == gaze
            && *target == attacker
            && participants.len() == 1
            && participants[0].permanent == blocker
            && participants[0].incarnation > 0
    )));
    pass_pair(&mut game);

    println!(
        "Gaze of the Gorgon trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(blocker), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == gaze && *card == blocker
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { ability, .. }
            if *ability == cardbench_magic_engine::DELAYED_COMBAT_HISTORY_DESTRUCTION_ABILITY_ID
    )));
    game.validate_invariants()
        .expect("delayed combat-history transition preserves invariants");
}
