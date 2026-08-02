//! Red-to-green full-fidelity contract for Golgari Grave-Troll.
//!
//! A zero-toughness creature must receive its graveyard-derived entry counters
//! before state-based actions run. The follow-up activation consumes one of
//! those physical counters as a cost and creates the ordinary regeneration
//! shield on the stack.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CastRequest, Color, CounterKind, Effect, Game,
    GameEvent, GeneralizedAbilityActivation, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
    rav_replacement_effect_bindings, rav_static_entry_restriction_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn golgari_grave_troll_enters_with_graveyard_counters_and_regenerates() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-GRAVE-TROLL")
        .expect("Golgari Grave-Troll definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Golgari Grave-Troll cannot be positive-manifest while its entry counters and regeneration are absent"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "entry-plus-one-counters-equal-controller-graveyard-creature-cards",
            "remove-plus-one-counter-regenerate",
        ]
    );
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Grave-Troll regeneration binding exists");
    assert_eq!(binding.ability.id, "remove-plus-one-counter-regenerate");
    assert_eq!(binding.ability.mana_cost, ManaCost::new(1));
    assert_eq!(binding.ability.effects, [Effect::RegenerateSource]);

    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("entry bindings register");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("generalized counter costs register");
    let troll = game
        .add_card(controller, "RAV-GOLGARI-GRAVE-TROLL", Zone::Hand)
        .expect("Troll begins in hand");
    for definition in ["RAV-WATCHWOLF", "RAV-GOLGARI-BROWNSCALE", "RAV-SEWERDREG"] {
        game.add_card(controller, definition, Zone::Graveyard)
            .expect("three creature cards begin in controller graveyard");
    }
    for _ in 0..6 {
        game.grant_mana(controller, Color::Green, 1)
            .expect("mana setup succeeds");
    }

    game.cast_spell(
        controller,
        CastRequest {
            card: troll,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Troll casts");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(troll), Some(Zone::Battlefield));
    assert_eq!(
        game.object(troll)
            .expect("Troll remains a battlefield object")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&3),
        "entry counters must arrive before the zero-toughness SBA check"
    );
    assert_eq!(
        game.characteristics(troll)
            .expect("counter-bearing Troll has characteristics")
            .toughness,
        Some(3)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced {
            source,
            card,
            counter: CounterKind::PlusOnePlusOne,
            amount: 3,
        } if *source == troll && *card == troll
    )));

    game.activate_ability_with_generalized_costs(
        controller,
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: troll,
                ability_id: "remove-plus-one-counter-regenerate",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![troll],
                ..AbilityCostPayment::default()
            },
            mana_payment_selection: None,
        },
    )
    .expect("one generic mana and one counter activate regeneration");
    assert_eq!(
        game.object(troll)
            .expect("Troll remains while ability is on stack")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&2),
        "the counter cost is paid before the ability reaches the stack"
    );
    resolve_top(&mut game);
    println!(
        "Golgari Grave-Troll full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == troll && *target == troll
    )));
    game.validate_invariants()
        .expect("Grave-Troll entry and counter-cost regeneration preserve invariants");
}

#[test]
fn grave_troll_entry_counters_use_the_normal_quantity_replacement_chain() {
    let controller = PlayerId(0);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("entry bindings register");
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .expect("quantity-replacement bindings register");
    game.put_on_battlefield(controller, "RAV-DOUBLING-SEASON")
        .expect("Doubling Season starts live");
    let troll = game
        .add_card(controller, "RAV-GOLGARI-GRAVE-TROLL", Zone::Hand)
        .expect("Troll begins in hand");
    for definition in ["RAV-WATCHWOLF", "RAV-GOLGARI-BROWNSCALE", "RAV-SEWERDREG"] {
        game.add_card(controller, definition, Zone::Graveyard)
            .expect("three creature cards begin in controller graveyard");
    }
    game.grant_mana(controller, Color::Green, 5)
        .expect("Troll cast payment exists");
    game.cast_spell(
        controller,
        CastRequest {
            card: troll,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Troll casts");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(troll), Some(Zone::Battlefield));
    assert_eq!(
        game.object(troll)
            .expect("Troll survives entry")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&6),
        "Doubling Season replaces the one three-counter entry event once"
    );
    assert!(game.event_log.windows(3).any(|events| {
        matches!(
            events,
            [
                GameEvent::ReplacementEffectApplied {
                    affected_player: PlayerId(0),
                    event: cardbench_magic_engine::ReplacementEventKind::CounterPlacement {
                        counter: CounterKind::PlusOnePlusOne,
                    },
                    original_amount: 3,
                    replacement_amount: 6,
                    ..
                },
                GameEvent::CounterPlaced {
                    source,
                    card,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 6,
                },
                GameEvent::PermanentEnteredWithCounters {
                    permanent,
                    source: entry_source,
                    counter: CounterKind::PlusOnePlusOne,
                    base_amount: 3,
                    applied_amount: 6,
                    ..
                },
            ] if *source == troll
                && *card == troll
                && *permanent == troll
                && *entry_source == troll
        )
    }));
    game.validate_invariants()
        .expect("quantity-replaced Grave-Troll entry preserves invariants");
}
