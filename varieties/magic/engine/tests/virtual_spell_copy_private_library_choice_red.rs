//! Red regression: a virtual spell copy can open and complete a private
//! library decision without fabricating a physical source object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Zone,
};

const LOOK: &str = "TST-VIRTUAL-COPY-PRIVATE-LOOK";
const COPY: &str = "TST-VIRTUAL-COPY-PRIVATE-LOOK-COPY";
const HIDDEN: &str = "TST-VIRTUAL-COPY-PRIVATE-HIDDEN";

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
        supported_rules: &["virtual-spell-copy-private-library-choice-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<cardbench_magic_engine::Target>) -> CastRequest {
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
fn virtual_copy_can_complete_a_private_library_choice() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                LOOK,
                vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                    count: 1,
                    life_per_card: 1,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(HIDDEN, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let look = game
        .add_card(caster, LOOK, Zone::Hand)
        .expect("look spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let hidden = game
        .add_card(copy_controller, HIDDEN, Zone::Library)
        .expect("hidden card enters the copy controller library");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(look, vec![]))
        .expect("look spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        copy_controller,
        request(copy, vec![cardbench_magic_engine::Target::Spell(look)]),
    )
    .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates the virtual look spell");

    let result = resolve_top(&mut game);
    eprintln!(
        "virtual-copy private-library red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the virtual copy must open its private library decision without a physical object"
    );
    let choice = game
        .view_for_player(copy_controller)
        .expect("copy controller has a view")
        .private_library_choice
        .expect("the copied spell exposes its controller-only library choice");
    assert_eq!(choice.cards.len(), 1);
    game.choose_private_library_cards(copy_controller, choice.spell, vec![])
        .expect("copy controller may put the inspected card into the graveyard");
    assert_eq!(game.zone_of(hidden), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == choice.spell && *original == look
    )));
    game.validate_invariants()
        .expect("the virtual private-library resolution remains auditable");
}
