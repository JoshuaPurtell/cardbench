//! Red regression for a target-bearing Radiance damage-prevention instruction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    Zone,
};

const APOTHECARY: &str = "TST-RADIANCE-SHIELD-APOTHECARY";
const TARGET: &str = "TST-RADIANCE-SHIELD-TARGET";
const SHARED: &str = "TST-RADIANCE-SHIELD-SHARED";
const OFF_COLOR: &str = "TST-RADIANCE-SHIELD-OFF-COLOR";
const DAMAGE: &str = "TST-RADIANCE-SHIELD-DAMAGE";

fn creature(id: &'static str, colors: BTreeSet<Color>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["radiance-damage-shield-contract"],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        creature(APOTHECARY, BTreeSet::from([Color::White])),
        creature(TARGET, BTreeSet::from([Color::White])),
        creature(SHARED, BTreeSet::from([Color::White, Color::Green])),
        creature(OFF_COLOR, BTreeSet::from([Color::Green])),
        CardDefinition {
            id: DAMAGE,
            name: DAMAGE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["radiance-damage-shield-contract"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }],
        },
    ]
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn radiance_prevention_shields_target_and_every_shared_color_creature() {
    let mut game = Game::new_with_all_bindings(
        definitions(),
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: APOTHECARY,
            ability: ActivatedAbility {
                id: "tap-radiance-prevent-one-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::RadianceAddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
            },
        }],
    )
    .expect("fixture builds once Radiance shields are a valid effect");
    let apothecary = game
        .put_on_battlefield(PlayerId(0), APOTHECARY)
        .expect("source setup");
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target setup");
    let shared = game
        .put_on_battlefield(PlayerId(1), SHARED)
        .expect("shared-color setup");
    let off_color = game
        .put_on_battlefield(PlayerId(1), OFF_COLOR)
        .expect("off-color setup");
    let damage = game
        .add_card(PlayerId(0), DAMAGE, Zone::Hand)
        .expect("damage spell setup");

    for card in [apothecary, target, shared, off_color] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("pre-game creature ages through the setup boundary");
    }
    game.begin_game().expect("fixture begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: apothecary,
            ability_id: "tap-radiance-prevent-one-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Radiance ability stacks");
    pass_pair(&mut game);

    for protected in [apothecary, target, shared] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageShieldCreated {
                source,
                target: Target::Permanent(shielded),
                amount: 1,
            } if *source == apothecary && *shielded == protected
        )));
    }
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated {
            target: Target::Permanent(shielded),
            ..
        } if *shielded == off_color
    )));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: damage,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage spell casts");
    pass_pair(&mut game);

    assert_eq!(game.object(target).expect("target remains").damage, 1);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(shielded),
            amount: 1,
        } if *source == damage && *shielded == target
    )));
    eprintln!("radiance shield trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Radiance shields preserve invariant state");
}
