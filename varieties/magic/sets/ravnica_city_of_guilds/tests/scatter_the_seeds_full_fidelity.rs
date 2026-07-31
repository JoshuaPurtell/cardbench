//! Direct, ability-complete contract for the RAV Scatter the Seeds definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, CreatureSubtype, Effect,
    Game, Keyword, ManaCost, PlayerId, TokenSpec, Zone,
};
use cardbench_magic_rav::{card_definitions, run_all_scenarios};

#[test]
fn scatter_the_seeds_has_complete_convoke_and_typed_saproling_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCATTER-THE-SEEDS")
        .expect("Scatter the Seeds definition exists");

    assert_eq!(definition.supported_rules[0], "full-rules-fidelity");
    assert_eq!(
        definition.supported_rules,
        ["full-rules-fidelity", "convoke", "token-creation"]
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::Green])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Green]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!(definition.keywords, [Keyword::Convoke]);
    assert_eq!(
        definition.effects,
        vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 3,
        }]
    );

    let Effect::CreateToken { token, count } = &definition.effects[0] else {
        panic!("Scatter must create typed tokens");
    };
    assert_eq!(*count, 3);
    assert_eq!(token.name, "Saproling");
    assert_eq!(token.colors, BTreeSet::from([Color::Green]));
    assert_eq!(token.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(
        token.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Saproling])
    );
    assert_eq!((token.power, token.toughness), (1, 1));
}

#[test]
fn scatter_the_seeds_direct_trace_creates_three_typed_tokens_before_resolution_receipt() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let scatter = game
        .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .expect("Scatter in hand");
    let first = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
        .expect("first contributor");
    let second = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
        .expect("second contributor");
    let third = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
        .expect("third contributor");
    game.grant_mana(PlayerId(0), Color::Red, 2)
        .expect("fixture supplies generic payment");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
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
        },
    )
    .expect("convoke payment is legal");
    game.pass_priority(PlayerId(0))
        .expect("caster passes priority");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and spell resolves");

    let token_ids = game
        .player(PlayerId(0))
        .expect("controller exists")
        .battlefield
        .iter()
        .copied()
        .filter(|card| {
            game.object(*card)
                .is_ok_and(|object| object.token.is_some())
        })
        .collect::<Vec<_>>();
    assert_eq!(token_ids.len(), 3);
    for token in token_ids {
        let characteristics = game
            .characteristics(token)
            .expect("created token has characteristics");
        assert_eq!(characteristics.colors, BTreeSet::from([Color::Green]));
        assert_eq!(
            characteristics.card_types,
            BTreeSet::from([CardType::Creature])
        );
        assert_eq!(
            characteristics.creature_subtypes,
            BTreeSet::from([CreatureSubtype::Saproling])
        );
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(1))
        );
    }

    let events = game.canonical_event_log();
    assert_eq!(events.len(), 11);
    assert!(
        events[0..3]
            .iter()
            .all(|event| event.contains("ConvokeUsed"))
    );
    assert!(events[3].contains("SpellCast"));
    assert!(events[4].contains("PriorityPassed"));
    assert!(events[5].contains("PriorityPassed"));
    assert!(
        events[6..9]
            .iter()
            .all(|event| event.contains("TokenCreated"))
    );
    assert!(events[9].contains("SpellResolved"));
    assert!(events[10].contains("CardMoved"));
    game.validate_invariants()
        .expect("direct Scatter trace preserves engine invariants");
}

#[test]
fn scatter_the_seeds_public_trace_has_the_fixed_complete_receipt_digest() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios run")
        .into_iter()
        .find(|result| result.id == "rav_convoke_scatter_the_seeds")
        .expect("Scatter public scenario exists");
    assert_eq!(scenario.digest, "fnv1a64:964c69b67173b214");
    assert_eq!(
        scenario
            .event_log
            .iter()
            .filter(|event| event.contains("TokenCreated"))
            .count(),
        3
    );
}
