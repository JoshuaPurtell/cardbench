//! Red regression: an activated damage source keeps deathtouch from its
//! battlefield incarnation when it leaves before its ability resolves.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, BasicLandManaAbilityActivation,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, CastPaymentManaAbility,
    CastRequest, Color, ContinuousChange, Duration, Effect, Game, Keyword, ManaCost,
    ManaPaymentSelection, PlayerId, Target, TargetRequirement, Zone,
};

const PINGER: &str = "TST-DEATHTOUCH-LKI-PINGER";
const TARGET: &str = "TST-DEATHTOUCH-LKI-TARGET";
const KILL: &str = "TST-DEATHTOUCH-LKI-KILL";
const RETURN: &str = "TST-DEATHTOUCH-LKI-RETURN";
const FOREST: &str = "TST-DEATHTOUCH-LKI-FOREST";

fn creature(id: &'static str, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["deathtouch-source-lki-red"],
        power: Some(1),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // One response window owns this complete LKI/SBA causal trace.
fn departed_activated_damage_source_uses_deathtouch_lki() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let kill_definition = CardDefinition {
        id: KILL,
        name: KILL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["deathtouch-source-lki-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DestroyTargetNonblackCreature],
    };
    let mut game = Game::new_with_all_bindings(
        [creature(PINGER, 1), creature(TARGET, 3), kill_definition],
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
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    let pinger = game
        .put_on_battlefield(controller, PINGER)
        .expect("pinger enters");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target enters");
    let kill = game
        .add_card(opponent, KILL, Zone::Hand)
        .expect("kill enters hand");
    game.begin_game().expect("game begins");

    game.add_continuous_effect(
        pinger,
        pinger,
        ContinuousChange::AddKeyword(Keyword::Deathtouch),
        Duration::Permanent,
    )
    .expect("pinger gains deathtouch while live");
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
    .expect("deathtouch pinger activates");
    game.pass_priority(controller)
        .expect("controller passes to response");
    game.cast_spell(
        opponent,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent destroys pinger in response");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(pinger), Some(Zone::Graveyard));
    assert!(
        !game
            .characteristics(pinger)
            .expect("departed pinger has base values")
            .keywords
            .contains(&Keyword::Deathtouch),
        "the departed source's current graveyard characteristics have no deathtouch"
    );

    pass_pair(&mut game);
    eprintln!(
        "deathtouch LKI red trace: zones source={:?} target={:?}; events={:?}",
        game.zone_of(pinger),
        game.zone_of(target),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.zone_of(target),
        Some(Zone::Graveyard),
        "one point from the departed source's deathtouch incarnation must destroy the target"
    );
    game.validate_invariants()
        .expect("deathtouch source LKI remains invariant-valid");
}

#[test]
#[allow(clippy::too_many_lines)] // One leave-return response window owns this incarnation/LKI trace.
fn returned_source_cannot_replace_its_old_deathtouch_damage_identity() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let kill_definition = CardDefinition {
        id: KILL,
        name: KILL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["deathtouch-source-incarnation-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DestroyTargetNonblackCreature],
    };
    let return_definition = CardDefinition {
        id: RETURN,
        name: RETURN,
        set_code: "TST",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["deathtouch-source-incarnation-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
            color: Color::Red,
        }],
    };
    let forest_definition = CardDefinition {
        id: FOREST,
        name: FOREST,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Green]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["deathtouch-source-incarnation-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    };
    let mut game = Game::new_with_all_bindings(
        [
            creature(PINGER, 1),
            creature(TARGET, 3),
            kill_definition,
            return_definition,
            forest_definition,
        ],
        2,
        [],
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
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
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    let pinger = game
        .put_on_battlefield(controller, PINGER)
        .expect("pinger enters");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target enters");
    let forest = game
        .put_on_battlefield(controller, FOREST)
        .expect("forest enters");
    let kill = game
        .add_card(opponent, KILL, Zone::Hand)
        .expect("kill enters hand");
    let return_spell = game
        .add_card(controller, RETURN, Zone::Hand)
        .expect("return enters hand");
    game.begin_game().expect("game begins");
    let old_incarnation = game.object(pinger).expect("pinger exists").incarnation;

    game.add_continuous_effect(
        pinger,
        pinger,
        ContinuousChange::AddKeyword(Keyword::Deathtouch),
        Duration::Permanent,
    )
    .expect("old source gains deathtouch");
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
    .expect("old source activates");
    game.pass_priority(controller)
        .expect("controller passes to response");
    game.cast_spell(
        opponent,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent destroys the old source");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(pinger), Some(Zone::Graveyard));

    game.cast_spell_with_mana_spend(
        controller,
        CastRequest {
            card: return_spell,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![CastPaymentManaAbility::BasicLand(
                BasicLandManaAbilityActivation {
                    land: forest,
                    color: Color::Green,
                },
            )],
        },
        ManaPaymentSelection {
            generic: vec![Color::Green],
            hybrid: vec![],
        },
    )
    .expect("controller returns a new pinger incarnation before the old ability resolves");
    pass_pair(&mut game);
    let returned_incarnation = game.object(pinger).expect("pinger returns").incarnation;
    assert_eq!(game.zone_of(pinger), Some(Zone::Battlefield));
    assert!(returned_incarnation > old_incarnation);
    assert!(
        !game
            .characteristics(pinger)
            .expect("new source has live characteristics")
            .keywords
            .contains(&Keyword::Deathtouch),
        "the returned object is not the old deathtouch incarnation"
    );

    pass_pair(&mut game);
    eprintln!(
        "deathtouch source incarnation red trace: old={old_incarnation}; returned={returned_incarnation}; target={:?}; events={:?}",
        game.zone_of(target),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.zone_of(target),
        Some(Zone::Graveyard),
        "the pending ability must use its original source incarnation, never the returned object"
    );
    game.validate_invariants()
        .expect("returned source cannot corrupt old-damage provenance");
}
