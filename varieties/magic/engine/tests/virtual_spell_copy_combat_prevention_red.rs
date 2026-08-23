//! Red regressions: virtual instant copies can create both represented
//! source-independent combat-prevention effects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Step, Target, Zone,
};

const TARGET_PREVENT: &str = "TST-VIRTUAL-COPY-TARGET-COMBAT-PREVENT";
const GLOBAL_PREVENT: &str = "TST-VIRTUAL-COPY-GLOBAL-COMBAT-PREVENT";
const COPY: &str = "TST-VIRTUAL-COPY-COMBAT-PREVENT-COPY";
const BEAR: &str = "TST-VIRTUAL-COPY-COMBAT-PREVENT-BEAR";

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
        supported_rules: &["virtual-spell-copy-combat-prevention-red"],
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

fn pass_to(game: &mut Game, step: Step) {
    while game.step != step {
        let player = game.priority;
        game.pass_priority(player)
            .expect("ordinary priority passes advance the fixture");
    }
}

fn fixture() -> Game {
    Game::new(
        [
            definition(
                TARGET_PREVENT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
                    damage_target_controller_equal_to_power_if_mana_color_spent: None,
                }],
            ),
            definition(
                GLOBAL_PREVENT,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::PreventAllCombatDamageUntilEndOfTurn],
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
    .expect("fixture initializes")
}

fn copy_spell_and_resolve_virtual(
    game: &mut Game,
    original: ObjectId,
    copy: ObjectId,
    targets: Vec<Target>,
) -> Result<(), cardbench_magic_engine::RulesError> {
    game.cast_spell(PlayerId(0), request(original, targets))?;
    game.pass_priority(PlayerId(0))?;
    game.cast_spell(PlayerId(1), request(copy, vec![Target::Spell(original)]))?;
    resolve_top(game)?;
    resolve_top(game)
}

#[test]
fn virtual_copy_can_create_targeted_combat_prevention() {
    let mut game = fixture();
    let creature = game
        .put_on_battlefield(PlayerId(0), BEAR)
        .expect("creature enters before game begins");
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature predates this turn");
    let prevent = game
        .add_card(PlayerId(0), TARGET_PREVENT, Zone::Hand)
        .expect("targeted prevention enters hand");
    let copy = game
        .add_card(PlayerId(1), COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");
    pass_to(&mut game, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[creature])
        .expect("creature attacks before prevention is cast");

    let result =
        copy_spell_and_resolve_virtual(&mut game, prevent, copy, vec![Target::Permanent(creature)]);
    eprintln!(
        "virtual-copy targeted-combat-prevention red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a copied instant may prevent its target's combat damage this turn"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePreventionCreated {
            source,
            creature: target,
            ..
        }
            if *source != prevent && *target == creature
    )));
    game.validate_invariants()
        .expect("virtual-source targeted combat prevention remains auditable");
}

#[test]
fn virtual_copy_can_create_global_combat_prevention() {
    let mut game = fixture();
    let prevent = game
        .add_card(PlayerId(0), GLOBAL_PREVENT, Zone::Hand)
        .expect("global prevention enters hand");
    let copy = game
        .add_card(PlayerId(1), COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    let result = copy_spell_and_resolve_virtual(&mut game, prevent, copy, vec![]);
    eprintln!(
        "virtual-copy global-combat-prevention red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a copied instant may prevent all combat damage this turn"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::GlobalCombatDamagePreventionCreated { source, .. } if *source != prevent
    )));
    game.validate_invariants()
        .expect("virtual-source global combat prevention remains auditable");
}
