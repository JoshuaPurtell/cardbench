//! Red regression for independent end-of-turn continuous effects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, ContinuousChange, Duration, Effect, Game, GameEvent,
    Keyword, ManaCost, PlayerId, Target, Zone,
};

const SOURCE: &str = "TEST-TEMPORARY-EFFECT-SOURCE";
const TARGET: &str = "TEST-TEMPORARY-EFFECT-TARGET";
const DESTROY: &str = "TEST-DESTROY-ARTIFACT";

fn definition(id: &'static str, card_types: &[CardType], effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: card_types.iter().cloned().collect(),
        is_basic_land: false,
        supported_rules: &["test-fixture"],
        power: card_types.contains(&CardType::Creature).then_some(1),
        toughness: card_types.contains(&CardType::Creature).then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn resolving_end_of_turn_effect_survives_its_battlefield_source_departure() {
    let mut game = Game::new(
        vec![
            definition(SOURCE, &[CardType::Artifact], vec![]),
            definition(TARGET, &[CardType::Creature], vec![]),
            definition(
                DESTROY,
                &[CardType::Instant],
                vec![Effect::DestroyTargetArtifact],
            ),
        ],
        2,
    )
    .expect("fixture game builds");
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source setup");
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target setup");
    let destroy = game
        .add_card(PlayerId(1), DESTROY, Zone::Hand)
        .expect("response setup");

    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddKeyword(Keyword::Flying),
        Duration::EndOfTurn(game.turn),
    )
    .expect("independent temporary effect installs");
    game.pass_priority(PlayerId(0))
        .expect("effect controller yields priority");
    game.cast_spell(
        PlayerId(1),
        cardbench_magic_engine::CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(source)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response destroys the effect's former source");
    game.pass_priority(PlayerId(1))
        .expect("responder passes");
    game.pass_priority(PlayerId(0))
        .expect("destroy spell resolves");

    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    assert!(
        game.characteristics(target)
            .expect("target remains live")
            .keywords
            .contains(&Keyword::Flying),
        "a resolved end-of-turn effect is independent of its former battlefield source"
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ContinuousEffectExpired {
                source: expired_source,
                target: expired_target,
                layer,
            } if *expired_source == source && *expired_target == target && *layer == cardbench_magic_engine::Layer::Ability
        )),
        "source departure cannot expire an independent temporary effect early"
    );
    game.validate_invariants()
        .expect("source departure preserves independent-effect invariants");
}
