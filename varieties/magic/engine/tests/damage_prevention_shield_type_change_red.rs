//! Red regression: a resolved damage-prevention shield remains attached to
//! its exact permanent even if a later layer-one copy makes it noncreature.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const COPY_SOURCE: &str = "TST-SHIELD-TYPE-CHANGE-COPY-SOURCE";
const TARGET: &str = "TST-SHIELD-TYPE-CHANGE-TARGET";
const SHIELD: &str = "TST-SHIELD-TYPE-CHANGE-SHIELD";
const BOLT: &str = "TST-SHIELD-TYPE-CHANGE-BOLT";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["damage-prevention-shield-type-change-red"],
        power: (id == TARGET).then_some(4),
        toughness: (id == TARGET).then_some(4),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn shielded_creature_can_become_a_noncreature_without_losing_its_shield() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(COPY_SOURCE, BTreeSet::from([CardType::Artifact]), vec![]),
            definition(
                TARGET,
                BTreeSet::from([CardType::Artifact, CardType::Creature]),
                vec![],
            ),
            definition(
                SHIELD,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Permanent,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let copy_source = game
        .put_on_battlefield(controller, COPY_SOURCE)
        .expect("artifact copy source setup");
    let target = game
        .put_on_battlefield(controller, TARGET)
        .expect("creature target setup");
    let shield = game
        .add_card(controller, SHIELD, Zone::Hand)
        .expect("shield setup");
    let bolt = game
        .add_card(opponent, BOLT, Zone::Hand)
        .expect("bolt setup");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage shield casts");
    pass_pair(&mut game);

    let copy = game.copy_permanent(target, copy_source);
    eprintln!(
        "shield type-change copy result: {copy:?}; target={:?}; events={:?}",
        game.characteristics(target),
        game.canonical_event_log(),
    );
    copy.expect("a shielded target can become a noncreature");
    assert!(
        !game
            .characteristics(target)
            .expect("target remains live")
            .card_types
            .contains(&CardType::Creature),
        "the copy snapshot makes the targeted permanent noncreature",
    );

    game.pass_priority(controller)
        .expect("controller yields bolt priority");
    game.cast_spell(
        opponent,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bolt may target the now-artifact permanent");
    pass_pair(&mut game);

    assert_eq!(
        game.object(target).expect("target remains live").damage,
        0,
        "the previously resolved shield prevents damage to the same permanent",
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamagePrevented {
                target: Target::Permanent(card),
                amount: 2,
                ..
            } if *card == target
        )),
        "the shield records its ordinary prevention receipt after the type change",
    );
    game.validate_invariants()
        .expect("the type-change replacement state remains valid");
}
