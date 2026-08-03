//! Red regression: a copied token can pay a sacrifice activation and retain
//! its ability-source provenance after it ceases to exist.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding,
    AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, ManaCost, PlayerId,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-SACRIFICE-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-SACRIFICE-AURA";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
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
        supported_rules: &["copied-token-sacrifice-activation-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
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

#[test]
#[allow(clippy::too_many_lines)] // The copied-token activation/departure transcript is the contract.
fn copied_token_sacrifice_activation_keeps_historical_stack_provenance() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
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
        [ActivatedAbilityBinding {
            card_definition: CREATURE,
            ability: ActivatedAbility {
                id: "sacrifice-copied-token",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
        [TriggeredAbilityBinding {
            card_definition: COPY_AURA,
            ability: TriggeredAbility {
                id: "copy-attached-creature-at-upkeep",
                condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
            },
        }],
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
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, creature)
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
        .expect("upkeep ability creates one copied token");

    game.clear_event_log();
    game.activate_ability(
        controller,
        AbilityActivation {
            source: token,
            ability_id: "sacrifice-copied-token",
            sacrifice_sources: vec![token],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("copied token may pay the inherited sacrifice activation");
    eprintln!(
        "copied token sacrifice activation trace={:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated { source, ability, .. }
            if *source == token && *ability == "sacrifice-copied-token"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token
    )));
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        21
    );
    game.validate_invariants()
        .expect("departed copied-token activation remains auditable");
}
