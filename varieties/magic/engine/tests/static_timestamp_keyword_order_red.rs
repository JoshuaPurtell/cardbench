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
    StaticContinuousEffectBinding, Target, Zone,
};

const TARGET: &str = "TST-TIMESTAMP-TARGET";
const LATER_STATIC: &str = "TST-LATER-STATIC-FLYING";
const BOUNCE: &str = "TST-STATIC-SOURCE-BOUNCE";
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

fn bounce() -> CardDefinition {
    CardDefinition {
        id: BOUNCE,
        name: BOUNCE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["same-layer-timestamp-order-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ReturnControlledCreatureToHand],
    }
}

fn game() -> Game {
    Game::new_with_all_bindings_and_static_continuous_effects(
        [creature(TARGET), creature(LATER_STATIC), bounce()],
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
    .expect("timestamp fixture initializes")
}

fn pass_pair(game: &mut Game, active: PlayerId, opponent: PlayerId) {
    game.submit_policy_move(active, POLICY, PolicyAction::PassPriority)
        .expect("active policy passes priority");
    game.submit_policy_move(opponent, POLICY, PolicyAction::PassPriority)
        .expect("opponent policy passes and resolves the LIFO top");
    game.validate_invariants()
        .expect("each resolution boundary remains invariant-valid");
}

fn activate_remove_flying(
    game: &mut Game,
    active: PlayerId,
    target: cardbench_magic_engine::ObjectId,
) {
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
}

#[test]
fn later_static_keyword_grant_overrides_earlier_timestamped_removal() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let target = game
        .put_on_battlefield(active, TARGET)
        .expect("ability source starts on the battlefield");
    let static_source = game
        .add_card(active, LATER_STATIC, Zone::Hand)
        .expect("later static source starts in hand");

    activate_remove_flying(&mut game, active, target);
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

#[test]
fn static_source_reentry_retires_the_old_timestamp_and_allocates_a_new_one() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let static_source = game
        .put_on_battlefield(active, LATER_STATIC)
        .expect("older static source starts on the battlefield");
    let target = game
        .put_on_battlefield(active, TARGET)
        .expect("ability source starts on the battlefield");
    let bounce = game
        .add_card(active, BOUNCE, Zone::Hand)
        .expect("bounce spell starts in hand");
    let initial_static_incarnation = game
        .object(static_source)
        .expect("static source exists")
        .incarnation;

    assert!(
        game.characteristics(target)
            .expect("initial target characteristics")
            .keywords
            .contains(&Keyword::Flying),
        "the initial static grant is live"
    );
    activate_remove_flying(&mut game, active, target);
    pass_pair(&mut game, active, opponent);
    assert!(
        !game
            .characteristics(target)
            .expect("target after later removal")
            .keywords
            .contains(&Keyword::Flying),
        "the later timestamped removal beats the older static grant"
    );

    game.submit_policy_move(
        active,
        POLICY,
        PolicyAction::Cast(CastRequest {
            card: bounce,
            targets: vec![Target::Permanent(static_source)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("bounce legally targets the controlled static creature");
    pass_pair(&mut game, active, opponent);
    assert_eq!(game.zone_of(static_source), Some(Zone::Hand));
    let hand_incarnation = game
        .object(static_source)
        .expect("bounced source remains addressable")
        .incarnation;
    assert!(hand_incarnation > initial_static_incarnation);
    assert!(
        !game
            .characteristics(target)
            .expect("target after static source leaves")
            .keywords
            .contains(&Keyword::Flying)
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
    .expect("the same physical static source is legally recast");
    pass_pair(&mut game, active, opponent);
    let reentry_incarnation = game
        .object(static_source)
        .expect("reentered source exists")
        .incarnation;
    assert!(reentry_incarnation > hand_incarnation);
    assert_eq!(game.zone_of(static_source), Some(Zone::Battlefield));
    assert!(
        game.characteristics(target)
            .expect("target after static source reentry")
            .keywords
            .contains(&Keyword::Flying),
        "the reentered static source has a fresh, later timestamp"
    );
    assert_eq!(game.priority, active);
    assert!(game.stack.is_empty());
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, cardbench_magic_engine::GameEvent::SpellCast { .. }))
            .count(),
        2,
        "bounce and recast each have one cast receipt"
    );
    game.validate_invariants()
        .expect("static timestamp retirement and reentry remain auditable");
}
