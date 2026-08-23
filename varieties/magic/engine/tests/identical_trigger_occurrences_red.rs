//! RED regression: one source can create more than one identical trigger in
//! a simultaneous event, and each occurrence must remain separately orderable.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
    Zone,
};

const OBSERVER: &str = "TST-IDENTICAL-TRIGGER-OBSERVER";
const VICTIM: &str = "TST-IDENTICAL-TRIGGER-VICTIM";
const SWEEP: &str = "TST-IDENTICAL-TRIGGER-SWEEP";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types: BTreeSet<_> = card_types.into_iter().collect();
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: card_types.clone(),
        is_basic_land: false,
        supported_rules: &["synthetic-identical-trigger-occurrence-contract"],
        power: card_types.contains(&CardType::Creature).then_some(1),
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The complete simultaneous-death, ordering, and stack provenance trace is one regression boundary.
fn simultaneous_deaths_keep_identical_source_trigger_occurrences_distinct() {
    let binding = TriggeredAbilityBinding {
        card_definition: OBSERVER,
        ability: TriggeredAbility {
            id: "another-creature-died",
            condition: TriggerCondition::AnotherCreatureDies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(OBSERVER, [CardType::Creature], Some(1), vec![]),
            definition(VICTIM, [CardType::Creature], Some(1), vec![]),
            definition(
                SWEEP,
                [CardType::Sorcery],
                None,
                vec![Effect::DestroyAllNonTokenCreatures],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("synthetic trigger game initializes");
    let observer = game
        .put_on_battlefield(PlayerId(0), OBSERVER)
        .expect("observer begins on the battlefield");
    game.put_on_battlefield(PlayerId(0), VICTIM)
        .expect("first victim begins on the battlefield");
    game.put_on_battlefield(PlayerId(1), VICTIM)
        .expect("second victim begins on the battlefield");
    let sweep = game
        .add_card(PlayerId(0), SWEEP, Zone::Hand)
        .expect("sweep begins in the caster hand");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sweep,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the destroy-all spell casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the destroy-all spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves the destroy-all spell");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("active player view")
        .pending_decision
        .expect("one source's two identical death triggers require ordering");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(decision.trigger_candidates.len(), 2);
    assert!(
        decision
            .trigger_candidates
            .iter()
            .all(|entry| entry.source == observer
                && entry.source_incarnation == 1
                && entry.ability == "another-creature-died"),
        "both entries originate from the same source and ability"
    );
    assert_ne!(
        decision.trigger_candidates[0], decision.trigger_candidates[1],
        "the public decision needs an occurrence discriminator, not just source/ability identity"
    );
    let reversed = vec![
        decision.trigger_candidates[1],
        decision.trigger_candidates[0],
    ];
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::TriggerOrder(reversed.clone()),
    )
    .expect("the controller can order both distinct trigger occurrences");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityOrderChosen { controller, order }
            if *controller == PlayerId(0) && *order == reversed
    )));
    assert_eq!(
        game.stack
            .iter()
            .filter(|stack_object| {
                stack_object.card == observer
                    && stack_object.ability_id == Some("another-creature-died")
            })
            .count(),
        2,
        "each identical occurrence receives its own stack object"
    );
    game.validate_invariants()
        .expect("identical simultaneous trigger occurrences retain valid state-machine provenance");
}
