//! Red discovery regression for Dowsing Shaman's graveyard-recursion activation.

use cardbench_magic_engine::{
    AbilityActivation, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
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
    .expect("RAV fixture builds")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn dowsing_shaman_requires_its_typed_enchantment_return_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DOWSING-SHAMAN")
        .expect("Dowsing Shaman definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the complete targeted graveyard-recursion ability is required"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"return-target-enchantment-from-graveyard")
    );

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-DOWSING-SHAMAN"
                && binding.ability.id == "return-target-enchantment-from-graveyard"
        })
        .expect("Dowsing Shaman typed graveyard-return binding exists");
    assert_eq!(
        ability.ability.mana_cost,
        ManaCost::with_colors(2, [Color::Green])
    );
    assert!(ability.ability.tap_cost);
    assert_eq!(
        ability.ability.targets,
        [TargetRequirement::EnchantmentCardInControllerGraveyard]
    );
    assert_eq!(
        ability.ability.effects,
        [Effect::ReturnTargetEnchantmentCardToHand]
    );
}

#[test]
fn dowsing_shaman_returns_only_a_controller_enchantment_card_after_stack_resolution() {
    let mut game = game_with_rav_bindings();
    let shaman = game
        .put_on_battlefield(PlayerId(0), "RAV-DOWSING-SHAMAN")
        .expect("Shaman begins on the battlefield");
    let enchantment = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Graveyard)
        .expect("controller enchantment begins in the graveyard");
    let forests = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-FOREST")
                .expect("Forest begins on the battlefield")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(shaman, 0)
        .expect("Shaman predates the measured turn");
    game.begin_game().expect("fixture begins game");
    for forest in forests {
        game.activate_mana_ability(PlayerId(0), forest, Color::Green)
            .expect("Forest pays one green mana");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shaman,
            ability_id: "return-target-enchantment-from-graveyard",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(enchantment)],
        },
    )
    .expect("Dowsing Shaman ability enters the stack");
    assert!(game.object(shaman).expect("source exists").tapped);
    resolve_top(&mut game);

    println!("dowsing_shaman_event_log={:?}", game.canonical_event_log());
    assert_eq!(game.zone_of(enchantment), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, mana_cost, .. }
            if *source == shaman
                && *ability == "return-target-enchantment-from-graveyard"
                && *mana_cost == ManaCost::with_colors(2, [Color::Green])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == shaman && *ability == "return-target-enchantment-from-graveyard"
    )));
    game.validate_invariants()
        .expect("Dowsing Shaman trace preserves invariants");
}

#[test]
fn dowsing_shaman_rejects_a_non_enchantment_graveyard_target_before_costs() {
    let mut game = game_with_rav_bindings();
    let shaman = game
        .put_on_battlefield(PlayerId(0), "RAV-DOWSING-SHAMAN")
        .expect("Shaman begins on the battlefield");
    let creature = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("non-enchantment begins in the graveyard");
    let forests = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-FOREST")
                .expect("Forest begins on the battlefield")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(shaman, 0)
        .expect("Shaman predates the measured turn");
    game.begin_game().expect("fixture begins game");
    for forest in forests {
        game.activate_mana_ability(PlayerId(0), forest, Color::Green)
            .expect("Forest pays one green mana");
    }
    game.clear_event_log();
    let result = game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shaman,
            ability_id: "return-target-enchantment-from-graveyard",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(creature)],
        },
    );
    assert!(
        result.is_err(),
        "a creature card is not an enchantment target"
    );
    assert!(!game.object(shaman).expect("source exists").tapped);
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected Dowsing Shaman activation preserves invariants");
}
