//! Full-fidelity regressions for explicit selected-creature ability costs.

use cardbench_magic_engine::{
    AbilityActivation, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
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

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn creature_sacrifice_ability_definitions_and_bindings_are_exact() {
    let definitions = card_definitions();
    for id in ["RAV-DROOLING-GROODION", "RAV-GOLGARI-ROTWURM"] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "{id} is only full once all of its bound ability is represented"
        );
    }
    let groodion = definitions
        .iter()
        .find(|definition| definition.id == "RAV-DROOLING-GROODION")
        .expect("Drooling Groodion definition exists");
    assert_eq!(
        groodion.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Green])
    );
    let rotwurm = definitions
        .iter()
        .find(|definition| definition.id == "RAV-GOLGARI-ROTWURM")
        .expect("Golgari Rotwurm definition exists");
    assert_eq!(
        rotwurm.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Green])
    );

    let bindings = rav_activated_ability_bindings();
    let groodion_ability = &bindings
        .iter()
        .find(|binding| {
            binding.card_definition == "RAV-DROOLING-GROODION"
                && binding.ability.id == "sacrifice-creature-target-minus-two-minus-two"
        })
        .expect("Groodion ability binding exists")
        .ability;
    assert_eq!(
        groodion_ability.mana_cost,
        ManaCost::with_colors(0, [Color::Black, Color::Green])
    );
    assert_eq!(groodion_ability.sacrifice_creatures, 1);
    assert_eq!(groodion_ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        groodion_ability.effects,
        [Effect::ModifyTargetPtUntilEndOfTurn {
            power: -2,
            toughness: -2,
        }]
    );

    let rotwurm_ability = &bindings
        .iter()
        .find(|binding| {
            binding.card_definition == "RAV-GOLGARI-ROTWURM"
                && binding.ability.id == "sacrifice-creature-target-player-life-loss"
        })
        .expect("Rotwurm ability binding exists")
        .ability;
    assert_eq!(
        rotwurm_ability.mana_cost,
        ManaCost::with_colors(0, [Color::Black])
    );
    assert_eq!(rotwurm_ability.sacrifice_creatures, 1);
    assert_eq!(rotwurm_ability.targets, [TargetRequirement::Player]);
    assert_eq!(
        rotwurm_ability.effects,
        [Effect::LoseLifeTarget { amount: 1 }]
    );
}

#[test]
fn rotwurm_sacrifices_the_selected_creature_before_life_loss_resolves() {
    let mut game = game_with_rav_bindings();
    let rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("Rotwurm begins on battlefield");
    let victim = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on battlefield");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays Rotwurm activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: rotwurm,
            ability_id: "sacrifice-creature-target-player-life-loss",
            sacrifice_sources: vec![victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Rotwurm accepts one selected controlled creature");

    let sacrifice_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SacrificedAsAbilityCost { source, permanent, .. }
                    if *source == rotwurm && *permanent == victim
            )
        })
        .expect("selected creature sacrifice receipt exists");
    let activation_index = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::AbilityActivated { source, ability, .. }
                if *source == rotwurm && *ability == "sacrifice-creature-target-player-life-loss"
        ))
        .expect("ability activation receipt exists");
    assert!(sacrifice_index < activation_index);
    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
    resolve_top(&mut game);

    println!(
        "Rotwurm creature-sacrifice trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.players[1].life, 19);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeLost { source, player, amount }
            if *source == rotwurm && *player == PlayerId(1) && *amount == 1
    )));
    game.validate_invariants()
        .expect("Rotwurm sacrifice activation preserves invariants");
}

#[test]
fn groodion_sacrifices_the_selected_creature_before_creating_the_pt_effect() {
    let mut game = game_with_rav_bindings();
    let groodion = game
        .put_on_battlefield(PlayerId(0), "RAV-DROOLING-GROODION")
        .expect("Groodion begins on battlefield");
    let victim = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("controlled creature begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("target creature begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays Groodion activation");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays Groodion activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: groodion,
            ability_id: "sacrifice-creature-target-minus-two-minus-two",
            sacrifice_sources: vec![victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Groodion accepts one selected controlled creature");
    resolve_top(&mut game);

    println!(
        "Groodion creature-sacrifice trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
    assert_eq!(
        game.characteristics(target)
            .expect("target remains live")
            .power,
        Some(1)
    );
    assert_eq!(
        game.characteristics(target)
            .expect("target remains live")
            .toughness,
        Some(1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target: affected, .. }
            if *source == groodion && *affected == target
    )));
    game.validate_invariants()
        .expect("Groodion sacrifice activation preserves invariants");
}

#[test]
fn invalid_creature_sacrifice_selection_is_an_atomic_no_op() {
    let mut game = game_with_rav_bindings();
    let rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("Rotwurm begins on battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp produces pre-existing ability mana");
    let events_before = game.event_log.clone();

    let error = game
        .activate_ability(
            PlayerId(0),
            AbilityActivation {
                source: rotwurm,
                ability_id: "sacrifice-creature-target-player-life-loss",
                sacrifice_sources: vec![forest],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(PlayerId(1))],
            },
        )
        .expect_err("a land cannot pay a selected creature sacrifice cost");
    assert!(error.to_string().contains("requires a sacrificed creature"));
    assert_eq!(game.zone_of(forest), Some(Zone::Battlefield));
    assert_eq!(game.players[0].mana_pool.amount(Color::Black), 1);
    assert_eq!(game.event_log, events_before);
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("rejected creature selection preserves invariants");
}
