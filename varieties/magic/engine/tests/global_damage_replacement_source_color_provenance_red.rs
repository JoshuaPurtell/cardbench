//! Red regression: a suspended global-damage replacement decision must retain
//! the source colors captured by its resolving stack object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, ContinuousChange, DamageReplacementChoice, DamageReplacementEffect,
    DamageReplacementEffectBinding, Duration, Effect, Game, Keyword, ManaCost, PlayerId,
    ReplacementChoice, Target, Zone,
};

const COLOR_GRANTER: &str = "TST-GLOBAL-COLOR-GRANTER";
const PINGER: &str = "TST-GLOBAL-COLOR-PINGER";
const TARGET: &str = "TST-GLOBAL-COLOR-TARGET";
const HALVER: &str = "TST-GLOBAL-COLOR-HALVER";
const KILL: &str = "TST-GLOBAL-COLOR-KILL";
const PROTECTION: &str = "TST-GLOBAL-COLOR-PROTECTION";
const SHIELD: &str = "TST-GLOBAL-COLOR-SHIELD";

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
        supported_rules: &["global-damage-source-color-replacement-red"],
        power: Some(4),
        toughness: Some(4),
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
        supported_rules: &["global-damage-source-color-replacement-red"],
        power: None,
        toughness: None,
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
fn global_damage_replacement_uses_the_departed_sources_stack_colors() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            creature(COLOR_GRANTER, BTreeSet::from([Color::White]), vec![]),
            creature(PINGER, BTreeSet::new(), vec![]),
            creature(TARGET, BTreeSet::from([Color::White]), vec![]),
            CardDefinition {
                id: HALVER,
                name: HALVER,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::new(),
                mana_colors: BTreeSet::new(),
                card_types: BTreeSet::from([CardType::Enchantment]),
                is_basic_land: false,
                supported_rules: &["global-damage-source-color-replacement-red"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![],
            },
            instant(KILL, vec![Effect::DestroyTargetNonblackCreature]),
            instant(
                PROTECTION,
                vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::Protection(Color::Red),
                }],
            ),
            instant(
                SHIELD,
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 2 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: PINGER,
            ability: ActivatedAbility {
                id: "global-ping",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 2 }],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_damage_replacement_effect_bindings([DamageReplacementEffectBinding {
        source_definition: HALVER,
        effect: DamageReplacementEffect::HalveDamage,
    }])
    .expect("halver binding registers");
    let granter = game
        .put_on_battlefield(controller, COLOR_GRANTER)
        .expect("granter enters");
    let pinger = game
        .put_on_battlefield(controller, PINGER)
        .expect("pinger enters");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target enters");
    game.put_on_battlefield(controller, HALVER)
        .expect("halver enters");
    let kill = game
        .add_card(controller, KILL, Zone::Hand)
        .expect("kill enters hand");
    let protection = game
        .add_card(opponent, PROTECTION, Zone::Hand)
        .expect("protection enters hand");
    let shield = game
        .add_card(opponent, SHIELD, Zone::Hand)
        .expect("shield enters hand");
    game.begin_game().expect("game begins");

    game.add_continuous_effect(
        granter,
        pinger,
        ContinuousChange::AddColor(Color::Red),
        Duration::Permanent,
    )
    .expect("pinger becomes red");
    game.activate_ability(
        controller,
        AbilityActivation {
            source: pinger,
            ability_id: "global-ping",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("red global ability activates");

    game.pass_priority(controller)
        .expect("controller passes to the shield");
    game.cast_spell(
        opponent,
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent creates a damage shield");
    pass_pair(&mut game);

    game.pass_priority(controller)
        .expect("controller passes to protection");
    game.cast_spell(
        opponent,
        CastRequest {
            card: protection,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent grants protection from red");
    game.pass_priority(opponent)
        .expect("opponent passes back for the response");
    game.cast_spell(
        controller,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(granter)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controller removes the color-grant source");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(granter), Some(Zone::Graveyard));
    assert!(
        !game
            .characteristics(pinger)
            .expect("pinger remains")
            .colors
            .contains(&Color::Red),
        "the live source is no longer red"
    );
    pass_pair(&mut game);
    assert!(
        game.characteristics(target)
            .expect("target remains")
            .keywords
            .contains(&Keyword::Protection(Color::Red))
    );
    pass_pair(&mut game);

    let decision = game
        .view_for_player(opponent)
        .expect("opponent view")
        .pending_decision
        .expect("global replacement decision opens for the protected creature");
    eprintln!(
        "global source-color replacement red trace: options={:?}; stack={:?}; events={:?}",
        decision.replacement_candidates,
        game.stack,
        game.canonical_event_log(),
    );
    assert!(
        decision
            .replacement_candidates
            .contains(&ReplacementChoice::Damage(
                DamageReplacementChoice::SourceColorPrevention { permanent: target },
            )),
        "the decision must retain the departed source's activation-time red color"
    );
    game.validate_invariants()
        .expect("source-color replacement decision state remains auditable");
}
