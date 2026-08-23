//! Red discovery contract for Suppression Field's global nonmana ability tax.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityCostModifier, CardType, Color, Game, GameEvent, ManaCost,
    PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn suppression_field_game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .expect("RAV cost modifiers register");
    game
}

fn pass_to_main_phase(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
}

#[test]
fn suppression_field_requires_a_live_nonmana_activation_tax() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUPPRESSION-FIELD")
        .expect("Suppression Field definition exists");
    assert_eq!(definition.name, "Suppression Field");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"static-nonmana-activated-ability-tax")
    );

    let bindings = rav_activated_ability_cost_modifier_bindings();
    assert!(bindings.iter().any(|binding| {
        binding.source_definition == "RAV-SUPPRESSION-FIELD"
            && binding.modifier
                == ActivatedAbilityCostModifier::IncreaseGeneric {
                    amount: 2,
                    nonmana_only: true,
                }
    }));
}

#[test]
fn suppression_field_charges_each_live_nonmana_ability_and_leaves_mana_abilities_free() {
    let mut game = suppression_field_game();
    let controller = PlayerId(0);
    let field = game
        .put_on_battlefield(PlayerId(1), "RAV-SUPPRESSION-FIELD")
        .expect("Suppression Field enters");
    let guildmage = game
        .put_on_battlefield(controller, "RAV-DIMIR-GUILDMAGE")
        .expect("Dimir Guildmage enters");
    let payment_island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("Island enters");
    let free_island = game
        .put_on_battlefield(controller, "RAV-ISLAND")
        .expect("second Island enters");
    let mountains = (0..5)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("Mountain enters")
        })
        .collect::<Vec<_>>();
    game.add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("draw target has a library card");
    game.set_entered_turn_for_setup(guildmage, 0)
        .expect("Guildmage is old enough to activate");
    game.begin_game().expect("game starts");
    pass_to_main_phase(&mut game);
    game.activate_mana_ability(controller, payment_island, Color::Blue)
        .expect("Island makes blue");
    for mountain in mountains {
        game.activate_mana_ability(controller, mountain, Color::Red)
            .expect("Mountain makes generic payment mana");
    }
    game.clear_event_log();

    game.activate_ability(
        controller,
        AbilityActivation {
            source: guildmage,
            ability_id: "target-player-draw",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("the taxed nonmana activation is payable");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ActivatedAbilityCostCalculated { context }
            if context.source == guildmage
                && context.base_mana_cost == ManaCost::with_colors(3, [Color::Blue])
                && context.increases.len() == 1
                && context.increases[0].source == field
                && context.effective_mana_cost == ManaCost::with_colors(5, [Color::Blue])
    )));
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves ability");

    game.activate_mana_ability(controller, free_island, Color::Blue)
        .expect("Suppression Field does not tax mana abilities");
    eprintln!("Suppression Field trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Suppression Field trace preserves invariants");
}
