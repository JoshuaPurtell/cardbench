//! Red discovery contract for Privileged Position's other-permanent Shroud.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, HybridManaSymbol, ManaCost, PlayerId, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings,
};

fn privileged_position_game() -> Game {
    Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV Privileged Position fixture builds")
}

#[test]
fn privileged_position_requires_other_controlled_permanent_shroud() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PRIVILEGED-POSITION")
        .expect("Privileged Position definition exists");

    assert_eq!(definition.name, "Privileged Position");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_hybrid(
            2,
            [],
            [
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
            ],
        )
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"other-controlled-permanents-have-shroud")
    );
}

#[test]
fn privileged_position_blocks_targeting_its_other_controlled_permanents_but_not_itself() {
    let mut game = privileged_position_game();
    let position = game
        .put_on_battlefield(PlayerId(0), "RAV-PRIVILEGED-POSITION")
        .expect("Privileged Position setup");
    let protected = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("other controlled permanent setup");
    let last_gasp = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("opponent has a targeted creature spell");
    let leave_no_trace = game
        .add_card(PlayerId(1), "RAV-LEAVE-NO-TRACE", Zone::Hand)
        .expect("opponent has a targeted enchantment spell");
    game.grant_mana(PlayerId(1), Color::Black, 1)
        .expect("pre-game black mana setup");
    game.grant_mana(PlayerId(1), Color::White, 1)
        .expect("pre-game white mana setup");
    game.begin_game().expect("game begins");

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    let events_before_rejection = game.canonical_event_log();
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(protected)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect_err("Shroud makes the allied creature an illegal target before cost payment");
    assert_eq!(game.zone_of(last_gasp), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert_eq!(game.canonical_event_log(), events_before_rejection);

    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: leave_no_trace,
            targets: vec![Target::Permanent(position)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the source itself is not Shrouded and remains a legal target");
    game.pass_priority(PlayerId(1))
        .expect("caster passes");
    game.pass_priority(PlayerId(0))
        .expect("targeted enchantment spell resolves");

    assert_eq!(game.zone_of(position), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == position
    )));
    eprintln!("Privileged Position trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Shroud targeting boundary preserves state-machine invariants");
}
