//! Red regression for Mortipede's paid must-block activation.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Step,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_to_declare_attackers(game: &mut Game) {
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player).expect("priority advances");
    }
}

#[test]
fn mortipede_requires_its_green_must_block_activation_for_full_fidelity() {
    let mortipede = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MORTIPEDE")
        .expect("Mortipede definition exists");
    assert_eq!(mortipede.mana_cost, ManaCost::with_colors(3, [Color::Black]));
    assert_eq!(mortipede.card_types, [CardType::Creature].into_iter().collect());
    assert_eq!((mortipede.power, mortipede.toughness), (Some(4), Some(1)));
    assert!(mortipede.keywords.is_empty());
    assert!(
        mortipede
            .supported_rules
            .contains(&"activated-green-must-be-blocked")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&mortipede.id));

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == mortipede.id)
        .expect("Mortipede activated ability exists")
        .ability;
    assert_eq!(ability.mana_cost, ManaCost::with_colors(2, [Color::Green]));
    assert!(!ability.tap_cost);
    assert!(ability.targets.is_empty());
    assert_eq!(
        ability.effects,
        [Effect::AddSourceKeywordUntilEndOfTurn {
            keyword: Keyword::MustBeBlockedIfAble,
        }]
    );
}

#[test]
fn mortipede_activation_uses_the_stack_then_rejects_an_empty_legal_block_step() {
    let mut game = game_with_rav_bindings();
    let mortipede = game
        .put_on_battlefield(PlayerId(0), "RAV-MORTIPEDE")
        .expect("Mortipede starts on the battlefield");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent has one legal blocker");
    game.set_entered_turn_for_setup(mortipede, 0)
        .expect("Mortipede was controlled before this turn");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("setup funds the green activation");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: mortipede,
            ability_id: "green-must-be-blocked",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Mortipede activation is legal");
    game.pass_priority(PlayerId(0))
        .expect("controller passes activation");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves activation");
    assert!(
        game.characteristics(mortipede)
            .expect("Mortipede remains live")
            .keywords
            .contains(&Keyword::MustBeBlockedIfAble)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == mortipede && *target == mortipede
    )));

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[mortipede])
        .expect("Mortipede attacks");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes");
    game.pass_priority(PlayerId(1))
        .expect("defender receives blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);

    let rejection = game.declare_blockers(PlayerId(1), &[]);
    assert!(rejection.is_err(), "the legal blocker must be assigned");
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. })),
        "rejected block declarations must not emit a receipt"
    );
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: mortipede,
            blocker,
        }],
    )
    .expect("legal blocker satisfies the temporary must-block rule");
    println!("Mortipede trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Mortipede's temporary combat restriction preserves invariants");
}
