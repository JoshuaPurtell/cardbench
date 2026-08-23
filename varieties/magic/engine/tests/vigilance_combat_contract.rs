//! Expansion-neutral combat and event contracts for vigilance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Step,
};

const VIGILANT_ATTACKER: &str = "TEST-VIGILANT-ATTACKER";
const ORDINARY_ATTACKER: &str = "TEST-ORDINARY-ATTACKER";
const DECK_CARD: &str = "TEST-VIGILANCE-DECK-CARD";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn card_types(card_types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    card_types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: VIGILANT_ATTACKER,
            name: "Vigilant Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: card_types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics", "vigilance"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Vigilance],
            effects: vec![],
        },
        CardDefinition {
            id: ORDINARY_ATTACKER,
            name: "Ordinary Attacker",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: card_types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: DECK_CARD,
            name: "Deck Card",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: card_types([CardType::Land]),
            is_basic_land: true,
            supported_rules: &["deck-filler"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn deck() -> DeckList {
    DeckList {
        mainboard: vec![DeckEntry {
            card: DECK_CARD.to_owned(),
            count: 12,
        }],
        sideboard: vec![],
    }
}

fn pass_round(game: &mut Game) {
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .attackers_declared
    {
        game.declare_attackers(game.next_policy_player(), &[])
            .expect("empty attackers are explicit");
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("combat view")
            .blockers_declared
    {
        game.declare_blockers(game.next_policy_player(), &[])
            .expect("empty blockers are explicit");
    }
    for _ in 0..2 {
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw");
        }
        game.pass_priority(game.priority).expect("priority pass");
        game.validate_invariants()
            .expect("every public transition preserves invariants");
    }
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..80 {
        if game.turn == turn && game.step == step {
            return;
        }
        pass_round(game);
    }
    panic!("did not reach turn {turn} step {step:?}");
}

#[test]
fn vigilance_declaration_preserves_untapped_state_and_records_combat_event() {
    let attacker_controller = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("game initializes");
    game.load_deck_into_library(attacker_controller, &deck())
        .expect("attacker deck loads");
    game.load_deck_into_library(defender, &deck())
        .expect("defender deck loads");
    let vigilant = game
        .put_on_battlefield(attacker_controller, VIGILANT_ATTACKER)
        .expect("vigilant creature enters");
    let ordinary = game
        .put_on_battlefield(attacker_controller, ORDINARY_ATTACKER)
        .expect("ordinary creature enters");
    game.begin_game().expect("game begins");

    advance_to(&mut game, 3, Step::DeclareAttackers);
    game.clear_event_log();
    game.declare_attackers(attacker_controller, &[vigilant, ordinary])
        .expect("both legal attackers are declared");

    assert!(
        !game
            .object(vigilant)
            .expect("vigilant attacker exists")
            .tapped,
        "a vigilant attacker must remain untapped after attack declaration"
    );
    assert!(
        game.object(ordinary)
            .expect("ordinary attacker exists")
            .tapped,
        "an ordinary attacker still taps on declaration"
    );
    assert_eq!(
        game.canonical_event_log(),
        [format!(
            "AttackersDeclared {{ player: {attacker_controller:?}, attackers: [{vigilant:?}, {ordinary:?}] }}"
        )],
        "the combat event records the complete declaration without inventing a tap receipt"
    );
    assert_eq!(
        game.view_for_player(attacker_controller)
            .expect("combat view")
            .combat_attackers
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![vigilant, ordinary],
        "both attackers remain in the authoritative combat projection"
    );
    game.validate_invariants()
        .expect("vigilant and ordinary declaration states are internally coherent");

    game.pass_priority(attacker_controller)
        .expect("attacker passes after declaration");
    game.pass_priority(defender)
        .expect("defender passes into blockers");
    game.declare_blockers(defender, &[])
        .expect("empty blocking declaration is legal");
    game.pass_priority(attacker_controller)
        .expect("attacker passes into combat damage");
    game.pass_priority(defender)
        .expect("defender passes into combat damage");

    assert_eq!(game.step, Step::CombatDamage);
    assert_eq!(game.player(defender).expect("defender exists").life, 15);
    assert!(
        !game
            .object(vigilant)
            .expect("vigilant attacker remains")
            .tapped,
        "combat damage does not retroactively tap a vigilant attacker"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttackersDeclared { player, attackers }
            if *player == attacker_controller && attackers == &vec![vigilant, ordinary]
    )));
    game.validate_invariants()
        .expect("the combat state remains valid through damage");
}
