//! Red-to-green contract for controller end-step graveyard-return provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_step(game: &mut Game, step: Step) {
    for _ in 0..32 {
        if game.step == step {
            return;
        }
        pass_pair(game);
    }
    panic!("fixture did not reach {step:?}");
}

#[test]
fn gleancrawler_requires_exact_controller_end_step_return_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GLEANCRAWLER")
        .expect("Gleancrawler definition exists");

    assert_eq!(
        executable_definition_id_for_collector(247),
        Ok("RAV-GLEANCRAWLER")
    );
    assert_eq!(definition.name, "Gleancrawler");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Green])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Black, Color::Green]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(6), Some(6)));
    assert_eq!(definition.keywords, vec![Keyword::Trample]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(
        &"controller-end-step-return-creature-cards-put-into-graveyard-from-battlefield-this-turn"
    ));
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "controller-end-step-return-this-turn-battlefield-creatures"
    }));
}

#[test]
fn gleancrawler_returns_only_its_controllers_creature_cards_that_died_this_turn() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_triggers();
    let crawler = game
        .put_on_battlefield(controller, "RAV-GLEANCRAWLER")
        .expect("Gleancrawler setup");
    let dying = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("creature setup");
    let already_dead = game
        .add_card(controller, "RAV-SNAPPING-DRAKE", Zone::Graveyard)
        .expect("preexisting graveyard creature setup");
    let removal = game
        .add_card(controller, "RAV-LAST-GASP", Zone::Hand)
        .expect("removal setup");
    game.grant_mana(controller, Color::Black, 1)
        .expect("removal payment setup");

    game.begin_game().expect("game begins");
    advance_to_step(&mut game, Step::PrecombatMain);
    game.cast_spell(
        controller,
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(dying)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp targets Watchwolf");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(dying), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(already_dead), Some(Zone::Graveyard));

    advance_to_step(&mut game, Step::End);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == crawler
                && *ability == "controller-end-step-return-this-turn-battlefield-creatures"
    )));
    pass_pair(&mut game);

    assert_eq!(game.zone_of(dying), Some(Zone::Hand));
    assert_eq!(
        game.zone_of(already_dead),
        Some(Zone::Graveyard),
        "a card already in the graveyard was not put there from the battlefield this turn"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == crawler
                && *ability == "controller-end-step-return-this-turn-battlefield-creatures"
    )));
    game.validate_invariants()
        .expect("controller end-step return preserves state-machine invariants");
    eprintln!("gleancrawler_trace={:?}", game.canonical_event_log());
}
