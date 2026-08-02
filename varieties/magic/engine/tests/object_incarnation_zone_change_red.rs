//! Red regression: every rules-relevant zone change creates a fresh incarnation.
//!
//! This is deliberately synthetic.  It proves an engine property before any
//! RAV card relies on it: a self-referential activated ability must not affect
//! a new permanent that happens to retain the same public card object id.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, BasicLandManaAbilityActivation,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, CastPaymentManaAbility,
    CastRequest, Color, Effect, Game, ManaCost, ManaPaymentSelection, PlayerId, Target, Zone,
};

const SOURCE: &str = "INCARNATION-SOURCE";
const DESTROY: &str = "INCARNATION-DESTROY";
const RETURN: &str = "INCARNATION-RETURN";
const PING: &str = "INCARNATION-PING";
const LIBRARY_CARD: &str = "INCARNATION-LIBRARY-CARD";
const FOREST: &str = "INCARNATION-FOREST";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    mana_cost: ManaCost,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["object-incarnation-zone-change-probe"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        definition(
            SOURCE,
            BTreeSet::from([CardType::Creature]),
            ManaCost::new(0),
            Some(1),
            Some(1),
            vec![],
        ),
        definition(
            DESTROY,
            BTreeSet::from([CardType::Instant]),
            ManaCost::new(0),
            None,
            None,
            vec![Effect::DestroyTargetNonblackCreature],
        ),
        definition(
            RETURN,
            BTreeSet::from([CardType::Instant]),
            ManaCost::new(1),
            None,
            None,
            vec![
                Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                    color: Color::Red,
                },
            ],
        ),
        definition(
            PING,
            BTreeSet::from([CardType::Instant]),
            ManaCost::new(0),
            None,
            None,
            vec![Effect::DealDamage {
                amount: 1,
                target: cardbench_magic_engine::TargetRequirement::Player,
            }],
        ),
        definition(
            LIBRARY_CARD,
            BTreeSet::from([CardType::Creature]),
            ManaCost::new(0),
            Some(1),
            Some(1),
            vec![],
        ),
        CardDefinition {
            id: FOREST,
            name: FOREST,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["object-incarnation-zone-change-probe"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn game() -> Game {
    Game::new_with_all_bindings(
        definitions(),
        2,
        [],
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "self-pump",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }],
            },
        }],
    )
    .expect("fixture game initializes")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first priority pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass succeeds");
}

#[test]
fn source_relative_effect_cannot_modify_a_returned_source_incarnation() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = game();
    let source = game
        .put_on_battlefield(caster, SOURCE)
        .expect("source enters the battlefield");
    let destroy = game
        .add_card(responder, DESTROY, Zone::Hand)
        .expect("destroy enters responder hand");
    let return_spell = game
        .add_card(caster, RETURN, Zone::Hand)
        .expect("return spell enters caster hand");
    let forest = game
        .put_on_battlefield(caster, FOREST)
        .expect("forest enters the battlefield");
    game.begin_game().expect("fixture game begins");

    let original_incarnation = game.object(source).expect("source exists").incarnation;
    game.activate_ability(
        caster,
        AbilityActivation {
            source,
            ability_id: "self-pump",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("self-pump is placed on the stack");
    game.pass_priority(caster)
        .expect("caster passes to responder");
    game.cast_spell(
        responder,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(source)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("destroy targets original source incarnation");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));

    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: return_spell,
            targets: vec![Target::Permanent(source)],
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
    .expect("return spell targets the source card in its graveyard");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(source), Some(Zone::Battlefield));
    let returned_incarnation = game.object(source).expect("source returned").incarnation;
    assert!(
        returned_incarnation > original_incarnation,
        "the returned permanent must be a new rules object"
    );

    pass_pair(&mut game);
    eprintln!(
        "incarnation source regression: original={original_incarnation}; returned={returned_incarnation}; source={source:?}; characteristics={:?}; events={:?}",
        game.characteristics(source),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.characteristics(source)
            .expect("returned source has characteristics")
            .power,
        Some(1),
        "the old ability must not modify the returned source incarnation"
    );
    assert!(
        !game.canonical_event_log().iter().any(|event| {
            event.contains("ContinuousEffectCreated")
                && event.contains(&format!("target: {source:?}"))
        }),
        "the old ability must not create a continuous effect on the returned incarnation"
    );
    game.validate_invariants()
        .expect("failed source-relative resolution leaves a valid state");
}

#[test]
fn entering_stack_and_leaving_library_advance_the_object_incarnation() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let ping = game
        .add_card(caster, PING, Zone::Hand)
        .expect("spell enters hand");
    let library_card = game
        .add_card(caster, LIBRARY_CARD, Zone::Library)
        .expect("card enters library");
    let hand_incarnation = game.object(ping).expect("spell exists").incarnation;
    let library_incarnation = game
        .object(library_card)
        .expect("library card exists")
        .incarnation;
    // Setup supports opening-hand draws before the game becomes live.  This
    // is still a real zone transition and must not be a special identity
    // exception merely because it is eventless fixture setup.
    game.draw_card(caster, None)
        .expect("setup opening-hand draw succeeds");
    let drawn_incarnation = game
        .object(library_card)
        .expect("drawn card remains allocated")
        .incarnation;
    game.begin_game().expect("fixture game begins");

    game.cast_spell(
        caster,
        CastRequest {
            card: ping,
            targets: vec![Target::Player(opponent)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell is cast");
    let stack_incarnation = game
        .object(ping)
        .expect("spell remains allocated")
        .incarnation;
    eprintln!(
        "incarnation transition regression: hand={hand_incarnation}; stack={stack_incarnation}; library={library_incarnation}; hand_after_draw={drawn_incarnation}; events={:?}",
        game.canonical_event_log(),
    );
    assert_eq!(
        stack_incarnation,
        hand_incarnation + 1,
        "casting moves the physical card from hand to a new stack incarnation"
    );
    assert_eq!(
        drawn_incarnation,
        library_incarnation + 1,
        "drawing moves the physical card from library to a new hand incarnation"
    );
}
