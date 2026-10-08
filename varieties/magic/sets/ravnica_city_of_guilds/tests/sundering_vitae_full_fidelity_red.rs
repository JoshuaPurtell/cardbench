use cardbench_magic_engine::{
    CardType, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{SET_CODE, card_definitions};

#[path = "../src/cards/sundering_vitae.rs"]
mod candidate_sundering_vitae;

#[test]
fn sundering_vitae_is_cataloged_and_destroys_an_artifact_or_enchantment() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUNDERING-VITAE")
        .expect("Sundering Vitae must be present in the executable catalog");
    assert_eq!(definition.set_code, SET_CODE);
    assert_eq!(definition, candidate_sundering_vitae::definition());
    assert_eq!(
        definition.colors,
        std::collections::BTreeSet::from([Color::Green])
    );
    assert_eq!(
        definition.card_types,
        std::collections::BTreeSet::from([CardType::Instant])
    );

    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-SUNDERING-VITAE", Zone::Hand)
        .expect("card is executable");
    let artifact = game
        .add_card(PlayerId(1), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("target exists");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("test mana");
    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(artifact)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast succeeds");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and spell resolves");
    assert_eq!(game.zone_of(artifact), Some(Zone::Graveyard));
    let events = game.canonical_event_log();
    let destroyed = events
        .iter()
        .position(|event| event.contains("CardDestroyed"))
        .expect("destruction receipt");
    assert!(events[destroyed + 1].contains("CardMoved"));
}

#[test]
fn sundering_vitae_convoke_pays_its_green_symbol_and_destroys_the_targeted_artifact() {
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-SUNDERING-VITAE", Zone::Hand)
        .expect("card is executable");
    let artifact = game
        .put_on_battlefield(PlayerId(1), "RAV-BOROS-SIGNET")
        .expect("artifact target exists");
    let convoker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("green-white creature can convoke the green pip");
    game.grant_mana(PlayerId(0), Color::White, 2)
        .expect("white pays the two generic symbols");

    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(artifact)],
            convoke: vec![ConvokePayment {
                creature: convoker,
                contribution: ConvokeContribution::Color(Color::Green),
            }],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Convoke supplies the green symbol while white mana pays generic cost");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ConvokeUsed {
            player: PlayerId(0),
            creature,
            contribution: Some(Color::Green),
        } if *creature == convoker
    )));
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and spell resolves");
    assert_eq!(game.zone_of(artifact), Some(Zone::Graveyard));
    game.validate_invariants().expect("destruction leaves valid zones");
}

#[test]
fn sundering_vitae_rejects_a_land_target_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-SUNDERING-VITAE", Zone::Hand)
        .expect("card is executable");
    let land = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("target exists");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("test mana");
    let result = game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(land)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(
        result.is_err(),
        "a land is not an artifact-or-enchantment target"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(land), Some(Zone::Battlefield));
    assert!(game.event_log.is_empty());
}
