//! Red regression: a virtual spell copy can finish a private top-library
//! partition without attempting to move its stack-only identity to a zone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const PARTITION: &str = "TST-VIRTUAL-COPY-PARTITION";
const COPY: &str = "TST-VIRTUAL-COPY-PARTITION-COPY";
const LOWER: &str = "TST-VIRTUAL-COPY-PARTITION-LOWER";
const TOP: &str = "TST-VIRTUAL-COPY-PARTITION-TOP";

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
        supported_rules: &["virtual-spell-copy-library-partition-red"],
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
fn virtual_copy_can_complete_a_private_library_partition() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                PARTITION,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 2 }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(LOWER, BTreeSet::from([CardType::Artifact]), vec![]),
            definition(TOP, BTreeSet::from([CardType::Artifact]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let partition = game
        .add_card(caster, PARTITION, Zone::Hand)
        .expect("partition spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let lower = game
        .add_card(copy_controller, LOWER, Zone::Library)
        .expect("lower library card enters");
    let top = game
        .add_card(copy_controller, TOP, Zone::Library)
        .expect("top library card enters");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(partition, vec![]))
        .expect("partition spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        copy_controller,
        request(copy, vec![Target::Spell(partition)]),
    )
    .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual partition spell");
    resolve_top(&mut game).expect("virtual partition opens private choice");

    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller has view")
        .pending_decision
        .expect("virtual partition opens a controller-private decision");
    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == partition => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual stack spell");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::LibraryTopPartition {
            hand: top,
            top: Some(lower),
            bottom: vec![],
        },
    );
    eprintln!(
        "virtual-copy library-partition red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the completed virtual partition must not attempt a physical spell-zone move"
    );
    assert_eq!(game.zone_of(top), Some(Zone::Hand));
    assert_eq!(game.players[copy_controller.0].library, vec![lower]);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == partition
    )));
    game.validate_invariants()
        .expect("virtual partition terminal lifecycle remains auditable");
}
