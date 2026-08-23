//! RED: a linked Aura must stay in exile when the returning primary is no
//! longer a legal object for that Aura to enchant.
//!
//! CR 303.4i keeps an Aura in its current zone when an effect attempts to put
//! it onto the battlefield attached to an object it cannot legally enchant.
//! This regression makes the primary creature change controller while it is
//! exiled, so the controller-relative Aura is legal before exile but illegal
//! when the delayed instruction would return it.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, ContinuousChange, Duration, Effect, Game, GameEvent, ManaCost, PlayerId,
    Step, Target, TargetRequirement, Zone,
};

const CONTROL_SOURCE: &str = "TST-LINKED-CONTROL-SOURCE";
const CREATURE: &str = "TST-LINKED-CONTROLLED-CREATURE";
const AURA: &str = "TST-LINKED-CONTROLLED-AURA";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["linked-exile-illegal-aura-return-probe"],
        power: is_creature.then_some(2),
        toughness: is_creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            definition(CONTROL_SOURCE, BTreeSet::from([CardType::Artifact]), vec![]),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
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

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
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
#[allow(clippy::too_many_lines)] // The regression intentionally records the entire delayed-return transition.
fn linked_aura_does_not_enter_unattached_when_primary_returns_to_other_controller() {
    let aura_controller = PlayerId(0);
    let creature_owner = PlayerId(1);
    let mut game = game();
    let control_source = game
        .put_on_battlefield(aura_controller, CONTROL_SOURCE)
        .expect("control source enters before the game begins");
    let creature = game
        .put_on_battlefield(creature_owner, CREATURE)
        .expect("opponent owns the creature");
    let aura = game
        .add_card(aura_controller, AURA, Zone::Hand)
        .expect("Aura enters hand before the game begins");

    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        control_source,
        creature,
        ContinuousChange::ChangeController(aura_controller),
        Duration::Permanent,
    )
    .expect("aura controller temporarily controls the primary");
    advance_to_precombat_main(&mut game);

    game.cast_spell(
        aura_controller,
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controlled-creature Aura casts while the primary is controlled");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.controller_of(creature).expect("derived controller"),
        aura_controller
    );

    game.activate_ability(
        aura_controller,
        AbilityActivation {
            source: aura,
            ability_id: "exile-linked-group",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("linked exile activation enters the stack");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    assert_eq!(game.zone_of(aura), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target, .. }
            if *source == control_source && *target == creature
    )));

    advance_to_end_step(&mut game);

    eprintln!(
        "linked-exile illegal-Aura return trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert_eq!(
        game.controller_of(creature)
            .expect("returned creature controller"),
        creature_owner,
        "the target-specific control effect must expire when the primary leaves",
    );
    assert_eq!(
        game.zone_of(aura),
        Some(Zone::Exile),
        "a delayed instruction must keep an Aura in exile when the returned primary is no longer a legal enchant target",
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedActionConsumed { returned, .. } if returned == &vec![creature]
    )));
    game.validate_invariants()
        .expect("the linked group transition remains invariant-valid");
}
