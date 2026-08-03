//! Regression: Greater Mossdog's graveyard trigger must observe creature
//! targets chosen for spells and abilities, then expose its optional return
//! through the normal policy decision boundary.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn greater_mossdog_returns_from_graveyard_after_creature_becomes_target() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let mossdog = game
        .add_card(PlayerId(0), "RAV-GREATER-MOSSDOG", Zone::Graveyard)
        .expect("Mossdog enters graveyard");
    let guildmage = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-GUILDMAGE")
        .expect("Guildmage enters");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target enters");
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    for permanent in [guildmage, target, mountain] {
        game.set_entered_turn_for_setup(permanent, 0)
            .expect("fixture permanent is old enough");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player).expect("advance to main phase");
    }
    game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
        .expect("red mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "grant-haste",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Guildmage targets creature");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked {
            source,
            ability: "targeted-creature-return-from-graveyard",
            ..
        } if *source == mossdog
    )));
    assert_eq!(game.stack.len(), 2, "Mossdog trigger is above the ability");

    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1))
        .expect("trigger resolves into optional choice");
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("Mossdog return is optional");
    game.submit_policy_move(
        PlayerId(0),
        "test.pay-mossdog-return.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: mossdog,
            ability: "targeted-creature-return-from-graveyard",
            pay: true,
            target: None,
        },
    )
    .expect("Mossdog return choice resolves");

    assert_eq!(game.zone_of(mossdog), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Hand }
            if *card == mossdog
    )));
    game.validate_invariants()
        .expect("trigger transition is valid");
}
