//! Red regression for static controller-relative damage prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Game, GameEvent, Keyword,
    ManaCost, PlayerId, StaticContinuousEffectBinding, Target, TargetRequirement, Zone,
};

const LIGHT: &str = "TST-STATIC-FRIENDLY-PREVENTION";
const CREATURE: &str = "TST-STATIC-FRIENDLY-CREATURE";
const DAMAGE: &str = "TST-STATIC-FRIENDLY-DAMAGE";

fn definition(
    id: &'static str,
    card_type: CardType,
    colors: BTreeSet<Color>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<cardbench_magic_engine::Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["static-friendly-source-prevention"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // Both controller branches share one state-machine trace.
fn static_friendly_source_prevention_blocks_only_same_controller_damage() {
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        vec![
            definition(
                LIGHT,
                CardType::Enchantment,
                BTreeSet::from([Color::White]),
                None,
                None,
                vec![],
            ),
            definition(
                CREATURE,
                CardType::Creature,
                BTreeSet::from([Color::White]),
                Some(3),
                Some(4),
                vec![],
            ),
            definition(
                DAMAGE,
                CardType::Instant,
                BTreeSet::from([Color::Red]),
                None,
                None,
                vec![cardbench_magic_engine::Effect::DealDamage {
                    amount: 3,
                    target: TargetRequirement::Creature,
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [StaticContinuousEffectBinding {
            card_definition: LIGHT,
            change: ContinuousChange::ControlledCreaturesAddKeyword(
                Keyword::PreventDamageFromControlledSources,
            ),
        }],
    )
    .expect("synthetic fixture builds");
    game.put_on_battlefield(PlayerId(0), LIGHT)
        .expect("static prevention source setup");
    let creature = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("friendly creature setup");
    let damage = game
        .add_card(PlayerId(0), DAMAGE, Zone::Hand)
        .expect("friendly damage source setup");
    let opponent_damage = game
        .add_card(PlayerId(1), DAMAGE, Zone::Hand)
        .expect("opposing damage source setup");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("pre-game payment setup");
    game.grant_mana(PlayerId(1), Color::Red, 1)
        .expect("opponent pre-game payment setup");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: damage,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("friendly damage spell casts");
    pass_pair(&mut game);

    assert_eq!(
        game.object(creature)
            .expect("creature remains present")
            .damage,
        0,
        "a controller's source cannot damage that controller's creature"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(target),
            amount: 3,
        } if *source == damage && *target == creature
    )));

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: opponent_damage,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opposing damage spell casts");
    pass_pair(&mut game);
    assert_eq!(
        game.object(creature)
            .expect("creature remains present")
            .damage,
        3,
        "the static rule must not prevent damage from an opposing controller"
    );
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented { source, .. } if *source == opponent_damage
    )));
    eprintln!(
        "static friendly-source prevention trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("static friendly-source prevention remains invariant-valid");
}
