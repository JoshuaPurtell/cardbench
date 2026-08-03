//! Red regression: source-bound Aura search must notify Aura-entry observers.
//!
//! The fetched Aura is a real battlefield entrant, so its controller's
//! `ControlledAuraEntersBattlefield` observers must join its own ETB batch
//! after legal attachment and before the resolving search ability finishes.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, DecisionSelection,
    Effect, Game, GameEvent, LibrarySearchSelection, ManaCost, PlayerId, Step, TargetRequirement,
    TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const HOST: &str = "TST-AURA-SEARCH-ENTRY-HOST";
const AURA: &str = "TST-AURA-SEARCH-ENTRY-AURA";
const WATCHER: &str = "TST-AURA-SEARCH-ENTRY-WATCHER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["aura-search-entry-observer-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    for _ in 0..2 {
        pass_pair(game);
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
#[allow(clippy::too_many_lines)] // The full cast/search/entry ordering transcript is the regression.
fn source_bound_aura_search_stacks_controlled_aura_entry_observer() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(HOST, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
            definition(WATCHER, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [
            TriggeredAbilityBinding {
                card_definition: HOST,
                ability: TriggeredAbility {
                    id: "search-compatible-aura",
                    condition: TriggerCondition::EntersBattlefield,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![
                        Effect::SearchControllerLibraryForCompatibleAuraAttachedToSource {
                            selection: LibrarySearchSelection::PolicySubmitted {
                                may_fail_to_find: false,
                            },
                        },
                    ],
                },
            },
            TriggeredAbilityBinding {
                card_definition: WATCHER,
                ability: TriggeredAbility {
                    id: "controlled-aura-entered",
                    condition: TriggerCondition::ControlledAuraEntersBattlefield,
                    mana_cost: ManaCost::new(0),
                    optional: true,
                    targets: vec![],
                    effects: vec![Effect::CreateToken {
                        token: TokenSpec::saproling(),
                        count: 1,
                    }],
                },
            },
        ],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::ControlledCreature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding registers");
    let watcher = game
        .put_on_battlefield(controller, WATCHER)
        .expect("watcher setup");
    let host = game
        .add_card(controller, HOST, Zone::Hand)
        .expect("host setup");
    let aura = game
        .add_card(controller, AURA, Zone::Library)
        .expect("Aura setup");
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);
    game.cast_spell(
        controller,
        CastRequest {
            card: host,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("host casts");
    pass_pair(&mut game);
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("Aura search opens private choice");

    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![aura]),
    )
    .expect("selected Aura enters attached");
    eprintln!(
        "source-bound Aura-search trace={:?}",
        game.canonical_event_log()
    );

    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura exists").attached_to,
        Some(host)
    );
    let watcher_incarnation = game.object(watcher).expect("watcher exists").incarnation;
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: "controlled-aura-entered",
                ..
            } if *source == watcher && *source_incarnation == watcher_incarnation
        )),
        "a fetched Aura must stack its controller's Aura-entry observer"
    );
    game.validate_invariants()
        .expect("Aura-search observer transition remains auditable");
}
