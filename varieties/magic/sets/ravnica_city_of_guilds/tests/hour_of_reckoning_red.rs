//! Red discovery contract for the White source-lane Hour of Reckoning path.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, Game, Keyword, ManaCost,
    PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hour_of_reckoning_requires_convoke_and_global_nontoken_creature_destruction() {
    let hour = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HOUR-OF-RECKONING")
        .expect("Hour of Reckoning definition exists");
    assert_eq!(
        hour.mana_cost,
        ManaCost::with_colors(4, [Color::White, Color::White, Color::White])
    );
    assert_eq!(hour.card_types, [CardType::Sorcery].into());
    assert_eq!(hour.keywords, [Keyword::Convoke]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&hour.id));
    assert!(
        hour.supported_rules
            .contains(&"destroy-all-nontoken-creatures")
    );
}

#[test]
fn hour_of_reckoning_snapshots_nontoken_creatures_and_preserves_tokens() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let hour = game
        .add_card(PlayerId(0), "RAV-HOUR-OF-RECKONING", Zone::Hand)
        .expect("Hour is in hand");
    let scatter = game
        .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .expect("Scatter is in hand");
    let contributors = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                .expect("non-token convoke contributor enters")
        })
        .collect::<Vec<_>>();
    let opposing_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opposing non-token creature enters");
    game.grant_mana(PlayerId(0), Color::Red, 2)
        .expect("fixture supplies Scatter generic payment");
    game.grant_mana(PlayerId(0), Color::White, 7)
        .expect("fixture supplies Hour payment");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: scatter,
            targets: vec![],
            convoke: contributors
                .iter()
                .enumerate()
                .map(|(index, creature)| ConvokePayment {
                    creature: *creature,
                    contribution: if index < 2 {
                        ConvokeContribution::Color(Color::Green)
                    } else {
                        ConvokeContribution::Generic
                    },
                })
                .collect(),
            payment_mana_abilities: vec![],
        },
    )
    .expect("Scatter convoke payment is legal");
    game.pass_priority(PlayerId(0))
        .expect("Scatter caster passes");
    game.pass_priority(PlayerId(1)).expect("Scatter resolves");
    let tokens_before_hour = game
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
    assert_eq!(tokens_before_hour.len(), 3, "Scatter created typed tokens");

    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: hour,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("target-free Hour cast is legal");
    game.pass_priority(PlayerId(0)).expect("Hour caster passes");
    game.pass_priority(PlayerId(1)).expect("Hour resolves");

    for creature in contributors.into_iter().chain([opposing_creature]) {
        assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    }
    for token in tokens_before_hour {
        assert_eq!(game.zone_of(token), Some(Zone::Battlefield));
    }
    let events = game.canonical_event_log();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.contains("CardDestroyed"))
            .count(),
        4,
        "the pre-resolution non-token snapshot receives four destroy instructions"
    );
    let resolved = events
        .iter()
        .position(|event| event.contains("SpellResolved"))
        .expect("Hour resolution receipt exists");
    assert!(
        events[..resolved]
            .iter()
            .filter(|event| event.contains("CardDestroyed"))
            .count()
            == 4,
        "all snapshot destructions precede the terminal resolution receipt"
    );
    assert!(
        events
            .iter()
            .all(|event| !event.contains("TokenCeasedToExist")),
        "Hour does not apply destruction instructions to tokens"
    );
    game.validate_invariants()
        .expect("Hour sweep preserves state-machine invariants");
}
