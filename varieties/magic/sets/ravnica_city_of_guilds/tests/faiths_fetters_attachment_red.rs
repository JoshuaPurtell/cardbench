//! Red regression for the missing general Aura attachment/suppression substrate.

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn faiths_fetters_requires_general_permanent_attachment_semantics() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FAITHS-FETTERS")
        .expect("Faith's Fetters definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::White])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"aura-enchant-permanent")
    );
    assert!(definition.supported_rules.contains(&"etb-gain-four-life"));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-permanent-combat-and-activation-restriction")
    );
}

fn fetters_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn faiths_fetters_attaches_restricts_and_stacks_its_life_trigger() {
    let mut game = fetters_game();
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-DIMIR-GUILDMAGE")
        .expect("target enters");
    game.set_entered_turn_for_setup(target, 0)
        .expect("old target");
    let aura = game
        .add_card(PlayerId(0), "RAV-FAITHS-FETTERS", Zone::Hand)
        .expect("Aura enters hand");
    let plains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("opponent pass");
    }
    for land in plains {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("white mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    game.pass_priority(PlayerId(0)).expect("Aura pass");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");
    assert_eq!(
        game.object(aura).expect("Aura lives").attached_to,
        Some(target)
    );
    assert!(
        game.characteristics(target)
            .expect("target characteristics")
            .keywords
            .contains(&Keyword::CannotAttackOrBlock)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked {
            source,
            ability: "etb-gain-four-life",
            ..
        } if *source == aura
    )));
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    assert_eq!(game.player(PlayerId(0)).expect("player").life, 24);
    game.pass_priority(PlayerId(0))
        .expect("pass to target controller");
    let before = game.event_log.clone();
    let rejected = game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: target,
            ability_id: "target-player-discard",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(0))],
        },
    );
    assert!(rejected.is_err(), "attached nonmana activation must fail");
    assert_eq!(game.event_log, before, "rejection is atomic");
    game.validate_invariants().expect("Aura state is valid");
}

#[test]
fn faiths_fetters_can_enchant_a_land_without_suppressing_its_mana_ability() {
    let mut game = fetters_game();
    let mountain = game
        .put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    let aura = game
        .add_card(PlayerId(0), "RAV-FAITHS-FETTERS", Zone::Hand)
        .expect("Aura enters hand");
    let plains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("opponent pass");
    }
    for land in plains {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("white mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(mountain)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts on a land");
    game.pass_priority(PlayerId(0)).expect("Aura pass");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    game.pass_priority(PlayerId(0))
        .expect("pass to land controller");
    game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
        .expect("mana ability remains legal");
    assert_eq!(
        game.player(PlayerId(1))
            .expect("player")
            .mana_pool
            .amount(Color::Red),
        1
    );
    game.validate_invariants().expect("enchanted land is valid");
}
