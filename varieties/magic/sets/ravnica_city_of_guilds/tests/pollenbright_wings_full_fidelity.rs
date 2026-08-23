//! Full-fidelity contract for Pollenbright Wings' Aura-relative combat trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_attackers(game: &mut Game) {
    game.begin_game().expect("game begins");
    for _ in 0..16 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances toward attackers");
    }
    panic!("fixture did not reach declare attackers");
}

#[test]
fn pollenbright_wings_has_an_exact_aura_and_attached_combat_trigger_definition() {
    assert_eq!(
        executable_definition_id_for_collector(219),
        Ok("RAV-POLLENBRIGHT-WINGS")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-POLLENBRIGHT-WINGS")
        .expect("Pollenbright Wings definition exists");

    assert_eq!(definition.name, "Pollenbright Wings");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Green, Color::Blue])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::Blue])
    );
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-creature-combat-damage-saproling-count")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "attached-creature-combat-damage-create-saprolings"
    }));
}

#[test]
fn pollenbright_wings_uses_committed_attached_creature_combat_damage_as_its_token_count() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_triggers();
    let creature = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    let aura = game
        .add_card(controller, "RAV-POLLENBRIGHT-WINGS", Zone::Hand)
        .expect("Aura setup");
    game.grant_mana(controller, Color::Green, 3)
        .expect("green payment setup");
    game.grant_mana(controller, Color::Blue, 3)
        .expect("blue payment setup");
    game.cast_spell(
        controller,
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts targeting Watchwolf");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura remains live").attached_to,
        Some(creature)
    );
    assert!(
        game.characteristics(creature)
            .expect("creature characteristics")
            .keywords
            .contains(&Keyword::Flying),
        "the attachment grants Flying before combat"
    );
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature is long controlled for combat setup");
    game.clear_event_log();

    advance_to_attackers(&mut game);
    game.declare_attackers(controller, &[creature])
        .expect("enchanted creature attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(1), &[])
        .expect("defender declares no blocks");
    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount: 3 }
            if *source == creature && *player == PlayerId(1)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttachedCombatDamageTokenCountCaptured {
            aura: source,
            creature: captured_creature,
            player,
            ability,
            amount: 3,
            ..
        } if *source == aura
            && *captured_creature == creature
            && *player == PlayerId(1)
            && *ability == "attached-creature-combat-damage-create-saprolings"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == aura && *ability == "attached-creature-combat-damage-create-saprolings"
    )));
    pass_pair(&mut game);

    println!(
        "pollenbright_wings_event_log={:?}",
        game.canonical_event_log()
    );
    let token_count = game.players[controller.0]
        .battlefield
        .iter()
        .filter(|card| {
            game.object(**card)
                .is_ok_and(|object| object.token.is_some())
        })
        .count();
    assert_eq!(token_count, 3, "one token per committed combat damage");
    assert_eq!(game.players[PlayerId(1).0].life, 17);
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == controller))
            .count(),
        3
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == aura && *ability == "attached-creature-combat-damage-create-saprolings"
    )));
    game.validate_invariants()
        .expect("Aura-relative combat-token lifecycle preserves invariants");
}
