//! Red regression for the public Terrarion semantic slice.
//!
//! The green milestone must model its entry replacement, paid sacrificed
//! mana ability, explicit two-color allocation, and resulting graveyard
//! trigger through generic engine substrates.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaAbilityActivation,
    ManaAbilityBundleChoiceActivation, ManaBundle, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_static_entry_restriction_bindings,
    rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn terrarion_is_full_fidelity_colorless_artifact() {
    let terrarion = definition("RAV-TERRARION");
    assert_eq!(terrarion.mana_cost, ManaCost::new(1));
    assert_eq!(
        terrarion.card_types,
        [CardType::Artifact].into_iter().collect()
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&terrarion.id),
        "Terrarion must be explicitly promoted only with its complete public behavior"
    );
}

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
    .expect("RAV fixture builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn terrarion_entry_mana_cost_trigger_and_selected_bundle_are_auditable() {
    let mut entry_game = game();
    entry_game
        .register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("entry bindings register");
    let entered = entry_game
        .add_card(PlayerId(0), "RAV-TERRARION", Zone::Hand)
        .expect("Terrarion begins in hand");
    entry_game
        .grant_mana(PlayerId(0), Color::Colorless, 1)
        .expect("setup mana is available for the cast");
    entry_game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: entered,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect("Terrarion casts");
    pass_pair(&mut entry_game);
    assert!(entry_game.object(entered).expect("entered artifact").tapped);
    assert!(entry_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentEnteredTapped { permanent, source, .. }
            if *permanent == entered && *source == entered
    )));
    entry_game
        .validate_invariants()
        .expect("self-entry replacement has valid provenance");

    let mut game = game();
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .expect("mana cost bindings register");
    let terrarion = game
        .put_on_battlefield(PlayerId(0), "RAV-TERRARION")
        .expect("artifact begins untapped in isolated activation fixture");
    let first = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("first payment land enters");
    let second = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("second payment land enters");
    let drawn = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("library card exists for trigger draw");
    game.activate_mana_ability(PlayerId(0), first, Color::White)
        .expect("first land pays the activation");
    game.activate_mana_ability(PlayerId(0), second, Color::White)
        .expect("second land pays the activation");
    game.activate_bound_mana_ability_with_bundle_choice(
        PlayerId(0),
        ManaAbilityBundleChoiceActivation {
            activation: ManaAbilityActivation {
                source: terrarion,
                ability_id: "sacrifice-add-two-chosen-mana",
                chosen_color: None,
            },
            chosen_bundle: ManaBundle::new([(Color::White, 1), (Color::Blue, 1)]),
        },
    )
    .expect("paid source-sacrifice mana activation is legal");
    assert_eq!(game.zone_of(terrarion), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "graveyard trigger reaches the stack");
    pass_pair(&mut game);
    println!("Terrarion trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player exists")
            .mana_pool
            .amount(Color::White),
        1
    );
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player exists")
            .mana_pool
            .amount(Color::Blue),
        1
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedAsManaAbilityCost { player, source, ability }
            if *player == PlayerId(0) && *source == terrarion && *ability == "sacrifice-add-two-chosen-mana"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == terrarion && *ability == "graveyard-draw"
    )));
    game.validate_invariants()
        .expect("Terrarion lifecycle remains invariant-valid");
}
