//! Red regression: a virtual copied policy-submitted multi-card library
//! search can complete without attempting a physical source-zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent,
    LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, ObjectId, PlayerId, Target, Zone,
};

const SEARCH: &str = "TST-VIRTUAL-COPY-MULTI-LIBRARY-SEARCH";
const COPY: &str = "TST-VIRTUAL-COPY-MULTI-LIBRARY-SEARCH-COPY";
const CREATURE: &str = "TST-VIRTUAL-COPY-MULTI-LIBRARY-SEARCH-CREATURE";

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
        supported_rules: &["virtual-spell-copy-multi-library-search-terminal-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
fn virtual_copy_can_complete_policy_submitted_multi_library_search() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SEARCH,
                CardType::Instant,
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    cardinality: LibrarySearchCardinality::Exactly(1),
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: true,
                }],
            ),
            definition(
                COPY,
                CardType::Instant,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(CREATURE, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let search = game
        .add_card(caster, SEARCH, Zone::Hand)
        .expect("search enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let creature = game
        .add_card(copy_controller, CREATURE, Zone::Library)
        .expect("copy controller has hidden search candidate");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(search, vec![]))
        .expect("search casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(search)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual search spell");
    resolve_top(&mut game).expect("virtual multi-search opens private decision");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == search => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual search");
    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller view")
        .pending_decision
        .expect("virtual copy exposes private multi-search choice");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Objects(vec![creature]),
    );
    eprintln!(
        "virtual-copy multi-library-search red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual multi-library-search spell must not attempt a physical terminal-zone move"
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == search
    )));
    game.validate_invariants()
        .expect("virtual multi-library-search terminal lifecycle remains auditable");
}
