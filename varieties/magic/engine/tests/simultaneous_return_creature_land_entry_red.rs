//! Red regression: simultaneous creature-land returns retain land-entry events.
//!
//! Every returned creature-land is a land entering in the one graveyard-return
//! event.  CR 603.6a therefore makes each returning land observer see every
//! returning land, including itself.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, Step, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const MARCH: &str = "TST-CREATURE-LAND-RETURN-WATCHER";
const CREATURE_LAND: &str = "TST-SIMULTANEOUS-CREATURE-LAND";
const CAST_CREATURE: &str = "TST-SIMULTANEOUS-CREATURE-CAST";

fn definition(
    id: &'static str,
    name: &'static str,
    card_types: BTreeSet<CardType>,
) -> CardDefinition {
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["simultaneous-return-creature-land-entry-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
}

fn submit_all_trigger_orders(game: &mut Game, players: &[PlayerId]) {
    loop {
        let view = game
            .view_for_player(players[0])
            .expect("public game view is available");
        let Some(_) = view.pending_decision else {
            return;
        };
        let player = view.decision_player;
        let decision = game
            .view_for_player(player)
            .expect("deciding-player view is available")
            .pending_decision
            .expect("decision remains available to its player");
        assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
        game.submit_decision(
            player,
            decision.id,
            DecisionSelection::TriggerOrder(decision.trigger_candidates),
        )
        .expect("controller orders simultaneous land-entry triggers");
    }
}

#[test]
fn matching_creature_land_returns_capture_each_land_entry_observation() {
    let caster = PlayerId(0);
    let controller = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(MARCH, MARCH, BTreeSet::from([CardType::Enchantment])),
            definition(
                CREATURE_LAND,
                "Shared Creature Land",
                BTreeSet::from([CardType::Creature, CardType::Land]),
            ),
            definition(
                CAST_CREATURE,
                "Shared Creature Land",
                BTreeSet::from([CardType::Creature]),
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [
            TriggeredAbilityBinding {
                card_definition: MARCH,
                ability: TriggeredAbility {
                    id: "return-matching-creature-lands",
                    condition: TriggerCondition::AnyPlayerCastsCreatureSpell,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![
                        Effect::ReturnAllCreatureCardsMatchingCastCreatureSpellNameFromGraveyards,
                    ],
                },
            },
            TriggeredAbilityBinding {
                card_definition: CREATURE_LAND,
                ability: TriggeredAbility {
                    id: "observe-land-entry",
                    condition: TriggerCondition::LandEntersBattlefield,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
        ],
    )
    .expect("fixture initializes");
    game.put_on_battlefield(controller, MARCH)
        .expect("return watcher setup");
    let first = game
        .add_card(caster, CREATURE_LAND, Zone::Graveyard)
        .expect("first creature-land graveyard setup");
    let second = game
        .add_card(controller, CREATURE_LAND, Zone::Graveyard)
        .expect("second creature-land graveyard setup");
    let cast = game
        .add_card(caster, CAST_CREATURE, Zone::Hand)
        .expect("cast creature-land setup");

    advance_to_first_main(&mut game);
    game.clear_event_log();
    game.cast_spell(
        caster,
        CastRequest {
            card: cast,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("creature-land spell casts");
    pass_pair(&mut game);
    submit_all_trigger_orders(&mut game, &[caster, controller]);

    let land_entry_triggers = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: "observe-land-entry",
                    ..
                } if *source == first || *source == second
            )
        })
        .count();
    eprintln!(
        "simultaneous creature-land return trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(first), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(second), Some(Zone::Battlefield));
    assert_eq!(
        land_entry_triggers, 4,
        "each simultaneously returned creature-land must observe both land entries"
    );
    game.validate_invariants()
        .expect("simultaneous creature-land return remains invariant-valid");
}
