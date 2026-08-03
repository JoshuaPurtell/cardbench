//! Red regression: static and timestamped layer-six effects share one
//! chronological timestamp order.
//!
//! A creature first resolves an ability that removes Flying from itself until
//! end of turn. A later creature spell then enters with a static ability that
//! grants Flying to its controller's creatures. The later static timestamp
//! must win; grouping all static effects ahead of all timestamped effects
//! incorrectly leaves the earlier removal last.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, ContinuousChange, Effect, Game, Keyword, ManaCost, PlayerId, PolicyAction,
    StaticContinuousEffectBinding, Zone,
};

const TARGET: &str = "TST-TIMESTAMP-TARGET";
const LATER_STATIC: &str = "TST-LATER-STATIC-FLYING";
const REMOVE_FLYING: &str = "remove-flying-until-end-of-turn";
const POLICY: &str = "adversarial.static-timestamp.v1";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["same-layer-timestamp-order-probe"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game, active: PlayerId, opponent: PlayerId) {
    game.submit_policy_move(active, POLICY, PolicyAction::PassPriority)
        .expect("active policy passes priority");
    game.submit_policy_move(opponent, POLICY, PolicyAction::PassPriority)
        .expect("opponent policy passes and resolves the LIFO top");
    game.validate_invariants()
        .expect("each resolution boundary remains invariant-valid");
}

#[test]
fn later_static_keyword_grant_overrides_earlier_timestamped_removal() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        [creature(TARGET), creature(LATER_STATIC)],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: TARGET,
            ability: ActivatedAbility {
                id: REMOVE_FLYING,
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RemoveSourceKeywordUntilEndOfTurn {
                    keyword: Keyword::Flying,
                }],
            },
        }],
        [StaticContinuousEffectBinding {
            card_definition: LATER_STATIC,
            change: ContinuousChange::ControlledCreaturesAddKeyword(Keyword::Flying),
        }],
    )
    .expect("timestamp fixture initializes");
    let target = game
        .put_on_battlefield(active, TARGET)
        .expect("ability source starts on the battlefield");
    let static_source = game
        .add_card(active, LATER_STATIC, Zone::Hand)
        .expect("later static source starts in hand");

    game.submit_policy_move(
        active,
        POLICY,
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: target,
                ability_id: REMOVE_FLYING,
                targets: vec![],
                additional_tap_creatures: vec![],
                sacrifice_sources: vec![],
                discard_cards: vec![],
            },
        },
    )
    .expect("remove-Flying ability is legally activated");
    assert_eq!(game.priority, active, "the activator retains priority");
    assert_eq!(game.stack.len(), 1);
    pass_pair(&mut game, active, opponent);
    assert!(
        !game
            .characteristics(target)
            .expect("target characteristics after removal")
            .keywords
            .contains(&Keyword::Flying),
        "the earlier removal is live before the static source enters"
    );

    game.submit_policy_move(
        active,
        POLICY,
        PolicyAction::Cast(CastRequest {
            card: static_source,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the later static creature is legally cast");
    assert_eq!(game.priority, active, "the caster retains priority");
    assert_eq!(game.stack.len(), 1);
    pass_pair(&mut game, active, opponent);

    let target_characteristics = game
        .characteristics(target)
        .expect("target remains on the battlefield");
    let source_characteristics = game
        .characteristics(static_source)
        .expect("static source remains on the battlefield");
    eprintln!(
        "same-layer timestamp trace: target={target_characteristics:?}; source={source_characteristics:?}; events={:?}",
        game.canonical_event_log(),
    );
    assert_eq!(game.zone_of(static_source), Some(Zone::Battlefield));
    assert!(
        source_characteristics.keywords.contains(&Keyword::Flying),
        "the static source grants Flying to itself"
    );
    assert!(
        target_characteristics.keywords.contains(&Keyword::Flying),
        "the later static layer-six grant must apply after the earlier removal"
    );
    game.validate_invariants()
        .expect("the final layered state remains auditable");
}
