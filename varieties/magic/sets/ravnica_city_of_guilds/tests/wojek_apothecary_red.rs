//! Red discovery contract for Wojek Apothecary's Radiance prevention ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn wojek_apothecary_has_its_radiance_damage_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOJEK-APOTHECARY")
        .expect("Wojek Apothecary definition exists");
    assert_eq!(definition.name, "Wojek Apothecary");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-radiance-prevent-one-damage")
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The public trace validates every Radiance shield recipient.
fn wojek_apothecary_can_stack_its_targeted_radiance_prevention_ability() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let apothecary = game
        .put_on_battlefield(PlayerId(0), "RAV-WOJEK-APOTHECARY")
        .expect("Wojek Apothecary setup");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-BLAZING-ARCHON")
        .expect("colored creature setup");
    let shared = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("shared-color creature setup");
    let off_color = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("off-color creature setup");
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("damage spell setup");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("red mana source setup")
        })
        .collect::<Vec<_>>();
    for card in [apothecary, target, shared, off_color] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("pre-game creature is old enough for a tap cost");
    }
    game.begin_game().expect("game begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: apothecary,
            ability_id: "tap-radiance-prevent-one-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Radiance prevention ability stacks");
    let first = game.priority;
    game.pass_priority(first)
        .expect("ability controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("opponent resolves ability");

    for protected in [apothecary, target, shared] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageShieldCreated {
                source,
                target: Target::Permanent(shielded),
                amount: 1,
            } if *source == apothecary && *shielded == protected
        )));
    }
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated {
            target: Target::Permanent(shielded),
            ..
        } if *shielded == off_color
    )));

    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("mountain produces red mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage spell casts after the shield is installed");
    let first = game.priority;
    game.pass_priority(first)
        .expect("caster passes damage spell");
    let second = game.priority;
    game.pass_priority(second).expect("damage spell resolves");

    assert_eq!(game.object(target).expect("target remains").damage, 3);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(shielded),
            amount: 1,
        } if *source == char && *shielded == target
    )));
    eprintln!("Wojek Apothecary trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Wojek Apothecary preserves invariant state");
}
