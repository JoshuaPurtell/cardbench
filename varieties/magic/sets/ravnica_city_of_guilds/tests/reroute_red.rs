//! Red regression for the public Reroute semantic slice.
//!
//! The green milestone must model an activated-ability stack item as a typed
//! retargetable object. A source permanent is not enough: two activations of
//! one source can coexist on the stack.

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game,
    GameEvent, HybridManaSymbol, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn reroute_is_a_full_fidelity_hybrid_instant() {
    let reroute = definition("RAV-REROUTE");
    assert_eq!(
        reroute.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [HybridManaSymbol {
                first: Color::Blue,
                second: Color::Red,
            }],
        )
    );
    assert_eq!(
        reroute.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&reroute.id),
        "Reroute requires a resolution-time single-target activated-ability retarget choice"
    );
}

fn rav_game() -> Game {
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

fn rotwurm_life(
    source: cardbench_magic_engine::ObjectId,
    sacrifice: cardbench_magic_engine::ObjectId,
) -> AbilityActivation {
    AbilityActivation {
        source,
        ability_id: "sacrifice-creature-target-player-life-loss",
        sacrifice_sources: vec![sacrifice],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![Target::Player(PlayerId(0))],
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The priority/stack trace is intentionally reviewed end-to-end.
fn reroute_retargets_one_exact_lower_activated_stack_item_then_draws() {
    let mut game = rav_game();
    let reroute = game
        .add_card(PlayerId(0), "RAV-REROUTE", Zone::Hand)
        .expect("Reroute setup");
    let drawn = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Library)
        .expect("draw setup");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("Reroute mana setup");
    let first_rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("first Rotwurm setup");
    let second_rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("second Rotwurm setup");
    let first_sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("first sacrifice setup");
    let second_sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("second sacrifice setup");
    let swamps = (0..8)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Rotwurm mana setup")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");

    // Advance through the opening turn's untap, upkeep, and draw boundaries,
    // then leave priority with the active Rotwurm controller in precombat
    // main. These instant-speed sacrifice abilities may coexist on the stack.
    for _ in 0..10 {
        if game.step == Step::PrecombatMain && game.priority == PlayerId(0) {
            break;
        }
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("advance toward Guildmage main-phase priority");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, PlayerId(0));
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("black mana ability");
    }
    game.activate_ability(PlayerId(0), rotwurm_life(first_rotwurm, first_sacrifice))
        .expect("first life-loss activation");
    let lower_ability = game.stack.last().expect("first ability stacked").id;
    game.activate_ability(PlayerId(0), rotwurm_life(second_rotwurm, second_sacrifice))
        .expect("second life-loss activation");
    let upper_ability = game.stack.last().expect("second ability stacked").id;
    assert_ne!(
        lower_ability, upper_ability,
        "distinct activated sources create distinct stack items"
    );
    let public_stack = game
        .view_for_player(PlayerId(0))
        .expect("public stack view");
    assert_eq!(public_stack.stack_activated_abilities.len(), 2);
    assert!(
        public_stack
            .stack_activated_abilities
            .iter()
            .any(|ability| ability.id == lower_ability
                && ability.targets == [Target::Player(PlayerId(0))])
    );

    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("blue mana ability");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: reroute,
            targets: vec![Target::ActivatedAbility(lower_ability)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Reroute targets only the lower activated ability");
    game.pass_priority(PlayerId(0))
        .expect("Reroute controller passes");
    game.pass_priority(PlayerId(1))
        .expect("Reroute reaches its resolution choice");

    let retarget_choice = game
        .view_for_player(PlayerId(0))
        .expect("retarget decision view")
        .pending_decision
        .expect("Reroute opens a no-priority retarget decision");
    assert_eq!(retarget_choice.kind, DecisionKind::RetargetActivatedAbility);
    game.submit_decision(
        PlayerId(0),
        retarget_choice.id,
        DecisionSelection::Targets(vec![Target::Player(PlayerId(1))]),
    )
    .expect("Reroute chooses a different legal target");

    assert_eq!(
        game.zone_of(drawn),
        Some(Zone::Hand),
        "Reroute controller draws"
    );
    assert_eq!(game.stack.len(), 2, "only the Reroute spell resolved");
    assert_eq!(
        game.stack[0].id, lower_ability,
        "lower ability remains identified"
    );
    assert_eq!(game.stack[0].targets, [Target::Player(PlayerId(1))]);
    assert_eq!(
        game.stack[1].id, upper_ability,
        "upper ability remains untouched"
    );
    assert_eq!(game.stack[1].targets, [Target::Player(PlayerId(0))]);

    // The still-upper original activation resolves first, then the retargeted
    // lower activation. This verifies that only the selected stack identity
    // changed, not every ability sharing its source.
    game.pass_priority(PlayerId(0)).expect("pass upper ability");
    game.pass_priority(PlayerId(1))
        .expect("resolve upper ability");
    game.pass_priority(PlayerId(0)).expect("pass lower ability");
    game.pass_priority(PlayerId(1))
        .expect("resolve lower ability");

    println!("Reroute exact-stack-item trace: {:?}", game.event_log);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ActivatedAbilityTargetChanged {
            source,
            target_ability,
            previous: Target::Player(PlayerId(0)),
            new: Target::Player(PlayerId(1)),
        } if *source == reroute && *target_ability == lower_ability
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == reroute
    )));
    game.validate_invariants()
        .expect("exact stack retarget trace preserves invariants");
}
