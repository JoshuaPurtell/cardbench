//! Red-to-green contract for Firemane Angel's zone-aware abilities.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId,
    TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
    RAV_FULL_FIDELITY_DEFINITION_IDS,
};

fn rav_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn firemane_angel_is_manifested_with_battlefield_and_owner_graveyard_upkeep_rules() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FIREMANE-ANGEL")
        .expect("Firemane Angel definition exists");

    assert_eq!(definition.name, "Firemane Angel");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::White, Color::White])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!(definition.power, Some(4));
    assert_eq!(definition.toughness, Some(3));
    assert_eq!(definition.keywords, [Keyword::Flying, Keyword::FirstStrike]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "first-strike",
            "battlefield-and-owner-graveyard-upkeep-life-gain",
            "owner-graveyard-return-activation",
        ]
    );

    let battlefield_upkeep = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.condition == TriggerCondition::BeginningOfUpkeep
        })
        .expect("battlefield upkeep trigger binding exists");
    assert_eq!(battlefield_upkeep.ability.effects.len(), 1);

    let return_activation = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "owner-graveyard-return-to-battlefield"
        })
        .expect("owner-graveyard return activation exists");
    assert_eq!(
        return_activation.ability.mana_cost,
        ManaCost::with_colors(6, [Color::Red, Color::Red, Color::White, Color::White])
    );
    assert!(return_activation.ability.targets.is_empty());
}

#[test]
fn firemane_angel_gains_life_from_battlefield_and_owner_graveyard_then_returns_from_that_graveyard()
{
    let controller = PlayerId(0);
    let mut battlefield_game = rav_game();
    let battlefield_angel = battlefield_game
        .put_on_battlefield(controller, "RAV-FIREMANE-ANGEL")
        .expect("battlefield Angel setup");
    battlefield_game.begin_game().expect("game begins");
    pass_pair(&mut battlefield_game);
    assert_eq!(
        battlefield_game
            .player(controller)
            .expect("controller exists")
            .life,
        21
    );
    assert!(battlefield_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player, amount } if *player == controller && *amount == 1
    )));

    let mut graveyard_game = rav_game();
    let graveyard_angel = graveyard_game
        .add_card(controller, "RAV-FIREMANE-ANGEL", Zone::Graveyard)
        .expect("graveyard Angel setup");
    let mountains = (0..8)
        .map(|_| {
            graveyard_game
                .put_on_battlefield(controller, "RAV-MOUNTAIN")
                .expect("red mana source setup")
        })
        .collect::<Vec<_>>();
    let plains = (0..2)
        .map(|_| {
            graveyard_game
                .put_on_battlefield(controller, "RAV-PLAINS")
                .expect("white mana source setup")
        })
        .collect::<Vec<_>>();
    graveyard_game.begin_game().expect("game begins");
    pass_pair(&mut graveyard_game);
    assert_eq!(
        graveyard_game
            .player(controller)
            .expect("controller exists")
            .life,
        21
    );
    for mountain in mountains {
        graveyard_game
            .activate_mana_ability(controller, mountain, Color::Red)
            .expect("red activation mana");
    }
    for plain in plains {
        graveyard_game
            .activate_mana_ability(controller, plain, Color::White)
            .expect("white activation mana");
    }
    graveyard_game
        .activate_ability(
            controller,
            AbilityActivation {
                source: graveyard_angel,
                ability_id: "owner-graveyard-return-to-battlefield",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .expect("owner activates the graveyard return ability");
    pass_pair(&mut graveyard_game);

    assert_eq!(
        graveyard_game.zone_of(graveyard_angel),
        Some(Zone::Battlefield)
    );
    assert!(graveyard_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated { source, ability, .. }
            if *source == graveyard_angel && *ability == "owner-graveyard-return-to-battlefield"
    )));
    println!(
        "firemane_battlefield_trace={:#?}",
        battlefield_game.canonical_event_log()
    );
    println!(
        "firemane_graveyard_trace={:#?}",
        graveyard_game.canonical_event_log()
    );
    battlefield_game
        .validate_invariants()
        .expect("battlefield upkeep trace is valid");
    graveyard_game
        .validate_invariants()
        .expect("graveyard trace is valid");
    assert_eq!(
        battlefield_game.zone_of(battlefield_angel),
        Some(Zone::Battlefield)
    );
}
