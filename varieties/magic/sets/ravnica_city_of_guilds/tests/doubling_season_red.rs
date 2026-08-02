//! Red regression for the token/counter replacement substrate used by Doubling Season.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, CounterKind, Game,
    GameEvent, ManaCost, ManaPaymentSelection, PlayerId, ReplacementEventKind, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_replacement_effect_bindings,
};

#[test]
fn doubling_season_has_a_full_fidelity_replacement_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DOUBLING-SEASON")
        .expect("Doubling Season definition exists");

    assert_eq!(definition.name, "Doubling Season");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Green])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"controlled-token-creation-replacement")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"controlled-plus-one-counter-replacement")
    );
}

fn game_with_replacements() -> Game {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .expect("RAV replacement bindings register before the game begins");
    game
}

fn resolve_scatter(game: &mut Game, controller: PlayerId) {
    let scatter = game
        .add_card(controller, "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .expect("Scatter setup");
    let first = game
        .put_on_battlefield(controller, "RAV-GOLGARI-BROWNSCALE")
        .expect("first convoke contributor");
    let second = game
        .put_on_battlefield(controller, "RAV-GOLGARI-BROWNSCALE")
        .expect("second convoke contributor");
    let third = game
        .put_on_battlefield(controller, "RAV-GOLGARI-BROWNSCALE")
        .expect("third convoke contributor");
    game.grant_mana(controller, Color::Red, 2)
        .expect("generic payment setup");
    game.cast_spell(
        controller,
        CastRequest {
            card: scatter,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: first,
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: second,
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: third,
                    contribution: ConvokeContribution::Generic,
                },
            ],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Scatter casts");
    game.pass_priority(controller)
        .expect("controller passes priority");
    game.pass_priority(PlayerId((controller.0 + 1) % 2))
        .expect("opponent resolves Scatter");
}

fn token_count(game: &Game, player: PlayerId) -> usize {
    game.player(player)
        .expect("player exists")
        .battlefield
        .iter()
        .filter(|card| {
            game.object(**card)
                .is_ok_and(|object| object.token.is_some())
        })
        .count()
}

#[test]
#[allow(clippy::too_many_lines)] // The token and counter replacement traces share one source-lifecycle contract.
fn doubling_season_doubles_controlled_token_and_counter_events_once_per_source() {
    let mut game = game_with_replacements();
    let first_season = game
        .put_on_battlefield(PlayerId(0), "RAV-DOUBLING-SEASON")
        .expect("first replacement source");
    let second_season = game
        .put_on_battlefield(PlayerId(0), "RAV-DOUBLING-SEASON")
        .expect("second replacement source");
    game.clear_event_log();
    resolve_scatter(&mut game, PlayerId(0));

    assert_eq!(token_count(&game, PlayerId(0)), 12);
    let token_replacements = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::ReplacementEffectApplied {
                    event: ReplacementEventKind::TokenCreation,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(token_replacements.len(), 2);
    assert!(matches!(
        token_replacements[0],
        GameEvent::ReplacementEffectApplied {
            source,
            affected_player: PlayerId(0),
            original_amount: 3,
            replacement_amount: 6,
            ..
        } if *source == first_season
    ));
    assert!(matches!(
        token_replacements[1],
        GameEvent::ReplacementEffectApplied {
            source,
            affected_player: PlayerId(0),
            original_amount: 6,
            replacement_amount: 12,
            ..
        } if *source == second_season
    ));
    game.validate_invariants()
        .expect("finite token-replacement chain preserves invariants");

    let mut counter_game = game_with_replacements();
    let season = counter_game
        .put_on_battlefield(PlayerId(0), "RAV-DOUBLING-SEASON")
        .expect("counter replacement source");
    let vigor = counter_game
        .add_card(PlayerId(0), "RAV-VIGOR-MORTIS", Zone::Hand)
        .expect("Vigor Mortis setup");
    let target = counter_game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("counter target setup");
    counter_game
        .grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black mana setup");
    counter_game
        .grant_mana(PlayerId(0), Color::Green, 2)
        .expect("green mana setup");
    counter_game.clear_event_log();
    counter_game
        .cast_spell_with_mana_spend(
            PlayerId(0),
            CastRequest {
                card: vigor,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            ManaPaymentSelection {
                generic: vec![Color::Green, Color::Green],
                hybrid: vec![],
            },
        )
        .expect("Vigor Mortis casts");
    counter_game
        .pass_priority(PlayerId(0))
        .expect("caster passes");
    counter_game
        .pass_priority(PlayerId(1))
        .expect("Vigor Mortis resolves");

    assert_eq!(
        counter_game
            .object(target)
            .expect("target remains live")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&2)
    );
    assert!(counter_game.event_log.windows(2).any(|events| {
        matches!(
            events,
            [
                GameEvent::ReplacementEffectApplied {
                    source,
                    affected_player: PlayerId(0),
                    event: ReplacementEventKind::CounterPlacement {
                        counter: CounterKind::PlusOnePlusOne,
                    },
                    original_amount: 1,
                    replacement_amount: 2,
                },
                GameEvent::CounterPlaced {
                    card,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 2,
                    ..
                },
            ] if *source == season && *card == target
        )
    }));
    println!(
        "Doubling Season trace: {:?}",
        counter_game.canonical_event_log()
    );
    counter_game
        .validate_invariants()
        .expect("counter replacement preserves invariants");
}

#[test]
fn replacement_source_stops_applying_after_its_battlefield_lifecycle_ends() {
    let mut game = game_with_replacements();
    let season = game
        .put_on_battlefield(PlayerId(0), "RAV-DOUBLING-SEASON")
        .expect("replacement source setup");
    let seed_spark = game
        .add_card(PlayerId(1), "RAV-SEED-SPARK", Zone::Hand)
        .expect("removal spell setup");
    game.grant_mana(PlayerId(1), Color::White, 1)
        .expect("removal spell mana setup");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("removal spell generic mana setup");
    game.clear_event_log();
    game.pass_priority(PlayerId(0))
        .expect("source controller passes to opponent");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: seed_spark,
            targets: vec![Target::Permanent(season)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seed Spark targets the replacement source");
    game.pass_priority(PlayerId(1))
        .expect("caster passes response");
    game.pass_priority(PlayerId(0))
        .expect("Seed Spark resolves");

    assert_eq!(game.zone_of(season), Some(Zone::Graveyard));
    assert_eq!(token_count(&game, PlayerId(1)), 2);
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ReplacementEffectApplied { source, .. } if *source == season
        )),
        "the destroyed source cannot modify the later token instruction"
    );
    game.validate_invariants()
        .expect("replacement-source departure preserves invariants");
}
