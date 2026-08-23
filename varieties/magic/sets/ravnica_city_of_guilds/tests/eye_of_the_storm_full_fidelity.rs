//! Public stack-copy contract for Eye of the Storm's source-scoped exile loop.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
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
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn submit_exiled_spell_copy(
    game: &mut Game,
    caster: PlayerId,
    card: Option<cardbench_magic_engine::ObjectId>,
    targets: Vec<Target>,
) {
    let decision = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("Eye serial decision is open");
    assert_eq!(decision.kind, DecisionKind::ExiledSpellCopyCast);
    game.submit_decision(
        caster,
        decision.id,
        DecisionSelection::ExiledSpellCopyCast {
            card,
            targets,
            mode: None,
            color: None,
        },
    )
    .expect("serial Eye cast-or-decline selection is legal");
}

#[test]
fn eye_of_the_storm_has_its_exact_global_cast_trigger_contract() {
    let eye = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-EYE-OF-THE-STORM")
        .expect("Eye of the Storm definition exists");
    assert_eq!(eye.name, "Eye of the Storm");
    assert_eq!(
        eye.mana_cost,
        ManaCost::with_colors(5, [Color::Blue, Color::Blue])
    );
    assert_eq!(
        eye.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&eye.id));
    assert!(
        eye.supported_rules
            .contains(&"any-player-instant-sorcery-cast-exile-and-copy")
    );
    assert!(
        eye.supported_rules
            .contains(&"cast-exiled-spell-copies-without-paying-mana")
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The full no-priority serial loop is one state-machine contract.
fn eye_of_the_storm_exiles_physical_spells_and_terminates_observed_virtual_copies() {
    let caster = PlayerId(0);
    let eye_controller = PlayerId(1);
    let mut game = game_with_rav_bindings();
    let eye = game
        .put_on_battlefield(eye_controller, "RAV-EYE-OF-THE-STORM")
        .expect("Eye setup");
    let helix = game
        .add_card(caster, "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Helix setup");
    let mountain = game
        .put_on_battlefield(caster, "RAV-MOUNTAIN")
        .expect("red mana source");
    let plains = game
        .put_on_battlefield(caster, "RAV-PLAINS")
        .expect("white mana source");
    for player in [caster, eye_controller] {
        for _ in 0..3 {
            game.add_card(player, "RAV-FOREST", Zone::Library)
                .expect("draw-step library setup");
        }
    }
    game.begin_game().expect("game begins");
    pass_pair(&mut game);
    pass_pair(&mut game);
    game.activate_mana_ability(caster, mountain, Color::Red)
        .expect("red mana");
    game.activate_mana_ability(caster, plains, Color::White)
        .expect("white mana");
    game.clear_event_log();
    game.cast_spell(
        caster,
        CastRequest {
            card: helix,
            targets: vec![Target::Player(eye_controller)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("caster casts physical Helix");
    pass_pair(&mut game);

    let initial = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("Eye opens a public serial copy decision");
    assert_eq!(initial.kind, DecisionKind::ExiledSpellCopyCast);
    assert_eq!(initial.min_selections, 0);
    assert_eq!(initial.max_selections, 1);
    assert_eq!(
        initial
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![helix]
    );
    assert_eq!(
        game.priority, caster,
        "the spell caster chooses, not Eye's controller"
    );
    assert!(
        game.pass_priority(caster).is_err(),
        "priority remains blocked by the decision"
    );
    assert_eq!(game.zone_of(helix), Some(Zone::Exile));
    game.submit_decision(
        caster,
        initial.id,
        DecisionSelection::ExiledSpellCopyCast {
            card: Some(helix),
            targets: vec![Target::Player(eye_controller)],
            mode: None,
            color: None,
        },
    )
    .expect("caster independently targets the exiled Helix copy");

    let serial_decline = game
        .view_for_player(caster)
        .expect("caster sees the next serial decision")
        .pending_decision
        .expect("parent ability remains below the virtual copy");
    assert_eq!(serial_decline.kind, DecisionKind::ExiledSpellCopyCast);
    assert!(
        serial_decline.candidates.is_empty(),
        "one template is cast only once per trigger"
    );
    game.submit_decision(
        caster,
        serial_decline.id,
        DecisionSelection::ExiledSpellCopyCast {
            card: None,
            targets: vec![],
            mode: None,
            color: None,
        },
    )
    .expect("caster declines the remaining templates and ends the parent trigger");

    let (copy, copied_index, free_cast_index) =
        game.event_log
            .iter()
            .enumerate()
            .find_map(|(index, event)| match event {
                GameEvent::SpellCopied { copy, original, .. } if *original == helix => {
                    game.event_log.iter().enumerate().skip(index + 1).find_map(
                        |(free_index, next)| match next {
                            GameEvent::SpellCopyCastWithoutPayingManaCost {
                                player,
                                copy: free_copy,
                                original,
                            } if *player == caster && *free_copy == *copy && *original == helix => {
                                Some((*copy, index, free_index))
                            }
                            _ => None,
                        },
                    )
                }
                _ => None,
            })
            .expect("virtual copy and no-cost cast receipts are ordered");
    assert!(copied_index < free_cast_index);
    assert!(game.stack.iter().any(|item| {
        item.card == copy
            && item.controller == caster
            && item.targets == [Target::Player(eye_controller)]
    }));

    // The virtual copy is itself a cast spell. Eye observes it, terminates
    // it without adding it as a physical template, then opens one more
    // optional-copy boundary. Declining that boundary proves the loop is
    // policy-visible rather than recursively auto-casting.
    pass_pair(&mut game);
    let virtual_observation = game
        .view_for_player(caster)
        .expect("caster sees virtual-copy observation choice")
        .pending_decision
        .expect("virtual copy observation opens a serial choice");
    assert_eq!(virtual_observation.kind, DecisionKind::ExiledSpellCopyCast);
    assert_eq!(
        virtual_observation
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![helix]
    );
    game.submit_decision(
        caster,
        virtual_observation.id,
        DecisionSelection::ExiledSpellCopyCast {
            card: None,
            targets: vec![],
            mode: None,
            color: None,
        },
    )
    .expect("caster declines the observed-copy trigger");
    assert_eq!(game.zone_of(helix), Some(Zone::Exile));
    assert!(!game.stack.iter().any(|item| item.card == copy));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellExiledByTrigger { source, card, .. } if *source == eye && *card == helix
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyExiledByTrigger { copy: observed, original, source }
            if *observed == copy && *original == helix && *source == eye
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == eye && *ability == "any-player-instant-or-sorcery-cast-exile-and-copy"
    )));
    game.validate_invariants()
        .expect("Eye serial copy state preserves invariants");
    eprintln!("eye_of_the_storm_trace={:#?}", game.canonical_event_log());
}

#[test]
#[allow(clippy::too_many_lines)] // This scenario covers one complete historical-template serial choice boundary.
fn eye_of_the_storm_serially_casts_each_physical_template_without_priority() {
    let caster = PlayerId(0);
    let eye_controller = PlayerId(1);
    let mut game = game_with_rav_bindings();
    game.put_on_battlefield(eye_controller, "RAV-EYE-OF-THE-STORM")
        .expect("Eye setup");
    let first_helix = game
        .add_card(caster, "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("first Helix setup");
    let second_helix = game
        .add_card(caster, "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("second Helix setup");
    let lands = [
        game.put_on_battlefield(caster, "RAV-MOUNTAIN")
            .expect("first Mountain"),
        game.put_on_battlefield(caster, "RAV-PLAINS")
            .expect("first Plains"),
        game.put_on_battlefield(caster, "RAV-MOUNTAIN")
            .expect("second Mountain"),
        game.put_on_battlefield(caster, "RAV-PLAINS")
            .expect("second Plains"),
    ];
    for player in [caster, eye_controller] {
        for _ in 0..3 {
            game.add_card(player, "RAV-FOREST", Zone::Library)
                .expect("draw-step library setup");
        }
    }
    game.begin_game().expect("game begins");
    pass_pair(&mut game);
    pass_pair(&mut game);

    for (land, color) in lands[..2].iter().zip([Color::Red, Color::White]) {
        game.activate_mana_ability(caster, *land, color)
            .expect("first Helix mana");
    }
    game.cast_spell(
        caster,
        CastRequest {
            card: first_helix,
            targets: vec![Target::Player(eye_controller)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("caster casts the first physical Helix");
    pass_pair(&mut game);
    submit_exiled_spell_copy(&mut game, caster, None, vec![]);
    assert_eq!(game.zone_of(first_helix), Some(Zone::Exile));

    for (land, color) in lands[2..].iter().zip([Color::Red, Color::White]) {
        game.activate_mana_ability(caster, *land, color)
            .expect("second Helix mana");
    }
    game.clear_event_log();
    game.cast_spell(
        caster,
        CastRequest {
            card: second_helix,
            targets: vec![Target::Player(eye_controller)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("caster casts the second physical Helix");
    pass_pair(&mut game);

    let initial = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("both physical exile templates are offered");
    assert_eq!(initial.kind, DecisionKind::ExiledSpellCopyCast);
    assert_eq!(
        initial
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![first_helix, second_helix]
    );
    submit_exiled_spell_copy(
        &mut game,
        caster,
        Some(first_helix),
        vec![Target::Player(eye_controller)],
    );
    let second_choice = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("next serial template choice");
    assert_eq!(
        second_choice
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![second_helix],
        "each physical template can be copied once per resolving Eye trigger"
    );
    submit_exiled_spell_copy(
        &mut game,
        caster,
        Some(second_helix),
        vec![Target::Player(eye_controller)],
    );
    submit_exiled_spell_copy(&mut game, caster, None, vec![]);

    let first_copy_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SpellCopyCastWithoutPayingManaCost { original, .. }
                    if *original == first_helix
            )
        })
        .expect("first historical template cast receipt");
    let parent_resolution = game
        .event_log
        .iter()
        .enumerate()
        .skip(first_copy_index)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::AbilityResolved { ability, .. }
                    if *ability == "any-player-instant-or-sorcery-cast-exile-and-copy"
            )
            .then_some(index)
        })
        .expect("parent trigger resolves after all serial copy choices");
    assert!(
        !game.event_log[first_copy_index..parent_resolution]
            .iter()
            .any(|event| matches!(event, GameEvent::PriorityPassed { .. })),
        "no player receives priority between serial no-cost copy casts"
    );
    assert_eq!(game.zone_of(first_helix), Some(Zone::Exile));
    assert_eq!(game.zone_of(second_helix), Some(Zone::Exile));
    game.validate_invariants()
        .expect("serial template copying preserves source-scoped state invariants");
}
