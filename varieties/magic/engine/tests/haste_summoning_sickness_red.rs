//! Red regression for haste's two summoning-sickness exceptions.
//!
//! The keyword representation is present, but the current engine still rejects
//! an otherwise legal same-turn attack and a creature mana ability with a tap
//! cost. These are two pieces of one rules exception, and neither rejection
//! may mutate the combat or mana-ability state machine.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Game, Keyword, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaCost, PlayerId, RulesError, Step,
};

const HASTY_ADEPT: &str = "TEST-HASTY-ADEPT";

fn hasty_adept() -> CardDefinition {
    CardDefinition {
        id: HASTY_ADEPT,
        name: "Hasty Adept",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "haste", "test-mana-ability"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Haste],
        effects: vec![],
    }
}

fn hasty_game() -> Game {
    Game::new_with_mana_abilities(
        vec![hasty_adept()],
        2,
        [ManaAbilityBinding {
            card_definition: HASTY_ADEPT,
            ability: ActivatedManaAbility {
                id: "tap-red",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Red),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        }],
    )
    .expect("valid hasty test catalog initializes")
}

fn reach_declare_attackers(game: &mut Game) {
    for _ in 0..8 {
        game.pass_priority(game.priority)
            .expect("priority passes reach attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn haste_allows_a_same_turn_attack_without_an_illegal_transition() {
    let player = PlayerId(0);
    let mut game = hasty_game();
    let attacker = game
        .put_on_battlefield(player, HASTY_ADEPT)
        .expect("hasty creature enters on turn one");
    game.begin_game().expect("fixture reaches upkeep");
    reach_declare_attackers(&mut game);
    game.clear_event_log();

    assert!(
        game.view_for_player(player)
            .expect("active player view")
            .own_battlefield
            .iter()
            .find(|card| card.id == attacker)
            .expect("attacker is visible")
            .can_attack,
        "the policy view must expose the legal same-turn Haste attack"
    );

    let result = game.declare_attackers(player, &[attacker]);

    assert!(
        result.is_ok(),
        "haste must permit this creature to attack on its entry turn; result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(game.object(attacker).expect("attacker exists").tapped);
    game.validate_invariants()
        .expect("a hasty same-turn attack preserves combat invariants");
}

#[test]
fn haste_allows_a_same_turn_tap_mana_ability_without_an_illegal_transition() {
    let player = PlayerId(0);
    let mut game = hasty_game();
    let source = game
        .put_on_battlefield(player, HASTY_ADEPT)
        .expect("hasty creature enters on turn one");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    let result = game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "tap-red",
            chosen_color: None,
        },
    );

    assert!(
        result.is_ok(),
        "haste must permit a creature to pay a tap cost on its entry turn; result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Red),
        1
    );
    game.validate_invariants()
        .expect("a hasty tap ability preserves mana invariants");
}

#[test]
fn rejected_nonhaste_summoning_sickness_attempts_remain_atomic() {
    let player = PlayerId(0);
    let mut definition = hasty_adept();
    definition.keywords.clear();
    let mut game = Game::new_with_mana_abilities(
        vec![definition],
        2,
        [ManaAbilityBinding {
            card_definition: HASTY_ADEPT,
            ability: ActivatedManaAbility {
                id: "tap-red",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Red),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        }],
    )
    .expect("valid ordinary test catalog initializes");
    let source = game
        .put_on_battlefield(player, HASTY_ADEPT)
        .expect("ordinary creature enters");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    assert_eq!(
        game.activate_bound_mana_ability(
            player,
            ManaAbilityActivation {
                source,
                ability_id: "tap-red",
                chosen_color: None,
            },
        ),
        Err(RulesError::IllegalAction(
            "a summoning-sick creature cannot pay a tap mana-ability cost"
        ))
    );
    assert!(game.canonical_event_log().is_empty());
    assert!(!game.object(source).expect("source exists").tapped);
    game.validate_invariants()
        .expect("rejected nonhaste activation is atomic");
}
