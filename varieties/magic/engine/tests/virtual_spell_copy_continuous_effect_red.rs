//! Red regression: a virtual instant copy may create an end-of-turn layer
//! effect even though it has no physical card object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, ObjectId, PlayerId, Target, Zone,
};

const SEIZE: &str = "TST-VIRTUAL-COPY-CONTINUOUS-SEIZE";
const COPY: &str = "TST-VIRTUAL-COPY-CONTINUOUS-COPY";
const BEAR: &str = "TST-VIRTUAL-COPY-CONTINUOUS-BEAR";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    let creature = types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-continuous-effect-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
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
fn virtual_copy_can_install_an_end_of_turn_control_effect() {
    let original_controller = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SEIZE,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(BEAR, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let bear = game
        .put_on_battlefield(original_controller, BEAR)
        .expect("bear enters before game begins");
    let seize = game
        .add_card(original_controller, SEIZE, Zone::Hand)
        .expect("seize enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        original_controller,
        request(seize, vec![Target::Permanent(bear)]),
    )
    .expect("original control spell casts");
    game.pass_priority(original_controller)
        .expect("original controller passes");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(seize)]))
        .expect("opponent copies the control spell");
    resolve_top(&mut game).expect("copy effect resolves into virtual control spell");

    let result = resolve_top(&mut game);
    eprintln!(
        "virtual-copy continuous-effect red trace: result={result:?}; controller={:?}; events={:?}",
        game.controller_of(bear),
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a copied instant may create a temporary continuous effect"
    );
    assert_eq!(
        game.controller_of(bear).expect("bear exists"),
        copy_controller,
        "the copied spell's controller owns its temporary control effect"
    );
    game.validate_invariants()
        .expect("virtual-source end-of-turn effect remains auditable");
}
