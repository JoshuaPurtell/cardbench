//! Red regression: damage that cannot be prevented also bypasses redirection.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, Keyword, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const REDIRECTOR: &str = "DAMAGE-CANNOT-BE-PREVENTED-REDIRECTOR";
const TARGET: &str = "DAMAGE-CANNOT-BE-PREVENTED-TARGET";
const RED_SOURCE: &str = "DAMAGE-CANNOT-BE-PREVENTED-SOURCE";

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: REDIRECTOR,
            name: REDIRECTOR,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["damage-redirection-probe"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: TARGET,
            name: TARGET,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["damage-redirection-probe"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: RED_SOURCE,
            name: RED_SOURCE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage-cannot-be-prevented", "damage-redirection-probe"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::DamageCannotBePrevented],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }],
        },
    ]
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
fn damage_cannot_be_prevented_bypasses_damage_redirection() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        definitions(),
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: REDIRECTOR,
            ability: ActivatedAbility {
                id: "redirect-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    TargetRequirement::Creature,
                    TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 2 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        }],
    )
    .expect("fixture game initializes");
    let redirector = game
        .put_on_battlefield(caster, REDIRECTOR)
        .expect("redirector enters");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("target enters");
    let source = game
        .add_card(caster, RED_SOURCE, Zone::Hand)
        .expect("damage source enters hand");

    game.activate_ability(
        caster,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target), Target::Player(opponent)],
        },
    )
    .expect("redirection ability activates");
    pass_pair(&mut game);

    game.cast_spell(
        caster,
        CastRequest {
            card: source,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage-cannot-be-prevented source casts");
    pass_pair(&mut game);

    eprintln!(
        "damage cannot be prevented red result: target={:?}; opponent_life={}; events={:?}",
        game.object(target),
        game.player(opponent).expect("opponent exists").life,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(target).expect("target remains alive").damage,
        2,
        "damage that cannot be prevented must not be redirected away"
    );
    assert_eq!(
        game.player(opponent).expect("opponent exists").life,
        20,
        "the redirection destination must not receive unpreventable damage"
    );
    assert!(
        !game
            .canonical_event_log()
            .iter()
            .any(|event| event.contains("DamageRedirected")),
        "unpreventable damage must bypass the redirection receipt"
    );
    game.validate_invariants()
        .expect("replacement precedence leaves a valid game state");
}
