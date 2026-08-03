//! Red regression: a virtual spell copy can finish a public library-reorder
//! decision without trying to move its stack-only identity to a card zone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const REORDER: &str = "TST-VIRTUAL-COPY-REORDER";
const COPY: &str = "TST-VIRTUAL-COPY-REORDER-COPY";
const HIDDEN: &str = "TST-VIRTUAL-COPY-REORDER-HIDDEN";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-library-reorder-red"],
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
fn virtual_copy_can_complete_a_public_library_reorder() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                REORDER,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::RevealTopLibraryCardsAndReorder { count: 1 }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(HIDDEN, BTreeSet::from([CardType::Artifact]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let reorder = game
        .add_card(caster, REORDER, Zone::Hand)
        .expect("reorder spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let hidden = game
        .add_card(copy_controller, HIDDEN, Zone::Library)
        .expect("hidden card enters copy controller library");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(reorder, vec![]))
        .expect("reorder spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(reorder)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual reorder spell");
    resolve_top(&mut game).expect("virtual reorder opens public choice");

    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller has view")
        .pending_decision
        .expect("virtual reorder has a public decision");
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![hidden]
    );
    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == reorder => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies the virtual stack spell");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Objects(vec![hidden]),
    );
    eprintln!(
        "virtual-copy library-reorder red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the completed virtual reorder must not attempt a physical spell-zone move"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == reorder
    )));
    game.validate_invariants()
        .expect("virtual reorder terminal lifecycle remains auditable");
}
