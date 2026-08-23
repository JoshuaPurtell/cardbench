//! Red discovery contract for two source-only White RAV spell paths.
//!
//! This keeps the source-branch reconciliation honest: the cards must first
//! exist with their typed targets before their stack effects can be claimed.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn seed_spark_requires_typed_artifact_or_enchantment_destruction_and_tokens() {
    let seed_spark = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEED-SPARK")
        .expect("Seed Spark definition exists");
    assert_eq!(
        seed_spark.mana_cost,
        ManaCost::with_colors(3, [Color::White])
    );
    assert_eq!(seed_spark.card_types, [CardType::Instant].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&seed_spark.id));
    assert!(
        seed_spark
            .supported_rules
            .contains(&"typed-artifact-or-enchantment-target")
    );
    assert!(
        seed_spark
            .supported_rules
            .contains(&"saproling-token-creation")
    );
}

#[test]
fn leave_no_trace_requires_a_typed_radiance_enchantment_destruction_path() {
    let leave_no_trace = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LEAVE-NO-TRACE")
        .expect("Leave No Trace definition exists");
    assert_eq!(
        leave_no_trace.mana_cost,
        ManaCost::with_colors(1, [Color::White])
    );
    assert_eq!(leave_no_trace.card_types, [CardType::Instant].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&leave_no_trace.id));
    assert!(
        leave_no_trace
            .supported_rules
            .contains(&"radiance-enchantment-destruction")
    );
}

#[test]
fn seed_spark_destroys_its_typed_target_then_creates_two_saprolings() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let seed_spark = game
        .add_card(PlayerId(0), "RAV-SEED-SPARK", Zone::Hand)
        .expect("Seed Spark enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("artifact target enters battlefield");
    game.grant_mana(PlayerId(0), Color::White, 4)
        .expect("Seed Spark mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: seed_spark,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seed Spark casts at an artifact");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("Seed Spark resolves");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    let saprolings = game
        .player(PlayerId(0))
        .expect("caster exists")
        .battlefield
        .iter()
        .filter(|card| {
            game.object(**card)
                .is_ok_and(|object| object.token.is_some())
        })
        .count();
    assert_eq!(saprolings, 2);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == seed_spark && *card == target
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)))
            .count(),
        2
    );
    println!("seed_spark_event_log={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Seed Spark trace preserves invariants");
}

#[test]
fn leave_no_trace_destroys_only_the_target_and_shared_color_enchantments() {
    let mut definitions = card_definitions();
    definitions.push(CardDefinition {
        id: "TEST-WHITE-ENCHANTMENT",
        name: "Test White Enchantment",
        set_code: "TEST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["test-enchantment"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    });
    let mut game = Game::new(definitions, 2).expect("RAV game builds");
    let leave_no_trace = game
        .add_card(PlayerId(0), "RAV-LEAVE-NO-TRACE", Zone::Hand)
        .expect("Leave No Trace enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-SEARING-MEDITATION")
        .expect("white enchantment target enters battlefield");
    let shared_color = game
        .put_on_battlefield(PlayerId(1), "TEST-WHITE-ENCHANTMENT")
        .expect("white-sharing enchantment enters battlefield");
    let other_color = game
        .put_on_battlefield(PlayerId(1), "RAV-BLOOD-FUNNEL")
        .expect("black enchantment enters battlefield");
    game.grant_mana(PlayerId(0), Color::White, 2)
        .expect("Leave No Trace mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: leave_no_trace,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Leave No Trace casts at an enchantment");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("Leave No Trace resolves");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(shared_color), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(other_color), Some(Zone::Battlefield));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardDestroyed { source, .. } if *source == leave_no_trace))
            .count(),
        2
    );
    println!("leave_no_trace_event_log={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Leave No Trace trace preserves invariants");
}

#[test]
fn leave_no_trace_rejects_a_non_enchantment_before_payment_or_stack_mutation() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let leave_no_trace = game
        .add_card(PlayerId(0), "RAV-LEAVE-NO-TRACE", Zone::Hand)
        .expect("Leave No Trace enters hand");
    let creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("creature enters battlefield");
    game.grant_mana(PlayerId(0), Color::White, 2)
        .expect("fixture mana");
    game.clear_event_log();

    let error = game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: leave_no_trace,
                targets: vec![Target::Permanent(creature)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect_err("non-enchantment must reject atomically");
    assert_eq!(error.to_string(), "illegal target Permanent(ObjectId(2))");
    assert_eq!(game.zone_of(leave_no_trace), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected target leaves a valid state");
}
