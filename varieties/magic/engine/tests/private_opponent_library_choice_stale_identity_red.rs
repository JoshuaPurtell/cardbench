//! Red regression: a private opponent-library choice cannot survive its
//! source's battlefield-to-hand round trip and answer a later activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, PolicyAction, Step, Target,
    Zone,
};

const SOURCE: &str = "TST-PRIVATE-OPPONENT-STALE-SOURCE";
const BOUNCE: &str = "TST-PRIVATE-OPPONENT-STALE-BOUNCE";
const ABILITY: &str = "inspect-opponent-library";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let is_creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["private-opponent-library-choice-stale-identity-red"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_precombat_main(game: &mut Game) {
    pass_pair(game);
    pass_pair(game);
    assert_eq!(game.step, Step::PrecombatMain);
}

fn cast_and_resolve(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    targets: Vec<Target>,
) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost spell casts");
    pass_pair(game);
}

fn activate_choice(
    game: &mut Game,
    controller: PlayerId,
    opponent: PlayerId,
    source: cardbench_magic_engine::ObjectId,
) {
    game.activate_ability(
        controller,
        AbilityActivation {
            source,
            ability_id: ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(opponent)],
        },
    )
    .expect("zero-cost inspect ability activates");
    pass_pair(game);
}

#[test]
#[allow(clippy::too_many_lines)] // The transcript establishes same-object re-entry before stale action replay.
fn stale_private_opponent_library_choice_cannot_answer_a_reentered_source_activation() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            definition(SOURCE, CardType::Creature, vec![]),
            definition(
                BOUNCE,
                CardType::Instant,
                vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: ABILITY,
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Opponent],
                effects: vec![Effect::LookAtTopCardsOfTargetOpponentExileOne { count: 1 }],
            },
        }],
    )
    .expect("fixture builds");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("source begins on battlefield");
    let bounce = game
        .add_card(controller, BOUNCE, Zone::Hand)
        .expect("bounce begins in hand");
    game.begin_game().expect("game begins");
    advance_to_precombat_main(&mut game);

    activate_choice(&mut game, controller, opponent, source);
    let first_choice = game
        .view_for_player(controller)
        .expect("controller receives first choice")
        .private_opponent_library_choice
        .expect("first empty private choice opens");
    assert!(first_choice.cards.is_empty());
    let first_incarnation = game
        .object(source)
        .expect("source remains live")
        .incarnation;
    game.submit_policy_move(
        controller,
        "private-opponent-library-stale.first-empty.v1",
        PolicyAction::ChoosePrivateOpponentLibraryCardToExile {
            source,
            ability: ABILITY,
            selected: None,
        },
    )
    .expect("first empty choice resolves");

    cast_and_resolve(
        &mut game,
        controller,
        bounce,
        vec![Target::Permanent(source)],
    );
    assert_eq!(game.zone_of(source), Some(Zone::Hand));
    let returned_incarnation = game.object(source).expect("source returns").incarnation;
    assert!(returned_incarnation > first_incarnation);
    cast_and_resolve(&mut game, controller, source, vec![]);
    assert_eq!(game.zone_of(source), Some(Zone::Battlefield));

    activate_choice(&mut game, controller, opponent, source);
    let second_choice = game
        .view_for_player(controller)
        .expect("controller receives second choice")
        .private_opponent_library_choice
        .expect("second empty private choice opens");
    assert!(second_choice.cards.is_empty());
    let second_incarnation = game
        .object(source)
        .expect("source remains live")
        .incarnation;
    assert!(second_incarnation > returned_incarnation);
    let stale_result = game.submit_policy_move(
        controller,
        "private-opponent-library-stale.replay.v1",
        PolicyAction::ChoosePrivateOpponentLibraryCardToExile {
            source,
            ability: ABILITY,
            selected: None,
        },
    );

    eprintln!(
        "private-opponent-library stale-choice red trace: first_incarnation={first_incarnation}; returned_incarnation={returned_incarnation}; second_incarnation={second_incarnation}; second_choice={second_choice:?}; result={stale_result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert!(
        stale_result.is_err(),
        "a first activation's private choice must not answer the reentered source's later activation"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PrivateOpponentLibraryChoiceOpened { source: opened, .. } if *opened == source
    )));
}
