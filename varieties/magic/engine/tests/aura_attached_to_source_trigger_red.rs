//! Red regression for Aura attachment observers bound to the enchanted source.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, ManaCost,
    PlayerId, Step, TargetRequirement, TokenSpec, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const HOST: &str = "TST-AURA-ATTACHED-HOST";
const OTHER: &str = "TST-AURA-ATTACHED-OTHER";
const AURA: &str = "TST-AURA-ATTACHED-AURA";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["aura-attached-to-source-trigger-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn game() -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(HOST, CardType::Creature, vec![]),
            definition(OTHER, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: HOST,
            ability: TriggeredAbility {
                id: "aura-attached-create-two",
                condition: TriggerCondition::AuraBecomesAttachedToSource,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 2,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding registers");
    let host = game
        .put_on_battlefield(PlayerId(0), HOST)
        .expect("trigger source enters fixture");
    let other = game
        .put_on_battlefield(PlayerId(0), OTHER)
        .expect("other creature enters fixture");
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..8 {
            game.add_card(player, OTHER, Zone::Library)
                .expect("draw-step card enters library");
        }
    }
    (game, host, other)
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("game begins");
    for _ in 0..32 {
        if game.step == Step::PrecombatMain {
            return;
        }
        let active = game.active_player;
        if game
            .view_for_player(active)
            .expect("active view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(active, None)
                .expect("ordinary draw resolves");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(active)
                    .expect("attacker view")
                    .attackers_declared =>
            {
                game.declare_attackers(active, &[])
                    .expect("empty attackers are declared");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("blocker view")
                    .blockers_declared =>
            {
                game.declare_blockers(game.next_policy_player(), &[])
                    .expect("empty blockers are declared");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority).expect("turn advances");
            }
        }
    }
    panic!("game did not reach first main phase");
}

#[test]
fn aura_attachment_to_exact_source_triggers_under_source_controller() {
    let (mut game, host, _) = game();
    // The Aura is owned and controlled by seat one, while the attached source
    // belongs to seat zero. The trigger must still be controlled by seat zero.
    let aura = game
        .add_card(PlayerId(1), AURA, Zone::Hand)
        .expect("opponent Aura enters hand");
    advance_to_first_main(&mut game);
    game.enter_attachment_without_cast(aura, host)
        .expect("opponent-controlled Aura attaches to host");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == host && *ability == "aura-attached-create-two"
    )));
    assert_eq!(
        game.stack.last().expect("trigger is on stack").controller,
        PlayerId(0)
    );
    game.validate_invariants().expect("trigger state is valid");
}

#[test]
fn aura_attached_to_a_different_creature_does_not_trigger_host() {
    let (mut game, host, other) = game();
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura enters hand");
    advance_to_first_main(&mut game);
    game.enter_attachment_without_cast(aura, other)
        .expect("Aura attaches to other creature");

    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == host && *ability == "aura-attached-create-two"
    )));
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("no-trigger state is valid");
}
