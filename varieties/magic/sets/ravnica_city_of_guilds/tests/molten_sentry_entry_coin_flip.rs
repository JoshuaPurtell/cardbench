//! Full behavior contract for Molten Sentry's entry-time coin flip.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_entry_coin_flip_bindings,
    run_public_scenario,
};

fn resolve_sentry(
    seed: u64,
) -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .expect("RAV entry coin-flip binding registers");
    game.set_entry_coin_flip_seed(seed)
        .expect("coin-flip seed is setup-only");
    let sentry = game
        .add_card(PlayerId(0), "RAV-MOLTEN-SENTRY", Zone::Hand)
        .expect("Molten Sentry enters the fixture hand");
    let copy_target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("copy target enters the fixture battlefield");
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player advances the turn");
        game.pass_priority(PlayerId(1))
            .expect("opponent advances the turn");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    game.add_mana_from_action(PlayerId(0), Color::Red, 4)
        .expect("red payment action");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sentry,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Molten Sentry casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes on Molten Sentry");
    game.pass_priority(PlayerId(1))
        .expect("Molten Sentry resolves");
    (game, sentry, copy_target)
}

#[test]
fn molten_sentry_definition_binds_the_complete_entry_coin_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOLTEN-SENTRY")
        .expect("Molten Sentry definition exists");
    assert_eq!(definition.name, "Molten Sentry");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(definition.keywords.is_empty());
    assert!(definition.effects.is_empty());
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "entry-coin-flip-copiable-characteristics",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn molten_sentry_entry_coin_flip_selects_and_records_each_copiable_shape() {
    let (mut heads_game, heads_sentry, copy_target) = resolve_sentry(0);
    let heads = heads_game
        .characteristics(heads_sentry)
        .expect("heads result characteristics");
    assert_eq!((heads.power, heads.toughness), (Some(5), Some(2)));
    assert_eq!(heads.keywords, vec![Keyword::Haste]);
    assert!(heads_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentEntryCoinFlipped {
            permanent,
            source_definition,
            heads: true,
            ..
        } if *permanent == heads_sentry && *source_definition == "RAV-MOLTEN-SENTRY"
    )));
    heads_game
        .copy_permanent(copy_target, heads_sentry)
        .expect("a later copy inherits the selected heads values");
    let copied_heads = heads_game
        .characteristics(copy_target)
        .expect("copied heads characteristics");
    assert_eq!(
        (copied_heads.power, copied_heads.toughness),
        (Some(5), Some(2))
    );
    assert_eq!(copied_heads.keywords, vec![Keyword::Haste]);
    assert_eq!(
        heads_game
            .event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::PermanentEntryCoinFlipped { .. }))
            .count(),
        1,
        "copying the selected permanent must not consume a second coin flip"
    );
    heads_game
        .validate_invariants()
        .expect("heads entry result preserves invariants");

    let (tails_game, tails_sentry, _) = resolve_sentry(1);
    let tails = tails_game
        .characteristics(tails_sentry)
        .expect("tails result characteristics");
    assert_eq!((tails.power, tails.toughness), (Some(2), Some(5)));
    assert_eq!(tails.keywords, vec![Keyword::Defender]);
    assert!(tails_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentEntryCoinFlipped {
            permanent,
            source_definition,
            heads: false,
            ..
        } if *permanent == tails_sentry && *source_definition == "RAV-MOLTEN-SENTRY"
    )));
    tails_game
        .validate_invariants()
        .expect("tails entry result preserves invariants");
}

#[test]
fn molten_sentry_public_scenario_replays_the_seeded_heads_result() {
    let trace = run_public_scenario("rav_molten_sentry_entry_coin_flip")
        .expect("Molten Sentry public scenario executes");
    assert_eq!(trace.digest, "fnv1a64:f938dd3eb8717ddb");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("PermanentEntryCoinFlipped"))
    );
}
