//! Red regression: copied tokens retain definition-bound combat triggers.
//!
//! The token is a current-turn creature but copies Haste, so it can legally
//! attack.  Its copied `Attacks` binding must then reach the stack just as the
//! physical original's binding would.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Step, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const ATTACKER: &str = "TST-COPIED-TOKEN-COMBAT-ATTACKER";
const COPY_AURA: &str = "TST-COPIED-TOKEN-COMBAT-AURA";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    keywords: Vec<Keyword>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["copied-token-combat-trigger-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords,
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_declare_attackers(game: &mut Game) {
    while game.step != Step::DeclareAttackers {
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The copy/haste/combat stack transcript is the regression contract.
fn copied_haste_token_stacks_its_attack_trigger() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                ATTACKER,
                BTreeSet::from([CardType::Creature]),
                vec![Keyword::Haste],
                vec![],
            ),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![],
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [
            TriggeredAbilityBinding {
                card_definition: COPY_AURA,
                ability: TriggeredAbility {
                    id: "copy-attached-creature-at-upkeep",
                    condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
                },
            },
            TriggeredAbilityBinding {
                card_definition: ATTACKER,
                ability: TriggeredAbility {
                    id: "copied-token-attacks",
                    condition: TriggerCondition::Attacks,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
        ],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: COPY_AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::ControlledCreature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("copy Aura binding registers");
    let attacker = game
        .put_on_battlefield(controller, ATTACKER)
        .expect("attacker setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, attacker)
        .expect("pregame Aura setup attaches");
    game.begin_game()
        .expect("game begins at the attached controller upkeep");
    pass_pair(&mut game);
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("upkeep ability creates one copied attacker token");

    advance_to_declare_attackers(&mut game);
    let attack_start = game.event_log.len();
    game.declare_attackers(controller, &[token])
        .expect("haste token attacks on the turn it was created");
    eprintln!(
        "copied token attack-trigger trace={:?}",
        &game.canonical_event_log()[attack_start..]
    );
    assert!(
        game.event_log[attack_start..].iter().any(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == token && *ability == "copied-token-attacks"
            )
        }),
        "a copied token's copied attack trigger must stack after it attacks"
    );
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        21,
        "the copied token's attack trigger resolves"
    );
    game.validate_invariants()
        .expect("copied token combat trigger lifecycle is auditable");
}
