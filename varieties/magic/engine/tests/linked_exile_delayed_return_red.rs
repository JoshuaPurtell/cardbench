//! RED: an Aura-relative exile effect must retain its exact attachment group
//! and schedule an incarnation-aware delayed return.
//!
//! This uses only synthetic `TST-*` definitions. It intentionally starts
//! from the pre-substrate `ExileTargetCreature` behavior: that effect exiles
//! the creature but loses its attached Aura to state-based actions, so there
//! is no group available for a later deterministic return.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, ContinuousChange, DelayedActionTiming, Effect, Game, GameEvent, ManaCost,
    PlayerId, Step, Target, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-LINKED-CREATURE";
const AURA: &str = "TST-LINKED-AURA";
const STALE: &str = "TST-LINKED-STALE-TARGET";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["linked-exile-delayed-return-probe"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            definition(
                CREATURE,
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
                vec![],
            ),
            definition(
                AURA,
                BTreeSet::from([CardType::Enchantment]),
                None,
                None,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
            ),
            definition(
                STALE,
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![Effect::DestroyTargetNonblackCreature],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: AURA,
            ability: ActivatedAbility {
                id: "exile-linked-group",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
            },
        }],
    )
    .expect("synthetic fixture initializes")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second).expect("second pass succeeds");
}

fn advance_to_end_step(game: &mut Game) {
    while game.step != Step::End {
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attacker declaration succeeds");
        }
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // Regression asserts the complete linked-exile event trace.
fn attached_group_exiles_together_instead_of_orphaning_the_aura() {
    let controller = PlayerId(0);
    let mut game = game();
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature enters before the game begins");
    let aura = game
        .add_card(controller, AURA, Zone::Hand)
        .expect("Aura enters hand before the game begins");
    let stale = game
        .add_card(PlayerId(1), STALE, Zone::Hand)
        .expect("stale-target spell enters hand before the game begins");
    game.begin_game().expect("game begins");
    pass_pair(&mut game); // Upkeep → Draw
    pass_pair(&mut game); // Draw → precombat main

    game.cast_spell(
        controller,
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura spell casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    let initial_creature_incarnation = game.object(creature).expect("creature is live").incarnation;
    game.pass_priority(controller)
        .expect("Aura controller passes priority to the opposing stale-target caster");

    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: stale,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("stale-target spell casts before the linked exile response");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes priority to the Aura controller");

    game.activate_ability(
        controller,
        AbilityActivation {
            source: aura,
            ability_id: "exile-linked-group",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("synthetic group ability enters the stack");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    assert_eq!(
        game.zone_of(aura),
        Some(Zone::Exile),
        "an attached Aura must remain in the exact linked exile group rather than die as an orphan",
    );
    let (action, group, members) = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::DelayedActionScheduled {
                action,
                timing: DelayedActionTiming::EndStep,
                group,
                members,
                ..
            } => Some((*action, *group, members.clone())),
            _ => None,
        })
        .expect("linked exile must schedule an end-step action");
    assert_eq!(members.len(), 2, "only the creature and attached Aura link");
    assert!(
        game.linked_exile_group(group).is_some(),
        "suspended linked group is typed game state",
    );

    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::SpellCounteredByRules { card } if *card == stale)
    }));
    advance_to_end_step(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura returned").attached_to,
        Some(creature),
        "returned Aura attaches only through a fresh explicit attachment",
    );
    assert!(
        game.object(creature)
            .expect("creature returned")
            .incarnation
            > initial_creature_incarnation,
        "a delayed return is a new creature incarnation",
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::DelayedActionConsumed { action: actual, group: actual_group, returned }
            if *actual == action && *actual_group == group && returned == &vec![creature, aura])
    }));
    eprintln!(
        "linked-exile trace: action={action:?}; group={group:?}; events={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("linked-exile suspended and consumed states are invariant-valid");
}
