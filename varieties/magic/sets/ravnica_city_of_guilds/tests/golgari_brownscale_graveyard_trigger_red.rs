//! Red-to-green full-fidelity contract for Golgari Brownscale.
//!
//! This uses only `CardBench` semantic operations: a Dredge replacement moves
//! the source from its graveyard to hand, then a source-bound trigger gains
//! its controller life after the replacement has completed.

use cardbench_magic_engine::{Effect, Game, GameEvent, PlayerId, Step, TriggerCondition, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn golgari_brownscale_graveyard_to_hand_trigger_is_ability_complete() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-BROWNSCALE")
        .expect("Golgari Brownscale definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Golgari Brownscale cannot be positive-manifest while its graveyard-to-hand trigger is absent"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "graveyard-to-hand-gain-life",
        ]
    );
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Brownscale needs its source-bound graveyard-to-hand trigger");
    assert_eq!(binding.ability.condition, TriggerCondition::GraveyardToHand);
    assert_eq!(
        binding.ability.effects,
        [Effect::GainLifeController { amount: 2 }]
    );

    // Three seats make player zero's turn-one draw live without advancing
    // through another full two-player turn.
    let player = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        3,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("three-player RAV game builds");
    let brownscale = game
        .add_card(player, "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .expect("Brownscale begins in its controller graveyard");
    let bottom = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("library bottom exists");
    let top = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("library top exists");
    game.begin_game().expect("game begins");
    for seat in 0..3 {
        game.pass_priority(PlayerId(seat))
            .expect("each live seat passes upkeep priority");
    }
    assert_eq!(game.step, Step::Draw);
    game.resolve_pending_draw(player, Some(brownscale))
        .expect("Brownscale replaces the live draw");
    assert_eq!(game.zone_of(bottom), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(top), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(brownscale), Some(Zone::Hand));
    assert_eq!(game.player(player).expect("player exists").life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Dredged { player: dredging_player, card, count: 2 }
            if *dredging_player == player && *card == brownscale
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == brownscale && *ability == "graveyard-to-hand-gain-life"
    )));

    for seat in 0..3 {
        game.pass_priority(PlayerId(seat))
            .expect("each live seat passes the Brownscale trigger");
    }
    println!(
        "Golgari Brownscale full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.player(player).expect("player exists").life, 22);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player: gained_player, amount: 2 }
            if *gained_player == player
    )));
    game.validate_invariants()
        .expect("Brownscale graveyard-to-hand trigger preserves invariants");
}

#[test]
fn brownscale_does_not_observe_an_unrelated_library_to_hand_move() {
    let player = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        3,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("three-player RAV game builds");
    let brownscale = game
        .add_card(player, "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .expect("Brownscale remains in its controller graveyard");
    let ordinary_draw = game
        .add_card(player, "RAV-FOREST", Zone::Library)
        .expect("ordinary draw card exists");
    game.begin_game().expect("game begins");
    for seat in 0..3 {
        game.pass_priority(PlayerId(seat))
            .expect("each live seat passes upkeep priority");
    }
    game.resolve_pending_draw(player, None)
        .expect("ordinary draw resolves");

    assert_eq!(game.zone_of(ordinary_draw), Some(Zone::Hand));
    assert_eq!(game.zone_of(brownscale), Some(Zone::Graveyard));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == brownscale && *ability == "graveyard-to-hand-gain-life"
        )),
        "a Brownscale trigger needs its own graveyard-to-hand transition"
    );
    game.validate_invariants()
        .expect("unrelated ordinary draw preserves trigger invariants");
}
