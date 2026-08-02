//! Red regression: protection must retain a stack source's last-known colors.
//!
//! A source can gain a color through a continuous effect and then leave the
//! battlefield as an activation cost.  Its stack object must retain the
//! activation-time colors: a later matching protection effect makes the
//! already-chosen target illegal at resolution.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color,
    ContinuousChange, Duration, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const COLOR_GRANTER: &str = "PROTECTION-COLOR-GRANTER";
const PINGER: &str = "PROTECTION-PINGER";
const TARGET: &str = "PROTECTION-PROVENANCE-TARGET";
const KILL: &str = "PROTECTION-KILL";
const GRANT_PROTECTION: &str = "PROTECTION-GRANT";

fn creature(id: &'static str, colors: BTreeSet<Color>, keywords: Vec<Keyword>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["protection-source-color-provenance-probe"],
        power: Some(2),
        toughness: Some(2),
        keywords,
        effects: vec![],
    }
}

fn instant(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["protection-source-color-provenance-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            creature(COLOR_GRANTER, BTreeSet::from([Color::White]), vec![]),
            creature(PINGER, BTreeSet::new(), vec![]),
            creature(TARGET, BTreeSet::from([Color::White]), vec![]),
            instant(KILL, vec![Effect::DestroyTargetNonblackCreature]),
            instant(
                GRANT_PROTECTION,
                vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::Protection(Color::Red),
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: PINGER,
            ability: ActivatedAbility {
                id: "ping",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
            },
        }],
    )
    .expect("fixture game initializes")
}

#[test]
fn departed_colored_source_cannot_target_later_matching_protection() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let granter = game
        .put_on_battlefield(controller, COLOR_GRANTER)
        .expect("color-grant source enters");
    let pinger = game
        .put_on_battlefield(controller, PINGER)
        .expect("colorless pinger enters");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target creature enters");
    let kill = game
        .add_card(controller, KILL, Zone::Hand)
        .expect("kill spell enters its controller's hand");
    let protection = game
        .add_card(opponent, GRANT_PROTECTION, Zone::Hand)
        .expect("protection spell enters its controller's hand");

    game.add_continuous_effect(
        granter,
        pinger,
        ContinuousChange::AddColor(Color::Red),
        Duration::Permanent,
    )
    .expect("the pinger is red while it activates");
    game.activate_ability(
        controller,
        AbilityActivation {
            source: pinger,
            ability_id: "ping",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("the initially legal ability activates");
    game.pass_priority(controller)
        .expect("controller passes priority");
    game.cast_spell(
        opponent,
        cardbench_magic_engine::CastRequest {
            card: protection,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts matching protection above the ability");
    game.pass_priority(opponent)
        .expect("opponent passes priority over the protection spell");
    game.cast_spell(
        controller,
        cardbench_magic_engine::CastRequest {
            card: kill,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller destroys the color-modified source in response");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("both players pass over the destruction spell");
    }
    assert_eq!(game.zone_of(pinger), Some(Zone::Graveyard));
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("both players pass over the protection spell");
    }
    assert!(
        game.characteristics(target)
            .expect("target characteristics are readable")
            .keywords
            .contains(&Keyword::Protection(Color::Red))
    );
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("both players pass over the original ability");
    }

    eprintln!(
        "protection source-color provenance trace: damage={}; stack={:?}; events={:?}",
        game.object(target).expect("target remains known").damage,
        game.stack,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(target).expect("target remains known").damage,
        0,
        "the departed source's activation-time red color must make the later protected target illegal"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::AbilityCounteredByRules { source, ability: "ping", .. }
                if *source == pinger
        )),
        "all-illegal protected target must counter the ability by rules; events={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("a protection rules counter leaves a valid state");
}
