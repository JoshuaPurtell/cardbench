//! RED: an activated ability copied from another permanent must retain the
//! copied source definition after its physical source changes zones.
//!
//! The source's printed definition reappears after a zone change, but the
//! ability already on the stack belongs to the exact pre-departure copied
//! permanent incarnation (CR 113.7a and 400.7). Its validation and resolution
//! must therefore not rediscover the source's new base definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};

const COPY_SOURCE: &str = "TST-COPIED-ABILITY-SOURCE";
const COPY_TARGET: &str = "TST-COPIED-ABILITY-TARGET";
const DESTROY: &str = "TST-COPIED-ABILITY-DESTROY";

fn definition(
    id: &'static str,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["copied-card-activated-ability-departure-probe"],
        power: is_creature.then_some(2),
        toughness: is_creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            definition(
                COPY_SOURCE,
                BTreeSet::from([Color::Red]),
                BTreeSet::from([CardType::Creature]),
                vec![],
            ),
            definition(
                COPY_TARGET,
                BTreeSet::from([Color::Blue]),
                BTreeSet::from([CardType::Artifact, CardType::Creature]),
                vec![],
            ),
            definition(
                DESTROY,
                BTreeSet::from([Color::Black]),
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetNonblackCreature],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: COPY_SOURCE,
            ability: ActivatedAbility {
                id: "copied-ping",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DealDamageController { amount: 1 }],
            },
        }],
    )
    .expect("synthetic fixture initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn copied_activated_ability_resolves_after_the_copied_card_is_destroyed() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let source = game
        .put_on_battlefield(controller, COPY_SOURCE)
        .expect("copy source enters before the game begins");
    let target = game
        .put_on_battlefield(controller, COPY_TARGET)
        .expect("copy target enters before the game begins");
    let destroy = game
        .add_card(opponent, DESTROY, Zone::Hand)
        .expect("opponent receives the response");
    game.begin_game().expect("game begins");

    game.copy_permanent(target, source)
        .expect("target copies the ability source");
    game.activate_ability(
        controller,
        AbilityActivation {
            source: target,
            ability_id: "copied-ping",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("copied card places its inherited ability on the stack");
    game.pass_priority(controller)
        .expect("controller passes to the responding opponent");
    game.cast_spell(
        opponent,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent may destroy the copied red creature");
    let first_destroy_pass = game.priority;
    game.pass_priority(first_destroy_pass)
        .expect("destroy caster passes priority");
    let second_destroy_pass = game.priority;
    let destroy_resolution = game.pass_priority(second_destroy_pass);

    eprintln!(
        "copied activated ability departure trace: result={destroy_resolution:?}; events={:?}",
        game.canonical_event_log(),
    );
    destroy_resolution
        .expect("destroying the copied ability source must not invalidate its stack ability");
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopyExpired { target: expired, .. } if *expired == target
    )));

    resolve_top(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        19,
        "the inherited ability must resolve from its copied source incarnation after the card has reverted in its graveyard",
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source: ability_source, ability, .. }
            if *ability_source == target && *ability == "copied-ping"
    )));
    game.validate_invariants()
        .expect("departed copied-card ability source remains provenance-valid");
}
