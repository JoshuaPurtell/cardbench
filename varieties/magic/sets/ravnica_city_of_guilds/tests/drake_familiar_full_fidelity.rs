//! Event-log contracts for Drake Familiar's target-free return-or-sacrifice ETB.

use cardbench_magic_engine::{
    CardDefinition, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game, GameEvent,
    Keyword, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

const DRAKE_FAMILIAR: &str = "RAV-DRAKE-FAMILIAR";
const ABILITY_ID: &str = "etb-return-any-enchantment-or-sacrifice-source";
const ENCHANTMENT: &str = "RAV-DARK-HEART-OF-THE-WOOD";
const UNTARGETABLE_ENCHANTMENT: &str = "TEST-UNTARGETABLE-ENCHANTMENT";

fn game() -> Game {
    game_with_definitions(card_definitions())
}

fn game_with_definitions(definitions: Vec<CardDefinition>) -> Game {
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn game_with_untargetable_enchantment() -> Game {
    let mut definitions = card_definitions();
    let mut enchantment = definitions
        .iter()
        .find(|definition| definition.id == ENCHANTMENT)
        .expect("Dark Heart of the Wood definition exists")
        .clone();
    enchantment.id = UNTARGETABLE_ENCHANTMENT;
    enchantment.name = "Test Untargetable Enchantment";
    enchantment.keywords.push(Keyword::Shroud);
    definitions.push(enchantment);
    game_with_definitions(definitions)
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    while game.step != Step::PrecombatMain {
        pass_pair(game);
    }
}

fn add_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, "RAV-ISLAND", Zone::Library)
            .expect("fixture library card");
    }
}

fn cast_drake_and_stack_trigger(game: &mut Game, drake: cardbench_magic_engine::ObjectId) {
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("Drake Familiar mana is available");
    add_library(game, PlayerId(0));
    add_library(game, PlayerId(1));
    game.begin_game().expect("game begins");
    advance_to_precombat_main(game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: drake,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Drake Familiar casts");
    pass_pair(game);
    let trigger = game.stack.last().expect("Drake ETB trigger is stacked");
    assert_eq!(trigger.ability_id, Some(ABILITY_ID));
    assert!(
        trigger.targets.is_empty(),
        "the printed choice is not a target"
    );
    assert_eq!(
        trigger.effects,
        vec![Effect::ReturnAnyEnchantmentOrSacrificeSource]
    );
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_none(),
        "the controller chooses during resolution, after the response window"
    );
}

#[test]
fn drake_familiar_can_choose_any_battlefield_enchantment_as_a_resolution_choice() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let own_enchantment = game
        .put_on_battlefield(PlayerId(0), ENCHANTMENT)
        .expect("controller's enchantment is live");
    let opponent_enchantment = game
        .put_on_battlefield(PlayerId(1), ENCHANTMENT)
        .expect("opponent's enchantment is live");

    cast_drake_and_stack_trigger(&mut game, drake);
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("resolution opens the optional return choice");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!((decision.min_selections, decision.max_selections), (0, 1));
    let candidates = decision
        .candidates
        .iter()
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&own_enchantment));
    assert!(candidates.contains(&opponent_enchantment));
    let decision_id = decision.id;
    game.submit_decision(
        PlayerId(0),
        decision_id,
        DecisionSelection::Objects(vec![own_enchantment]),
    )
    .expect("controller chooses its own enchantment");

    assert_eq!(game.zone_of(drake), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(own_enchantment), Some(Zone::Hand));
    assert_eq!(game.zone_of(opponent_enchantment), Some(Zone::Battlefield));
    let returned = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == own_enchantment))
        .expect("owner-hand return receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == drake && *ability == ABILITY_ID))
        .expect("trigger resolution receipt");
    assert!(
        returned < resolved,
        "the chosen return happens during resolution"
    );
    assert_eq!(
        game.object(own_enchantment).expect("returned object").owner,
        PlayerId(0)
    );
    assert!(
        game.player(PlayerId(0))
            .expect("controller owner")
            .hand
            .contains(&own_enchantment)
    );
    assert!(
        !game
            .player(PlayerId(1))
            .expect("opponent")
            .hand
            .contains(&own_enchantment)
    );
    game.validate_invariants()
        .expect("chosen enchantment return preserves invariants");
}

#[test]
fn drake_familiar_may_decline_the_return_and_sacrifice_with_an_eligible_enchantment() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let own_enchantment = game
        .put_on_battlefield(PlayerId(0), ENCHANTMENT)
        .expect("controller has an eligible enchantment");

    cast_drake_and_stack_trigger(&mut game, drake);
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("eligible enchantment opens a choice");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!((decision.min_selections, decision.max_selections), (0, 1));
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![own_enchantment]
    );

    game.submit_decision(PlayerId(0), decision.id, DecisionSelection::Objects(vec![]))
        .expect("controller may decline the optional return");

    assert_eq!(game.zone_of(drake), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(own_enchantment), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::SacrificedByEffect { source, player: PlayerId(0), permanent } if *source == drake && *permanent == drake)));
    assert!(!game.event_log.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == own_enchantment)));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == drake && *ability == ABILITY_ID)));
    game.validate_invariants()
        .expect("declining the return sacrifices only Drake Familiar");
}

#[test]
fn drake_familiar_allows_a_response_before_its_non_targeting_choice() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let own_enchantment = game
        .put_on_battlefield(PlayerId(0), ENCHANTMENT)
        .expect("controller's enchantment is live");
    let opponent_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("response spell's controlled target is live");
    let peel = game
        .add_card(PlayerId(1), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("response is in hand");
    let response_lands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(1), "RAV-ISLAND")
                .expect("response mana source")
        })
        .collect::<Vec<_>>();

    cast_drake_and_stack_trigger(&mut game, drake);
    game.pass_priority(PlayerId(0))
        .expect("Drake controller passes to the response window");
    assert_eq!(game.priority, PlayerId(1));
    for island in response_lands {
        game.activate_mana_ability(PlayerId(1), island, Color::Blue)
            .expect("opponent can activate mana while the trigger waits");
    }
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: peel,
            targets: vec![
                Target::Permanent(opponent_creature),
                Target::Permanent(drake),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent can respond before the trigger's non-targeting choice");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(drake), Some(Zone::Hand));
    assert_eq!(
        game.stack.last().expect("ETB trigger remains").ability_id,
        Some(ABILITY_ID)
    );

    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("trigger controller view")
        .pending_decision
        .expect("the live ETB trigger resolves after its source left");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![own_enchantment]
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![own_enchantment]),
    )
    .expect("the trigger may still return its controller's enchantment");
    assert_eq!(game.zone_of(own_enchantment), Some(Zone::Hand));
    assert_eq!(game.zone_of(drake), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == drake && *ability == ABILITY_ID)));
    game.validate_invariants()
        .expect("response and source departure preserve invariants");
}

#[test]
fn drake_familiar_returns_an_opponent_controlled_enchantment_to_its_owners_hand() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let opponent_enchantment = game
        .put_on_battlefield(PlayerId(1), ENCHANTMENT)
        .expect("opponent controls an enchantment");

    cast_drake_and_stack_trigger(&mut game, drake);
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("the opponent enchantment is an eligible choice");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![opponent_enchantment],
        "any battlefield enchantment is available, regardless of its controller"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![opponent_enchantment]),
    )
    .expect("the controller returns an opponent-controlled enchantment");

    assert_eq!(game.zone_of(drake), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(opponent_enchantment), Some(Zone::Hand));
    assert_eq!(
        game.object(opponent_enchantment)
            .expect("returned enchantment")
            .owner,
        PlayerId(1)
    );
    assert!(
        game.player(PlayerId(1))
            .expect("owner player")
            .hand
            .contains(&opponent_enchantment)
    );
    assert!(
        !game
            .player(PlayerId(0))
            .expect("controller player")
            .hand
            .contains(&opponent_enchantment)
    );
    assert!(!game.event_log.iter().any(|event| matches!(event, GameEvent::SacrificedByEffect { source, permanent, .. } if *source == drake && *permanent == drake)));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == drake && *ability == ABILITY_ID)));
    game.validate_invariants()
        .expect("returning the opponent's enchantment preserves invariants");
}

#[test]
fn drake_familiar_can_return_an_opponents_untargetable_enchantment() {
    let mut game = game_with_untargetable_enchantment();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");
    let opponent_enchantment = game
        .put_on_battlefield(PlayerId(1), UNTARGETABLE_ENCHANTMENT)
        .expect("opponent's untargetable enchantment enters");

    cast_drake_and_stack_trigger(&mut game, drake);
    assert!(game.stack.last().expect("ETB trigger").targets.is_empty());
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("trigger controller view")
        .pending_decision
        .expect("target-free choice includes the untargetable enchantment");
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![opponent_enchantment]
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![opponent_enchantment]),
    )
    .expect("shroud does not prevent a non-targeting resolution choice");

    assert_eq!(game.zone_of(opponent_enchantment), Some(Zone::Hand));
    assert!(
        game.player(PlayerId(1))
            .expect("owner player")
            .hand
            .contains(&opponent_enchantment)
    );
    assert_eq!(game.zone_of(drake), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("returning an untargetable enchantment preserves invariants");
}

#[test]
fn drake_familiar_sacrifices_itself_when_no_enchantment_can_be_returned() {
    let mut game = game();
    let drake = game
        .add_card(PlayerId(0), DRAKE_FAMILIAR, Zone::Hand)
        .expect("Drake Familiar begins in hand");

    cast_drake_and_stack_trigger(&mut game, drake);
    pass_pair(&mut game);

    assert_eq!(game.zone_of(drake), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::SacrificedByEffect { source, player: PlayerId(0), permanent } if *source == drake && *permanent == drake)));
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_none()
    );
    game.validate_invariants()
        .expect("no-enchantment branch preserves invariants");
}
