//! Event-log contracts for Copy Enchantment's replacement-style entry choice.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, Game, GameEvent,
    PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_entry_copy_bindings, rav_mana_ability_bindings,
};

fn rav_game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .expect("RAV entry-copy bindings register");
    game
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first priority pass");
        let second = game.priority;
        game.pass_priority(second).expect("second priority pass");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

fn add_and_activate_islands(game: &mut Game, player: PlayerId, count: usize) {
    let islands = (0..count)
        .map(|_| {
            game.put_on_battlefield(player, "RAV-ISLAND")
                .expect("Island setup")
        })
        .collect::<Vec<_>>();
    advance_to_precombat_main(game);
    for island in islands {
        game.activate_mana_ability(player, island, Color::Blue)
            .expect("Island produces blue mana");
    }
}

fn resolve_to_entry_choice(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("caster passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("opponent pass reaches entry replacement choice");
}

fn cast_copy_enchantment(game: &mut Game, copy: cardbench_magic_engine::ObjectId) {
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: copy,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Copy Enchantment casts");
    resolve_to_entry_choice(game);
}

fn source_choice(
    game: &Game,
    expected_candidate: cardbench_magic_engine::ObjectId,
) -> cardbench_magic_engine::DecisionId {
    let controller = game
        .view_for_player(PlayerId(0))
        .expect("controller view builds");
    let decision = controller
        .pending_decision
        .expect("Copy Enchantment opens one public source choice");
    assert_eq!(decision.kind, DecisionKind::PermanentEntryCopySource);
    assert_eq!(decision.visibility, DecisionVisibility::Public);
    assert_eq!((decision.min_selections, decision.max_selections), (0, 1));
    assert!(
        decision
            .candidates
            .iter()
            .any(|candidate| candidate.id == expected_candidate),
        "the live enchantment is a legal copied-value source"
    );
    let opponent = game
        .view_for_player(PlayerId(1))
        .expect("opponent view builds");
    assert_eq!(opponent.pending_decision, Some(decision.clone()));
    decision.id
}

#[test]
fn copy_enchantment_optionally_enters_with_a_non_aura_enchantments_copiable_values() {
    let mut game = rav_game();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-PERILOUS-FORAYS")
        .expect("non-Aura enchantment setup");
    let copy = game
        .add_card(PlayerId(0), "RAV-COPY-ENCHANTMENT", Zone::Hand)
        .expect("Copy Enchantment setup");
    add_and_activate_islands(&mut game, PlayerId(0), 3);
    game.clear_event_log();

    cast_copy_enchantment(&mut game, copy);
    let decision = source_choice(&game, source);
    assert_eq!(
        game.zone_of(copy),
        None,
        "the entry choice retains the spell on the separately represented stack"
    );
    game.submit_decision(
        PlayerId(0),
        decision,
        DecisionSelection::Objects(vec![source]),
    )
    .expect("controller selects the non-Aura source");

    assert_eq!(game.zone_of(copy), Some(Zone::Battlefield));
    assert_eq!(
        game.card_definition(copy)
            .expect("copied permanent has effective definition")
            .id,
        "RAV-PERILOUS-FORAYS"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-COPY-ENCHANTMENT"));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopied {
            source: event_source,
            target,
            ..
        } if *event_source == source && *target == copy
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == copy
    )));
    println!(
        "copy_enchantment_non_aura_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("non-Aura entry copy preserves stack and layer invariants");
}

#[test]
fn copy_enchantment_may_decline_its_public_entry_copy_choice() {
    let mut game = rav_game();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-PERILOUS-FORAYS")
        .expect("non-Aura enchantment setup");
    let copy = game
        .add_card(PlayerId(0), "RAV-COPY-ENCHANTMENT", Zone::Hand)
        .expect("Copy Enchantment setup");
    add_and_activate_islands(&mut game, PlayerId(0), 3);
    game.clear_event_log();

    cast_copy_enchantment(&mut game, copy);
    let decision = source_choice(&game, source);
    game.submit_decision(PlayerId(0), decision, DecisionSelection::Objects(vec![]))
        .expect("controller declines the optional copy");

    assert_eq!(game.zone_of(copy), Some(Zone::Battlefield));
    assert_eq!(
        game.card_definition(copy)
            .expect("uncopied permanent retains printed definition")
            .id,
        "RAV-COPY-ENCHANTMENT"
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::PermanentCopied { target, .. } if *target == copy
        )),
        "a declined replacement must not materialize a layer-one snapshot"
    );
    game.validate_invariants()
        .expect("declining entry copy preserves stack and layer invariants");
}

#[test]
#[allow(clippy::too_many_lines)] // The two linked no-priority choices need one readable event-log contract.
fn copying_an_aura_selects_its_attachment_before_the_copy_enters() {
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("Aura target setup");
    let source_aura = game
        .add_card(PlayerId(0), "RAV-GALVANIC-ARC", Zone::Hand)
        .expect("Aura source setup");
    let copy = game
        .add_card(PlayerId(0), "RAV-COPY-ENCHANTMENT", Zone::Hand)
        .expect("Copy Enchantment setup");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain setup")
        })
        .collect::<Vec<_>>();
    let islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("Island setup")
        })
        .collect::<Vec<_>>();
    advance_to_precombat_main(&mut game);
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Mountain produces red mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: source_aura,
            targets: vec![cardbench_magic_engine::Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Galvanic Arc casts");
    let first = game.priority;
    game.pass_priority(first).expect("Aura caster passes");
    let second = game.priority;
    game.pass_priority(second).expect("Aura resolves");
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("Island produces blue mana");
    }
    game.clear_event_log();

    cast_copy_enchantment(&mut game, copy);
    let source_decision = source_choice(&game, source_aura);
    game.submit_decision(
        PlayerId(0),
        source_decision,
        DecisionSelection::Objects(vec![source_aura]),
    )
    .expect("controller selects the Aura's copiable values");

    let aura_choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view builds")
        .pending_decision
        .expect("copied Aura opens an attachment choice before entry");
    assert_eq!(
        aura_choice.kind,
        DecisionKind::PermanentEntryCopyAuraAttachment
    );
    assert_eq!(
        aura_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![creature]
    );
    assert_eq!(
        game.zone_of(copy),
        None,
        "the copied Aura chooses an endpoint before it enters the battlefield"
    );
    game.submit_decision(
        PlayerId(0),
        aura_choice.id,
        DecisionSelection::Objects(vec![creature]),
    )
    .expect("controller chooses the copied Aura attachment");

    assert_eq!(game.zone_of(copy), Some(Zone::Battlefield));
    assert_eq!(
        game.card_definition(copy)
            .expect("copied Aura has effective definition")
            .id,
        "RAV-GALVANIC-ARC"
    );
    assert_eq!(
        game.object(copy)
            .expect("copied Aura object remains live")
            .attached_to,
        Some(creature)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopied {
            source,
            target,
            ..
        } if *source == source_aura && *target == copy
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttachmentEstablishedWithoutContinuousEffect {
            attachment,
            target,
            ..
        } if *attachment == copy && *target == creature
    )));
    println!(
        "copy_enchantment_aura_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("copied Aura enters attached without an orphaned-Aura state");
}
