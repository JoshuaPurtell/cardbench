//! Red regression for Carrion Howler's life-paid source pump.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CardType, Color, Effect, Game, GameEvent,
    GeneralizedAbilityActivation, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_costs() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("RAV generalized costs register before the game begins");
    game
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second).expect("opponent passes");
}

#[test]
fn carrion_howler_requires_a_life_paid_source_pump_without_double_strike() {
    let howler = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARRION-HOWLER")
        .expect("Carrion Howler definition exists");
    assert_eq!(howler.mana_cost, ManaCost::with_colors(3, [Color::Black]));
    assert_eq!(
        howler.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((howler.power, howler.toughness), (Some(2), Some(2)));
    assert!(
        howler.keywords.is_empty(),
        "Carrion Howler has no static combat keyword"
    );
    assert!(
        howler
            .supported_rules
            .contains(&"life-paid-source-plus-two-minus-one")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&howler.id));

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == howler.id)
        .expect("Carrion Howler source pump binding exists")
        .ability;
    assert_eq!(ability.mana_cost, ManaCost::new(0));
    assert!(!ability.tap_cost);
    assert!(ability.targets.is_empty());
    assert_eq!(
        ability.effects,
        [Effect::ModifySourcePtUntilEndOfTurn {
            power: 2,
            toughness: -1,
        }]
    );
}

#[test]
fn carrion_howler_pays_life_before_its_source_pump_resolves() {
    let mut game = game_with_rav_costs();
    let howler = game
        .put_on_battlefield(PlayerId(0), "RAV-CARRION-HOWLER")
        .expect("Carrion Howler starts on the battlefield");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: howler,
                ability_id: "pay-life-pump-plus-two-minus-one",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment::default(),
            mana_payment_selection: None,
        },
    )
    .expect("one-life source pump activation is legal");

    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 19);
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::AbilityLifePaid { player: PlayerId(0), source, ability: "pay-life-pump-plus-two-minus-one", amount: 1 },
            GameEvent::AbilityActivated { player: PlayerId(0), source: activated_source, ability: "pay-life-pump-plus-two-minus-one", .. },
        ] if *source == howler && *activated_source == howler
    )));
    resolve_top(&mut game);

    assert_eq!(game.zone_of(howler), Some(Zone::Battlefield));
    assert_eq!(
        game.characteristics(howler)
            .expect("Howler remains live")
            .power,
        Some(4)
    );
    assert_eq!(
        game.characteristics(howler)
            .expect("Howler remains live")
            .toughness,
        Some(1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability: "pay-life-pump-plus-two-minus-one", .. }
            if *source == howler
    )));
    println!("Carrion Howler trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("life-paid activation keeps state-machine invariants valid");
}
