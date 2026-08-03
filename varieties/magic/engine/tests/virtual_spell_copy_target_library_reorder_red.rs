//! Red regression: a virtual copy must finish a private target-library
//! reorder without taking a physical spell-card terminal path.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, Target, Zone,
};

const REORDER: &str = "TST-VIRTUAL-TARGET-REORDER";
const COPY: &str = "TST-VIRTUAL-TARGET-REORDER-COPY";
const FIRST: &str = "TST-VIRTUAL-TARGET-REORDER-FIRST";
const SECOND: &str = "TST-VIRTUAL-TARGET-REORDER-SECOND";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["virtual-target-library-reorder-red"],
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
fn virtual_copy_finishes_private_target_library_reorder_with_copy_terminal_receipt() {
    let caster = PlayerId(0);
    let copying_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                REORDER,
                vec![Effect::LookAtTopCardsOfTargetPlayerAndReorder { count: 2 }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(FIRST, vec![]),
            definition(SECOND, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let reorder = game
        .add_card(caster, REORDER, Zone::Hand)
        .expect("targeted reorder spell enters hand");
    let copy_spell = game
        .add_card(copying_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let first = game
        .add_card(copying_controller, FIRST, Zone::Library)
        .expect("first private library card enters");
    let second = game
        .add_card(copying_controller, SECOND, Zone::Library)
        .expect("second private library card enters");
    game.begin_game().expect("live game begins");

    game.cast_spell(
        caster,
        request(reorder, vec![Target::Player(copying_controller)]),
    )
    .expect("physical targeted reorder casts");
    game.pass_priority(caster)
        .expect("caster passes to copying controller");
    game.cast_spell(
        copying_controller,
        request(copy_spell, vec![Target::Spell(reorder)]),
    )
    .expect("copying controller copies the reorder spell");
    resolve_top(&mut game).expect("physical copying spell creates virtual reorder copy");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == reorder => Some(*copy),
            _ => None,
        })
        .expect("virtual target-library reorder copy exists");
    resolve_top(&mut game).expect("virtual reorder copy opens private decision");
    let decision = game
        .view_for_player(copying_controller)
        .expect("copying controller view")
        .pending_decision
        .expect("virtual reorder copy opens private decision");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerLibraryTopReorder);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![second, first],
        "the copied spell carries its target's current top-first snapshot"
    );

    let result = game.submit_decision(
        copying_controller,
        decision.id,
        DecisionSelection::TargetPlayerLibraryTopReorder {
            top: vec![first],
            bottom: vec![second],
        },
    );
    eprintln!(
        "virtual target-library reorder red: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual copied reorder spell must not attempt a physical terminal-zone move"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original }
            if *copy == virtual_copy && *original == reorder
    )));
    game.validate_invariants()
        .expect("virtual reorder completion keeps stack and private decision lifecycles valid");
}
