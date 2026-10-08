//! Full-game diagnostic authority for the RAV Conclave's Blessing task.

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, ConvokeContribution, ConvokePayment, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CONCLAVES-BLESSING";
const CARD_NAME: &str = "Conclave's Blessing";
const RECRUIT_ID: &str = "RAV-BOROS-RECRUIT";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclaves_blessing.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/conclaves_blessing.rs"]
mod candidate_conclaves_blessing;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string(&report).expect("report serializes")
            );
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Conclave's Blessing PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let captured = run_match(seed, Mutation::None)?;
    validate_trace(&captured)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Conclave's Blessing shuffled full-game replay differed".to_owned());
    }
    let mutation_sensitivity = json!({
        "wrong_power_modifier_detected_in_game": mutation_detected(seed, Mutation::WrongModifier),
        "wrong_controller_count_detected_in_game": mutation_detected(seed, Mutation::WrongController),
        "aura_attachment_removed_detected_in_game": mutation_detected(seed, Mutation::NoAttachment),
        "wrong_mana_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "convoke_removed_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_power_modifier_detected_in_game",
        "wrong_controller_count_detected_in_game",
        "aura_attachment_removed_detected_in_game",
        "wrong_mana_cost_detected_in_game",
        "convoke_removed_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Conclave's Blessing mutation escaped the gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":11,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["303.4 Auras","601.2 Casting and paying spell costs","702.50 Convoke","613.4c Power/toughness effects","704 State-based actions","510 Combat"],"source_contract":"CardBench-authored semantic summary: this white Aura has Convoke, enchants a creature, and dynamically increases its toughness by two for each other creature controlled by the Aura's controller.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,
        "cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","mutation_sensitivity":mutation_sensitivity,
        "traces":[trace_json(&captured)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Conclave's Blessing source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Conclave's Blessing source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Conclave's Blessing source has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut function_count = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err(
                        "Conclave's Blessing imports differ from task source contract".to_owned(),
                    );
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.sig.ident == "definition"
                    && matches!(&item.vis,Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.block.stmts.len() == 1 =>
            {
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err(
                        "Conclave's Blessing must return a direct definition literal".to_owned(),
                    );
                };
                if !type_is(&item.sig.output, "CardDefinition")
                    || !path_is_type(&literal.path, "CardDefinition")
                    || literal.rest.is_some()
                {
                    return Err(
                        "Conclave's Blessing must return a direct definition literal".to_owned(),
                    );
                }
                function_count += 1;
            }
            _ => {
                return Err(
                    "Conclave's Blessing source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || function_count != 1 || file.items.len() != 3 {
        return Err("Conclave's Blessing module structure differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_conclaves_blessing::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Conclave's Blessing definition is missing from RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![
                    ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledBySource {
                        power_per_creature: 0,
                        toughness_per_creature: 2,
                    },
                ],
            }]
    {
        return Err("Conclave's Blessing literal is outside the task source contract".to_owned());
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from source contract".to_owned());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongModifier,
    WrongController,
    NoAttachment,
    WrongCost,
    NoConvoke,
}
#[derive(Clone, Debug, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    aura: Option<ObjectId>,
    target: Option<ObjectId>,
    payment: Vec<Color>,
    target_controller: Option<usize>,
    power: Option<i32>,
    toughness: Option<i32>,
    source_creatures_at_attachment: usize,
    convokers: Vec<ObjectId>,
    convoker_tapped_at_cast: Vec<bool>,
    target_attacked: bool,
    combat_creatures: Vec<ObjectId>,
    combat_damage: i32,
}

fn find_trace_seed() -> Result<u64, String> {
    let mut last = "no trace attempts".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        match run_match(seed, Mutation::None).and_then(|capture| validate_trace(&capture)) {
            Ok(()) => return Ok(seed),
            Err(error) => last = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded Conclave's Blessing duel completed; {last}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let def = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Conclave's Blessing definition missing from registry")?;
    *def = candidate_conclaves_blessing::definition();
    match mutation {
        Mutation::WrongModifier => {
            def.effects = vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![
                    ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledBySource {
                        power_per_creature: 0,
                        toughness_per_creature: 1,
                    },
                ],
            }]
        }
        Mutation::WrongController => {
            def.effects = vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![
                    ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledByTarget {
                        power_per_creature: 0,
                        toughness_per_creature: 2,
                    },
                ],
            }]
        }
        Mutation::NoAttachment => def.effects.clear(),
        Mutation::WrongCost => def.mana_cost = ManaCost::with_colors(4, [Color::White]),
        Mutation::NoConvoke => def.keywords.retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::None => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|e| e.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|e| e.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.validate_invariants().map_err(|e| e.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: RECRUIT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: RECRUIT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 8000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(BlessingPolicy::new(PlayerId(0))),
        Box::new(BlessingPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut attachment_snapshot: Option<(ObjectId, usize, Option<i32>, Option<i32>, usize)> = None;
    let mut convokers = Vec::new();
    let mut convoker_tapped_at_cast = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Conclave's Blessing duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Conclave's Blessing policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        attempted += 1;
        let debug = format!("{action:?}");
        let event_start = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy rejected at turn {} {:?}: {e}; action={debug}; mana={:?}",
                    view.turn, view.step, view.mana_pool
                )
            })?;
        accepted += 1;
        game.validate_invariants().map_err(|e| e.to_string())?;
        for event in &game.event_log[event_start..] {
            if let GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } = event
            {
                convokers.push(*creature);
                convoker_tapped_at_cast
                    .push(game.object(*creature).map_err(|e| e.to_string())?.tapped);
            }
        }
        if attachment_snapshot.is_none() {
            let aura_id = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
            if let Some((aura_id, target_id)) = aura_id.and_then(|aura_id| {
                game.event_log.iter().find_map(|event| match event {
                    GameEvent::AuraAttached { aura, target } if *aura == aura_id => {
                        Some((aura_id, *target))
                    }
                    _ => None,
                })
            }) {
                if let (Ok(controller), Ok(characteristics), Ok(view)) = (
                    game.controller_of(target_id),
                    game.characteristics(target_id),
                    game.view_for_player(PlayerId(0)),
                ) {
                    let source_creatures = view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.card_types.contains(&CardType::Creature))
                        .count();
                    attachment_snapshot = Some((
                        target_id,
                        controller.0,
                        characteristics.power,
                        characteristics.toughness,
                        source_creatures,
                    ));
                }
                let _ = aura_id;
            }
        }
    }
    let events = game.event_log.clone();
    let aura = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let target = aura.and_then(|aura| {
        events.iter().find_map(|event| match event {
            GameEvent::AuraAttached { aura: a, target } if *a == aura => Some(*target),
            _ => None,
        })
    });
    let payment = aura
        .and_then(|aura| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == aura => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let (target_controller, power, toughness, source_creatures_at_attachment) = attachment_snapshot
        .map_or(
            (None, None, None, 0),
            |(_, controller, power, toughness, source_count)| {
                (Some(controller), power, toughness, source_count)
            },
        );
    let combat_creatures: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => Some(
                attackers
                    .iter()
                    .copied()
                    .filter(|id| game.card_definition(*id).is_ok_and(|d| d.id == RECRUIT_ID))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    let target_attacked = !combat_creatures.is_empty();
    let combat_damage = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if combat_creatures.contains(source) => Some(*amount),
            _ => None,
        })
        .sum();
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        aura,
        target,
        payment,
        target_controller,
        power,
        toughness,
        source_creatures_at_attachment,
        convokers,
        convoker_tapped_at_cast,
        target_attacked,
        combat_creatures,
        combat_damage,
    })
}

struct BlessingPolicy {
    player: PlayerId,
}
impl BlessingPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn activate(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }
    fn play_land(&self, view: &GameView) -> Option<ObjectId> {
        view.hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
            .map(|card| card.id)
    }
    fn cast_recruit(&self, view: &GameView, card: ObjectId) -> Option<PolicyAction> {
        if view.mana_pool.amount(Color::White) == 0 {
            return self.activate(view, Color::White);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![],
                hybrid: vec![Color::White],
            },
        })
    }
    fn cast_blessing(
        &self,
        view: &GameView,
        card: ObjectId,
        target: ObjectId,
    ) -> Option<PolicyAction> {
        let contributors: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|permanent| {
                permanent.card_types.contains(&CardType::Creature) && !permanent.tapped
            })
            .take(3)
            .collect();
        if contributors.len() != 3 {
            return None;
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.activate(view, Color::White);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![Target::Permanent(target)],
                convoke: contributors
                    .iter()
                    .map(|creature| ConvokePayment {
                        creature: creature.id,
                        contribution: ConvokeContribution::Generic,
                    })
                    .collect(),
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
}
impl CodePolicy for BlessingPolicy {
    fn id(&self) -> &'static str {
        "rav.conclaves-blessing-source-controller-convoke.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0) {
                    if view
                        .own_battlefield
                        .iter()
                        .any(|card| card.definition == Some(CARD_ID))
                    {
                        view.own_battlefield
                            .iter()
                            .filter(|card| card.definition == Some(RECRUIT_ID) && card.can_attack)
                            .map(|card| card.id)
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                },
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = self.play_land(view)
        {
            return PolicyAction::PlayLand { card: land };
        }
        if self.player == PlayerId(0) {
            let aura_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(RECRUIT_ID))
                .map(|card| card.id);
            let own_recruits = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(RECRUIT_ID))
                .count();
            if !aura_live
                && own_recruits >= 3
                && let (Some(target), Some(aura)) = (
                    target,
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID)),
                )
                && let Some(action) = self.cast_blessing(view, aura.id, target)
            {
                return action;
            }
            if !aura_live
                && own_recruits < 3
                && let Some(recruit) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(RECRUIT_ID))
            {
                if let Some(action) = self.cast_recruit(view, recruit.id) {
                    return action;
                }
            }
        } else if !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(RECRUIT_ID))
        {
            if let Some(recruit) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(RECRUIT_ID))
            {
                if let Some(action) = self.cast_recruit(view, recruit.id) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Conclave's Blessing duel did not finish as a clean P0 win".to_owned());
    }
    let aura = c.aura.ok_or("Conclave's Blessing was not cast")?;
    let target = c.target.ok_or("Conclave's Blessing did not attach")?;
    if c.payment != [Color::White] {
        return Err(
            "Conclave's Blessing did not pay its white symbol after three generic Convoke payments"
                .to_owned(),
        );
    }
    if c.target_controller != Some(1)
        || c.power != Some(1)
        || c.source_creatures_at_attachment != 3
        || c.toughness != Some(7)
    {
        return Err(format!(
            "attached opposing creature did not count the Aura controller's three creatures: controller={:?}, P/T={:?}/{:?}, source_count={}",
            c.target_controller, c.power, c.toughness, c.source_creatures_at_attachment
        ));
    }
    if c.convokers.len() != 3
        || c.convokers.iter().collect::<BTreeSet<_>>().len() != 3
        || c.convoker_tapped_at_cast != [true, true, true]
    {
        return Err("Conclave's Blessing did not tap three distinct controlled creatures for generic Convoke".to_owned());
    }
    if !c.target_attacked || c.combat_damage < 20 {
        return Err(
            "full-game combat did not finish after Conclave's Blessing attached".to_owned(),
        );
    }
    let convoke_indices: Vec<_> = c
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event,GameEvent::ConvokeUsed{player:PlayerId(0),creature,contribution:None}if c.convokers.contains(creature))
                .then_some(index)
        })
        .collect();
    if convoke_indices.len() != 3 {
        return Err(
            "typed event log does not contain exactly three generic Convoke receipts".to_owned(),
        );
    }
    let paid = event_index(&c.events, |e| {
        matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==aura&&colors.as_slice()==[Color::White])
    })
    .ok_or("exact white mana payment receipt is missing")?;
    let cast = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==aura),
    )
    .ok_or("Aura cast receipt missing")?;
    let resolved = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellResolved{card}if *card==aura),
    )
    .ok_or("Aura resolution receipt missing")?;
    let enters = event_index(
        &c.events,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==aura),
    )
    .ok_or("Aura battlefield entry receipt missing")?;
    let attached = event_index(
        &c.events,
        |e| matches!(e,GameEvent::AuraAttached{aura:a,target:t}if *a==aura&&*t==target),
    )
    .ok_or("exact opposing Aura attachment receipt missing")?;
    if convoke_indices.iter().any(|index| *index >= paid) {
        return Err(
            "Convoke receipts did not precede the Aura's remaining white mana payment".to_owned(),
        );
    }
    let attack=event_index(&c.events,|e|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.iter().any(|id|c.events.iter().any(|ev|matches!(ev,GameEvent::DamageDealtToPlayer{source,..}if source==id))))).ok_or("legal combat attack receipt missing")?;
    let damage = event_index_after(
        &c.events,
        attack,
        |e| matches!(e,GameEvent::DamageDealtToPlayer{player:PlayerId(1),amount,..}if *amount>0),
    )
    .ok_or("positive combat damage receipt missing")?;
    let end = event_index(&c.events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < enters
        && enters < attached
        && attached < attack
        && attack < damage
        && damage < end)
    {
        return Err(
            "Conclave's Blessing Convoke/payment/attachment/combat/game-end event order differs"
                .to_owned(),
        );
    }
    Ok(())
}
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|c| validate_trace(&c))
        .is_err()
}
fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(i, e)| predicate(e).then_some(i))
}
fn trace_json(c: &Captured) -> Value {
    let aura = c.aura;
    let target = c.target;
    let projected = project_events(&c.events);
    let events: Vec<Value> = c.events.iter().enumerate().filter_map(|(index, event)| {
        let include = match event {
            GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } => c.convokers.contains(creature),
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => aura == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Battlefield } => aura == Some(*card),
            GameEvent::AuraAttached { aura: a, target: t } => aura == Some(*a) && target == Some(*t),
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => attackers.iter().any(|id| c.combat_creatures.contains(id)),
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } => *amount > 0 && c.combat_creatures.contains(source),
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let e: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence":e.sequence,"turn":e.turn,"kind":e.kind,"facts":e.facts,"canonical":e.canonical}))
    }).collect();
    json!({
        "trace_id":"rav-conclaves-blessing-convoke-aura-controller-creature-count-full-game",
        "terminal":c.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),
        "winner":c.winner,"turns":c.turns,"accepted_policy_moves":c.accepted,"rejected_policy_moves":c.rejected,
        "mechanics":{
            "shuffled_60_card_decks":true,"mana_cost":"{3}{W}",
            "paid_mana_colors":c.payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "convoke_generic_symbols_paid":c.convokers.len(),
            "convoke_creature_objects":c.convokers.iter().map(|id|id.0).collect::<Vec<_>>(),
            "convoke_creatures_tapped_at_cast":c.convoker_tapped_at_cast,
            "aura_object":aura.map(|id|id.0),"target_creature_object":target.map(|id|id.0),
            "aura_attached_to_opponent_creature":aura.is_some()&&target.is_some()&&c.target_controller==Some(1),
            "aura_controller_other_creature_count":c.source_creatures_at_attachment,
            "target_controller_seat":c.target_controller,"live_power_toughness":[c.power,c.toughness],
            "modifier_per_other_creature":[0,2],"p0_creatures_attacked":c.target_attacked,
            "combat_creature_objects":c.combat_creatures.iter().map(|id|id.0).collect::<Vec<_>>(),
            "controller_combat_damage":c.combat_damage,"deterministic_replay":true,"terminal_combat_win":c.winner==Some(0)
        },"events":events
    })
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 11;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output,syn::ReturnType::Type(_,ty)if matches!(&**ty,syn::Type::Path(path)if path.qself.is_none()&&path_is_type(&path.path,expected)))
}
fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
