//! Red regression: a virtual copied policy-submitted library search can
//! complete its hidden selection without a fabricated source-zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent,
    LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const SEARCH: &str = "TST-VIRTUAL-COPY-LIBRARY-SEARCH";
const COPY: &str = "TST-VIRTUAL-COPY-LIBRARY-SEARCH-COPY";
const LAND: &str = "TST-VIRTUAL-COPY-LIBRARY-SEARCH-LAND";

fn definition(
    id: &'static str,
    types: BTreeSet<CardType>,
    is_basic_land: bool,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land,
        supported_rules: &["virtual-spell-copy-library-search-terminal-red"],
        power: None,
        toughness: None,
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
fn virtual_copy_can_complete_policy_submitted_library_search() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SEARCH,
                BTreeSet::from([CardType::Instant]),
                false,
                vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::ManaValueExactly(0),
                    destination: LibrarySearchDestination::Hand,
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                false,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(LAND, BTreeSet::from([CardType::Land]), true, vec![]),
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
    let land = game
        .add_card(copy_controller, LAND, Zone::Library)
        .expect("copy controller has hidden search candidate");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(search, vec![]))
        .expect("search casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(search)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual search spell");
    resolve_top(&mut game).expect("virtual search opens private decision");

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
        .expect("virtual copy exposes private library choice");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Objects(vec![land]),
    );
    eprintln!(
        "virtual-copy library-search red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual library-search spell must not attempt a physical terminal-zone move"
    );
    assert_eq!(game.zone_of(land), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == search
    )));
    game.validate_invariants()
        .expect("virtual library-search terminal lifecycle remains auditable");
}
