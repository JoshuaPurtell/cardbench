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
const PROTECTED_TARGET: &str = "PROTECTION-STATIC-TARGET";
const BLUE_BOLT: &str = "PROTECTION-BLUE-BOLT";
const COLORLESS_BOLT: &str = "PROTECTION-COLORLESS-BOLT";
const RED_AURA: &str = "PROTECTION-RED-AURA";

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
    colored_instant(id, BTreeSet::new(), effects)
}

fn colored_instant(
    id: &'static str,
    colors: BTreeSet<Color>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
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

fn cast_and_resolve(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) {
    game.cast_spell(
        player,
        cardbench_magic_engine::CastRequest {
            card,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell with a legal target casts");
    for _ in 0..2 {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("both players pass and resolve the spell");
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
#[allow(clippy::too_many_lines)] // The regression spans source departure, protection, and terminal receipts.
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

#[test]
fn protection_prevents_only_matching_colored_damage() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        vec![
            creature(
                PROTECTED_TARGET,
                BTreeSet::from([Color::White]),
                vec![Keyword::Protection(Color::Red)],
            ),
            colored_instant(
                BLUE_BOLT,
                BTreeSet::from([Color::Blue]),
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            ),
            instant(
                COLORLESS_BOLT,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            ),
        ],
        2,
    )
    .expect("damage fixture initializes");
    let protected = game
        .put_on_battlefield(opponent, PROTECTED_TARGET)
        .expect("protected creature enters");
    let blue = game
        .add_card(controller, BLUE_BOLT, Zone::Hand)
        .expect("blue bolt enters hand");
    let colorless = game
        .add_card(controller, COLORLESS_BOLT, Zone::Hand)
        .expect("colorless bolt enters hand");

    cast_and_resolve(&mut game, controller, blue, protected);
    assert_eq!(
        game.object(protected)
            .expect("protected object remains")
            .damage,
        1,
        "protection from red must not prevent blue damage"
    );
    cast_and_resolve(&mut game, controller, colorless, protected);
    assert_eq!(
        game.object(protected)
            .expect("protected object remains")
            .damage,
        2,
        "protection from red must not prevent colorless damage"
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamagePrevented { target: Target::Permanent(card), .. } if *card == protected
        )),
        "nonmatching and colorless damage must not produce a protection-prevention receipt; events={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("nonmatching-color damage preserves engine invariants");
}

#[test]
fn matching_color_aura_falls_off_when_protection_is_gained() {
    let aura_controller = PlayerId(0);
    let protected_controller = PlayerId(1);
    let mut game = Game::new(
        vec![
            creature(COLOR_GRANTER, BTreeSet::from([Color::White]), vec![]),
            creature(TARGET, BTreeSet::from([Color::White]), vec![]),
            CardDefinition {
                id: RED_AURA,
                name: RED_AURA,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::from([Color::Red]),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Enchantment]),
                is_basic_land: false,
                supported_rules: &["protection-aura-cleanup-probe"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
            },
        ],
        2,
    )
    .expect("aura fixture initializes");
    let granter = game
        .put_on_battlefield(protected_controller, COLOR_GRANTER)
        .expect("protection source enters");
    let target = game
        .put_on_battlefield(protected_controller, TARGET)
        .expect("aura target enters");
    let aura = game
        .add_card(aura_controller, RED_AURA, Zone::Hand)
        .expect("red aura enters hand");

    cast_and_resolve(&mut game, aura_controller, aura, target);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("aura remains known").attached_to,
        Some(target)
    );

    game.add_continuous_effect(
        granter,
        target,
        ContinuousChange::AddKeyword(Keyword::Protection(Color::Red)),
        Duration::Permanent,
    )
    .expect("protection becomes active and reaches an SBA fixed point");

    eprintln!(
        "protection aura cleanup trace: aura_zone={:?}; effects={:?}; events={:?}",
        game.zone_of(aura),
        game.continuous_effects,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.zone_of(aura),
        Some(Zone::Graveyard),
        "a matching-color Aura cannot remain attached after protection is gained"
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::StateBasedAction { card, reason: "Aura is not attached to a battlefield creature" }
                if *card == aura
        )),
        "SBA cleanup must explain the illegal Aura attachment; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        game.continuous_effects
            .iter()
            .all(|effect| effect.source != aura),
        "the removed Aura must not retain linked effects"
    );
    game.validate_invariants()
        .expect("matching-color Aura cleanup leaves a valid state");
}
