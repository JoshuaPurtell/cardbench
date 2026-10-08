//! Full-game PvP authority for RAV Gate Hound.

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GATE-HOUND";
const CARD_NAME: &str = "Gate Hound";
const AURA_ID: &str = "RAV-FISTS-OF-IRONWOOD";
const TRIGGER_ID: &str = "etb-two-saprolings";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/gate_hound.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8000;
const SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/gate_hound.rs"]
mod candidate_gate_hound;

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
            eprintln!("Gate Hound PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let capture = run_match(Mutation::None)?;
    validate_trace(&capture)?;
    let replay = run_match(Mutation::None)?;
    if capture != replay {
        return Err("Gate Hound full-game replay was not deterministic".into());
    }
    let mutations = json!({
        "conditional_vigilance_removed_detected_in_game": mutation_detected(Mutation::NoVigilance),
        "enchantment_condition_removed_detected_in_game": mutation_detected(Mutation::AlwaysVigilance),
        "mana_cost_changed_detected_in_game": mutation_detected(Mutation::WrongManaCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "conditional_vigilance_removed_detected_in_game",
        "enchantment_condition_removed_detected_in_game",
        "mana_cost_changed_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Gate Hound mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":19,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["302.1 Creature characteristics","303.4 Auras","601.2 Casting spells","608.2 Resolving spells","613 Continuous effects","702.20 Vigilance","510.1 Attacking creatures and tapping"],"source_contract":"CardBench-authored semantic summary: Gate Hound is a white 2W 1/1 creature. While it is enchanted, creatures its controller controls have vigilance.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,"mutation_sensitivity":mutations,"traces":[trace_json(&capture)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Gate Hound source: {e}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Gate Hound source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Gate Hound source has unsupported module attributes".into());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "StaticContinuousEffectBinding"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|parts| parts.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Gate Hound imports differ from source contract".into());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if matches!(&item.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let (name, output) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "static_continuous_effect" => {
                        ("static_continuous_effect", "StaticContinuousEffectBinding")
                    }
                    _ => return Err("Gate Hound has unsupported function".into()),
                };
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err("Gate Hound must return direct typed literals".into());
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                {
                    return Err("Gate Hound must return direct typed literals".into());
                }
                functions.insert(name.to_owned());
            }
            _ => return Err("Gate Hound contains an unsupported top-level source item".into()),
        }
    }
    if imports != expected
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "static_continuous_effect".to_owned(),
            ])
        || file.items.len() != 4
    {
        return Err("Gate Hound module structure differs from source contract".into());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_gate_hound::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|d| d.id == CARD_ID)
        .ok_or("Gate Hound definition is missing from RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || definition.is_basic_land
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Gate Hound literal is outside the task source contract".into());
    }
    let binding = candidate_gate_hound::static_continuous_effect();
    let registered = rav_static_continuous_effect_bindings()
        .into_iter()
        .find(|b| b.card_definition == CARD_ID)
        .ok_or("Gate Hound conditional Vigilance binding is missing from RAV registry")?;
    if binding != registered
        || binding.change
            != ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(Keyword::Vigilance)
    {
        return Err("Gate Hound static binding is outside the task source contract".into());
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
        return Err("RAV catalog denominator differs from source contract".into());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoVigilance,
    AlwaysVigilance,
    WrongManaCost,
}

#[derive(Clone, Debug, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    hound: Option<ObjectId>,
    aura: Option<ObjectId>,
    aura_target: Option<ObjectId>,
    aura_on_battlefield: bool,
    token_objects: Vec<ObjectId>,
    unenchanted_creatures: Vec<ObjectId>,
    unenchanted_no_vigilance: Vec<bool>,
    enchanted_creatures: Vec<ObjectId>,
    enchanted_vigilance: Vec<bool>,
    attacking_hound: bool,
    attacking_companion: bool,
    attacking_creatures: Vec<ObjectId>,
    all_attackers_untapped: bool,
    hound_damage: i32,
    total_combat_damage: i32,
}

fn run_match(mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let hound_definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Gate Hound definition missing")?;
    *hound_definition = candidate_gate_hound::definition();
    if mutation == Mutation::WrongManaCost {
        hound_definition.mana_cost = ManaCost::with_colors(1, [Color::White]);
    }
    let mut static_bindings = rav_static_continuous_effect_bindings();
    let binding_index = static_bindings
        .iter()
        .position(|b| b.card_definition == CARD_ID)
        .ok_or("Gate Hound static binding missing")?;
    static_bindings[binding_index] = candidate_gate_hound::static_continuous_effect();
    match mutation {
        Mutation::NoVigilance => {
            static_bindings[binding_index].change =
                ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(Keyword::Flying)
        }
        Mutation::AlwaysVigilance => {
            static_bindings[binding_index].change =
                ContinuousChange::ControlledCreaturesAddKeyword(Keyword::Vigilance)
        }
        Mutation::None | Mutation::WrongManaCost => {}
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
        static_bindings,
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: AURA_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 30,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("Gate Hound task deck", &deck0),
        ("opponent task deck", &deck1),
    ] {
        let count: usize = deck.mainboard.iter().map(|e| usize::from(e.count)).sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards, expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: SEED,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(SEED).map_err(|e| e.to_string())?;
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
        Box::new(GateHoundPolicy::new(PlayerId(0))),
        Box::new(GateHoundPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    let mut hound = None;
    let mut aura = None;
    let mut aura_target = None;
    let mut token_objects: Vec<ObjectId> = Vec::new();
    let mut aura_on_battlefield = false;
    let mut unenchanted_creatures: Vec<ObjectId> = Vec::new();
    let mut unenchanted_no_vigilance = Vec::new();
    let mut enchanted_creatures: Vec<ObjectId> = Vec::new();
    let mut enchanted_vigilance = Vec::new();
    let mut attacking_creatures: Vec<ObjectId> = Vec::new();
    let mut all_attackers_untapped = false;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err(format!(
                "Gate Hound duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(a) = policy.propose_pending_decision(&view) {
            a
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        let old_len = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "Gate Hound policy rejected at turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        for event in &game.event_log[old_len..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    hound = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game.card_definition(*card).is_ok_and(|d| d.id == AURA_ID) => {
                    aura = Some(*card)
                }
                GameEvent::AuraAttached { aura: a, target } if Some(*a) == aura => {
                    aura_target = Some(*target)
                }
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token,
                } => token_objects.push(*token),
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } => {
                    let current: Vec<_> = attackers
                        .iter()
                        .filter(|id| enchanted_creatures.contains(id))
                        .copied()
                        .collect();
                    let current_untapped = current
                        .iter()
                        .all(|id| game.object(*id).is_ok_and(|object| !object.tapped));
                    if !current.is_empty() {
                        all_attackers_untapped = if attacking_creatures.is_empty() {
                            current_untapped
                        } else {
                            all_attackers_untapped && current_untapped
                        };
                    }
                    for id in current {
                        if !attacking_creatures.contains(&id) {
                            attacking_creatures.push(id);
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(hound_id) = hound {
            if aura.is_none() {
                let is_battlefield = game.zone_of(hound_id) == Some(Zone::Battlefield);
                if is_battlefield {
                    let view = game
                        .view_for_player(PlayerId(0))
                        .map_err(|e| e.to_string())?;
                    let creatures: Vec<_> = view
                        .own_battlefield
                        .iter()
                        .filter(|c| c.card_types.contains(&CardType::Creature))
                        .collect();
                    if !creatures.is_empty() {
                        unenchanted_creatures = creatures.iter().map(|c| c.id).collect();
                        unenchanted_no_vigilance = creatures
                            .iter()
                            .map(|c| !c.keywords.contains(&Keyword::Vigilance))
                            .collect();
                    }
                }
            }
        }
        if let (Some(aura_id), Some(target_id)) = (aura, aura_target) {
            if !aura_on_battlefield && game.zone_of(aura_id) == Some(Zone::Battlefield) {
                aura_on_battlefield = true;
                if game.controller_of(target_id).ok() != Some(PlayerId(0)) {
                    return Err(
                        "Fists of Ironwood attached outside its controller's battlefield".into(),
                    );
                }
            }
        }
        if aura_on_battlefield
            && enchanted_creatures.is_empty()
            && game.event_log.iter().any(|event| {
                matches!(event,
                GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. }
                    if Some(*source) == aura)
            })
        {
            let view = game
                .view_for_player(PlayerId(0))
                .map_err(|e| e.to_string())?;
            let creatures: Vec<_> = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .collect();
            enchanted_creatures = creatures.iter().map(|card| card.id).collect();
            enchanted_vigilance = creatures
                .iter()
                .map(|card| card.keywords.contains(&Keyword::Vigilance))
                .collect();
        }
    }
    let events = game.event_log.clone();
    let hound_id = hound;
    let aura_id = aura;
    let attached_to = aura_target;
    let aura_still_live = aura_id.is_some_and(|id| game.zone_of(id) == Some(Zone::Battlefield));
    let hound_damage = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == hound_id => Some(*amount),
            _ => None,
        })
        .sum();
    let total_combat_damage = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if attacking_creatures.contains(source) => Some(*amount),
            _ => None,
        })
        .sum();
    let attacking_hound = attacking_creatures.contains(&hound.unwrap_or(ObjectId(u64::MAX)));
    let attacking_companion = attacking_creatures.iter().any(|id| Some(*id) != hound);
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        hound: hound_id,
        aura: aura_id,
        aura_target: attached_to,
        aura_on_battlefield: aura_still_live,
        token_objects,
        unenchanted_creatures,
        unenchanted_no_vigilance,
        enchanted_creatures,
        enchanted_vigilance,
        attacking_hound,
        attacking_companion,
        attacking_creatures,
        all_attackers_untapped,
        hound_damage,
        total_combat_damage,
    })
}

struct GateHoundPolicy {
    player: PlayerId,
    hound_cast: bool,
    aura_cast: bool,
}
impl GateHoundPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            hound_cast: false,
            aura_cast: false,
        }
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|c| (c, view.mana_pool.amount(c)))
            .collect();
        for color in &cost.colored {
            let (_, n) = available.iter_mut().find(|(c, _)| c == color)?;
            if *n == 0 {
                return None;
            }
            *n -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, n) = available.iter_mut().max_by_key(|(_, n)| *n)?;
            if *n == 0 {
                return None;
            }
            *n -= 1;
            generic.push(*color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn cast(
        &mut self,
        view: &GameView,
        id: &'static str,
        cost: ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view.hand.iter().find(|c| c.definition == Some(id))?;
            match id {
                CARD_ID => self.hound_cast = true,
                AURA_ID => self.aura_cast = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|c| view.mana_pool.amount(**c) == 0)
            .copied();
        let land = view
            .own_battlefield
            .iter()
            .find(|c| {
                c.card_types.contains(&CardType::Land)
                    && !c.tapped
                    && missing.is_none_or(|color| c.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land) && !c.tapped)
            })?;
        let color = missing
            .filter(|c| land.mana_colors.contains(c))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}
impl CodePolicy for GateHoundPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.gate-hound-enchanted-controller-vigilance.v1"
        } else {
            "rav.gate-hound-no-blocks.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|c| c.can_attack && c.card_types.contains(&CardType::Creature))
                    .map(|c| c.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|c| c.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            if !self.hound_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|c| c.definition == Some(CARD_ID))
                && view.hand.iter().any(|c| c.definition == Some(CARD_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    ManaCost::with_colors(2, [Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
            if !self.aura_cast && view.hand.iter().any(|c| c.definition == Some(AURA_ID)) {
                if let Some(hound) = view
                    .own_battlefield
                    .iter()
                    .find(|c| c.definition == Some(CARD_ID))
                {
                    if let Some(action) = self.cast(
                        view,
                        AURA_ID,
                        ManaCost::with_colors(1, [Color::Green]),
                        vec![Target::Permanent(hound.id)],
                    ) {
                        return action;
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Gate Hound duel did not end in clean P0 win".into());
    }
    let hound = c.hound.ok_or("Gate Hound was not cast")?;
    let aura = c.aura.ok_or("Fists of Ironwood was not cast")?;
    let target = c.aura_target.ok_or("Fists of Ironwood did not attach")?;
    if target != hound || !c.aura_on_battlefield {
        return Err("Gate Hound's Fists of Ironwood Aura attachment differs".into());
    }
    if c.unenchanted_creatures.is_empty()
        || c.unenchanted_creatures.len() != c.unenchanted_no_vigilance.len()
        || c.unenchanted_no_vigilance.iter().any(|v| !*v)
        || !c.unenchanted_creatures.contains(&hound)
    {
        return Err(format!(
            "Gate Hound pre-enchantment snapshot is wrong: creatures={:?} no_vigilance={:?} hound={hound:?}",
            c.unenchanted_creatures, c.unenchanted_no_vigilance
        ));
    }
    if c.token_objects.len() != 2
        || c.enchanted_creatures.len() < 3
        || c.enchanted_creatures.len() != c.enchanted_vigilance.len()
        || c.enchanted_vigilance.iter().any(|v| !*v)
        || !c.enchanted_creatures.contains(&hound)
        || c.token_objects
            .iter()
            .any(|id| !c.enchanted_creatures.contains(id))
    {
        return Err("Gate Hound did not grant Vigilance to its controller's Hound and Saprolings after enchantment".into());
    }
    if !c.attacking_hound || !c.attacking_companion || !c.all_attackers_untapped {
        return Err("Gate Hound and another controlled creature must attack and remain untapped through combat".into());
    }
    if c.total_combat_damage < 20 {
        return Err("Gate Hound duel did not continue through a terminal combat win".into());
    }
    let gate_paid=event_index(&c.events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==hound&&colors.len()==3&&colors.iter().filter(|x|**x==Color::White).count()==1)).ok_or("Gate Hound exact {2}{W} payment missing")?;
    let gate_cast = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==hound),
    )
    .ok_or("Gate Hound cast receipt missing")?;
    let aura_paid=event_index(&c.events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==aura&&colors.len()==2&&colors.iter().filter(|x|**x==Color::Green).count()==1)).ok_or("Fists exact {1}{G} payment missing")?;
    let aura_cast = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==aura),
    )
    .ok_or("Fists cast receipt missing")?;
    let aura_resolved = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellResolved{card}if *card==aura),
    )
    .ok_or("Fists resolution missing")?;
    let attached = event_index(
        &c.events,
        |e| matches!(e,GameEvent::AuraAttached{aura:a,target:t}if *a==aura&&*t==target),
    )
    .ok_or("exact Fists attachment missing")?;
    let trigger = event_index(&c.events, |event| matches!(event,
        GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. }
            if *source == aura)).ok_or("Fists ETB token trigger did not stack")?;
    let token_events: Vec<_> = c
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } if c.token_objects.contains(token) => Some((index, *token)),
            _ => None,
        })
        .collect();
    if token_events.len() != 2
        || token_events
            .iter()
            .any(|(_, token)| !c.token_objects.contains(token))
    {
        return Err("Fists must create exactly two Saproling tokens".into());
    }
    let ability = event_index_after(&c.events, trigger, |event| {
        matches!(event,
        GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == aura)
    })
    .ok_or("Fists ETB token trigger did not resolve")?;
    let attacks=event_index(&c.events,|e|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&hound)&&c.token_objects.iter().any(|id|attackers.contains(id)))).ok_or("Gate Hound and an enchanted Saproling attack receipt missing")?;
    let damage = event_index_after(
        &c.events,
        attacks,
        |e| matches!(e,GameEvent::DamageDealtToPlayer{player:PlayerId(1),amount,..}if *amount>0),
    )
    .ok_or("post-attack combat damage receipt missing")?;
    let end = event_index(&c.events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win missing")?;
    if !(gate_paid < gate_cast
        && gate_cast < aura_paid
        && aura_paid < aura_cast
        && aura_cast < aura_resolved
        && aura_resolved < attached
        && attached < trigger
        && trigger < token_events[0].0
        && token_events[0].0 < token_events[1].0
        && token_events[1].0 < ability
        && ability < attacks
        && attacks < damage
        && damage < end)
    {
        return Err("Gate Hound cast/attachment/combat order differs".into());
    }
    Ok(())
}
fn mutation_detected(m: Mutation) -> bool {
    run_match(m).and_then(|c| validate_trace(&c)).is_err()
}
fn event_index(events: &[GameEvent], p: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(p)
}
fn event_index_after(
    events: &[GameEvent],
    start: usize,
    p: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(i, e)| p(e).then_some(i))
}
fn trace_json(c: &Captured) -> Value {
    let hound = c.hound;
    let aura = c.aura;
    let target = c.aura_target;
    let projected = project_events(&c.events);
    let events: Vec<Value> = c.events.iter().enumerate().filter_map(|(i, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => hound == Some(*card) || aura == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Battlefield } => hound == Some(*card) || aura == Some(*card),
            GameEvent::AuraAttached { aura: source, target: attached } => aura == Some(*source) && target == Some(*attached),
            GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } | GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } => aura == Some(*source),
            GameEvent::TokenCreated { player: PlayerId(0), token } => c.token_objects.contains(token),
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => attackers.iter().any(|id| c.attacking_creatures.contains(id)),
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } => *amount > 0 && c.attacking_creatures.contains(source),
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let event: &ProjectedEvent = projected.get(i)?;
        Some(json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical}))
    }).collect();
    json!({
        "trace_id":"rav-gate-hound-controller-vigilance-while-enchanted",
        "terminal":c.winner.map_or_else(||"Draw".to_owned(),|seat|format!("Winner(PlayerId({seat}))")),
        "winner":c.winner,"turns":c.turns,"accepted_policy_moves":c.accepted,"rejected_policy_moves":c.rejected,
        "mechanics":{
            "mana_cost":"{2}{W}","gate_hound_object":hound.map(|id|id.0),"aura_object":aura.map(|id|id.0),
            "aura_target_object":target.map(|id|id.0),"aura_attached_to_gate_hound":hound.is_some()&&target==hound,
            "aura_remained_attached":c.aura_on_battlefield,
            "unenchanted_creature_objects":c.unenchanted_creatures.iter().map(|id|id.0).collect::<Vec<_>>(),
            "unenchanted_creatures_without_vigilance":!c.unenchanted_no_vigilance.is_empty()&&c.unenchanted_no_vigilance.iter().all(|v|*v),
            "enchanted_creature_objects":c.enchanted_creatures.iter().map(|id|id.0).collect::<Vec<_>>(),
            "saproling_objects":c.token_objects.iter().map(|id|id.0).collect::<Vec<_>>(),
            "all_enchanted_controller_creatures_have_vigilance":!c.enchanted_vigilance.is_empty()&&c.enchanted_vigilance.iter().all(|v|*v),
            "gate_hound_attacked":c.attacking_hound,"another_enchanted_creature_attacked":c.attacking_companion,
            "attacking_creature_objects":c.attacking_creatures.iter().map(|id|id.0).collect::<Vec<_>>(),
            "all_attacking_enchanted_creatures_untapped":c.all_attackers_untapped,
            "gate_hound_combat_damage":c.hound_damage,"controller_combat_damage":c.total_combat_damage,
            "deterministic_replay":true,"terminal_combat_win":c.winner==Some(0)&&c.total_combat_damage>=20
        },"events":events
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|a| a == "--candidate") {
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
    let passed = card.collector_number == 19;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(i) => {
            let mut p = prefix.to_vec();
            p.push(i.ident.to_string());
            flatten_use_tree(&p, &i.tree, paths)
        }
        UseTree::Name(i) => {
            let mut p = prefix.to_vec();
            p.push(i.ident.to_string());
            paths.push(p);
            true
        }
        UseTree::Group(g) => g.items.iter().all(|i| flatten_use_tree(prefix, i, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output,syn::ReturnType::Type(_,ty)if matches!(&**ty,syn::Type::Path(p)if p.qself.is_none()&&path_is_type(&p.path,expected)))
}
fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
