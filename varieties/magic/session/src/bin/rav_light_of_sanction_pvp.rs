//! Full-game RAV Light of Sanction source-relative damage-prevention authority.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, ContinuousChange, DeckEntry, DeckList, Game,
    GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, StaticContinuousEffectBinding, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LIGHT-OF-SANCTION";
const CARD_NAME: &str = "Light of Sanction";
const CHAR_ID: &str = "RAV-CHAR";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SKYKNIGHT_ID: &str = "RAV-SKYKNIGHT-LEGIONNAIRE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs";
const MAX_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs"]
mod candidate_light_of_sanction;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Light of Sanction PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Light of Sanction PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog.iter().any(|card| card.collector_number == 24 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Light of Sanction identity differs".to_owned());
    }

    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.light != replay.light
        || captured.watchwolf != replay.watchwolf
        || captured.pilot_char != replay.pilot_char
        || captured.opponent_char != replay.opponent_char
        || captured.skyknight != replay.skyknight
    {
        return Err("Light of Sanction full-game replay differs".to_owned());
    }

    let mutations = json!({
        "static_binding_removal_detected": mutation_detected(seed, Mutation::RemoveBinding)?,
        "wrong_source_scope_detected": mutation_detected(seed, Mutation::WrongScope)?,
        "prevention_keyword_mutation_detected": mutation_detected(seed, Mutation::WrongKeyword)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "static_binding_removal_detected",
        "wrong_source_scope_detected",
        "prevention_keyword_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Light of Sanction mutation escaped: {key}"));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 24, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #24 costs {1}{W}{W}. Prevent all damage that would be dealt to creatures you control by sources you control; damage from an opponent's source remains damage and may be lethal.",
            "checked_sections": ["120 Damage", "614 Replacement and prevention effects", "604 Static abilities", "611 Continuous effects", "601 Casting spells", "510 Combat damage", "704 State-based actions"]
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveBinding,
    WrongScope,
    WrongKeyword,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    light: Option<ObjectId>,
    light_incarnation: Option<u64>,
    light_payment: Vec<Color>,
    watchwolf: Option<ObjectId>,
    pilot_char: Option<ObjectId>,
    pilot_char_payment: Vec<Color>,
    opponent_char: Option<ObjectId>,
    opponent_char_payment: Vec<Color>,
    skyknight: Option<ObjectId>,
    skyknight_payment: Vec<Color>,
    prevention_index: Option<usize>,
    target_alive_after_friendly_packet: bool,
    target_damage_after_friendly_packet: Option<i32>,
    enemy_damage_index: Option<usize>,
    target_in_graveyard_after_enemy_packet: bool,
    finisher_attack_index: Option<usize>,
    finisher_damage_index: Option<usize>,
    p0_char_self_damage_index: Option<usize>,
}

impl Captured {
    fn new() -> Self {
        Self {
            events: Vec::new(), winner: None, turns: 0, accepted: 0,
            light: None, light_incarnation: None, light_payment: Vec::new(),
            watchwolf: None, pilot_char: None, pilot_char_payment: Vec::new(),
            opponent_char: None, opponent_char_payment: Vec::new(),
            skyknight: None, skyknight_payment: Vec::new(),
            prevention_index: None, target_alive_after_friendly_packet: false,
            target_damage_after_friendly_packet: None, enemy_damage_index: None,
            target_in_graveyard_after_enemy_packet: false,
            finisher_attack_index: None, finisher_damage_index: None,
            p0_char_self_damage_index: None,
        }
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Light of Sanction candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Light of Sanction candidate source is invalid Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) {
        return Err("Light of Sanction module must contain only its two constructors and imports".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(import) if import.attrs.is_empty() => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || function.block.stmts.len() != 1
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.attrs.iter().all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"))
                {
                    return Err(format!("Light of Sanction {name} is not a direct crate-visible constructor"));
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(format!("Light of Sanction {name} must return a direct struct literal"));
                };
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "static_continuous_effect" => "StaticContinuousEffectBinding",
                    _ => return Err(format!("unexpected Light of Sanction constructor: {name}")),
                };
                if !literal.path.is_ident(expected) || literal.rest.is_some() {
                    return Err(format!("Light of Sanction {name} has an unsupported literal shape"));
                }
            }
            _ => return Err("Light of Sanction module contains executable top-level code".to_owned()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "static_continuous_effect".to_owned()]) {
        return Err("Light of Sanction constructor API differs from its task contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let definition = candidate_light_of_sanction::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Light of Sanction definition missing from the RAV registry")?;
    let effect = candidate_light_of_sanction::static_continuous_effect();
    let registered: Vec<_> = rav_static_continuous_effect_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if definition != registered_definition || registered != [effect.clone()] {
        return Err("Light of Sanction definition and static effect must be delegated to its candidate module".to_owned());
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules != ["full-rules-fidelity", "colored-cost-casting", "static-prevent-friendly-source-damage"]
        || effect.card_definition != CARD_ID
        || effect.change != ContinuousChange::ControlledCreaturesAddKeyword(Keyword::PreventDamageFromControlledSources)
    {
        return Err("Light of Sanction identity or controller-relative prevention binding differs from RAV #24".to_owned());
    }
    Ok(())
}

fn bindings_with_mutation(mutation: Mutation) -> Vec<StaticContinuousEffectBinding> {
    let mut bindings: Vec<_> = rav_static_continuous_effect_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .collect();
    let mut binding = candidate_light_of_sanction::static_continuous_effect();
    match mutation {
        Mutation::RemoveBinding => return bindings,
        Mutation::WrongScope => binding.card_definition = CHAR_ID,
        Mutation::WrongKeyword => binding.change = ContinuousChange::ControlledCreaturesAddKeyword(Keyword::Flying),
        Mutation::None => {}
    }
    bindings.push(binding);
    bindings
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions.iter_mut().find(|item| item.id == CARD_ID)
        .ok_or("Light of Sanction missing from registry")? = candidate_light_of_sanction::definition();
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        bindings_with_mutation(mutation),
        rav_land_entry_bindings(),
    ).map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings()).map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings()).map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(rav_static_creature_spell_cost_modifier_bindings()).map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings()).map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings()).map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings()).map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings()).map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings()).map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings()).map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings()).map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(rav_activated_ability_cost_modifier_bindings()).map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(rav_generalized_activated_ability_cost_bindings()).map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings()).map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings()).map_err(|error| error.to_string())?;
    game.validate_invariants().map_err(|error| error.to_string())?;
    Ok(game)
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry { card: CARD_ID.to_owned(), count: 4 },
            DeckEntry { card: WATCHWOLF_ID.to_owned(), count: 4 },
            DeckEntry { card: SKYKNIGHT_ID.to_owned(), count: 4 },
            DeckEntry { card: CHAR_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-PLAINS".to_owned(), count: 20 },
            DeckEntry { card: "RAV-FOREST".to_owned(), count: 12 },
            DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 12 },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry { card: CHAR_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 56 },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [("Light of Sanction pilot", &pilot), ("Char opponent", &opponent)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 { return Err(format!("{name} deck has {count} cards; expected 60")); }
    }
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(LightPolicy::new(PlayerId(0))),
        Box::new(CharOpponentPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured::new();
    while !game.is_game_over() {
        if game.turn > 80 || captured.accepted > 15_000 {
            let view = game.view_for_player(PlayerId(0)).ok();
            let hand: Vec<_> = view.as_ref().map(|view| view.hand.iter().map(|card| card.definition.unwrap_or("?")).collect()).unwrap_or_default();
            let board: Vec<_> = view.as_ref().map(|view| view.own_battlefield.iter().map(|card| (card.definition.unwrap_or("?"), card.tapped, card.mana_colors.clone())).collect()).unwrap_or_default();
            let cast_names: Vec<_> = game.event_log.iter().filter_map(|event| match event { GameEvent::SpellCast { card, .. } => game.card_definition(*card).ok().map(|definition| definition.id), _ => None }).collect();
            return Err(format!(
                "Light of Sanction duel exceeded its bounded game horizon at turn {} (accepted {}, events {}, Light {:?}, Watchwolf {:?}, friendly Char {:?}, enemy Char {:?}, Skyknight {:?}, winner {:?}, hand {:?}, board {:?}, casts {:?})",
                game.turn, captured.accepted, game.event_log.len(), captured.light,
                captured.watchwolf, captured.pilot_char, captured.opponent_char,
                captured.skyknight, game.winner().map(|player| player.0), hand, board, cast_names
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("Light of Sanction policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        let start = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("Light of Sanction move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        captured.accepted += 1;
        game.validate_invariants().map_err(|error| format!("Light of Sanction invariant failed: {error}"))?;

        for (offset, event) in game.event_log[start..].iter().enumerate() {
            let index = start + offset;
            match event {
                GameEvent::SpellCast { player: PlayerId(0), card } => match game.card_definition(*card).map(|definition| definition.id) {
                    Ok(CARD_ID) => { captured.light.get_or_insert(*card); }
                    Ok(WATCHWOLF_ID) => { captured.watchwolf.get_or_insert(*card); }
                    Ok(CHAR_ID) => { captured.pilot_char.get_or_insert(*card); }
                    Ok(SKYKNIGHT_ID) => { captured.skyknight.get_or_insert(*card); }
                    _ => {}
                },
                GameEvent::SpellCast { player: PlayerId(1), card } if game.card_definition(*card).is_ok_and(|definition| definition.id == CHAR_ID) => {
                    captured.opponent_char.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } => match game.card_definition(*card).map(|definition| definition.id) {
                    Ok(CARD_ID) => { captured.light.get_or_insert(*card); captured.light_payment = colors.clone(); }
                    Ok(CHAR_ID) => { captured.pilot_char.get_or_insert(*card); captured.pilot_char_payment = colors.clone(); }
                    Ok(SKYKNIGHT_ID) => { captured.skyknight.get_or_insert(*card); captured.skyknight_payment = colors.clone(); }
                    _ => {}
                },
                GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if game.card_definition(*card).is_ok_and(|definition| definition.id == CHAR_ID) => {
                    captured.opponent_char.get_or_insert(*card);
                    captured.opponent_char_payment = colors.clone();
                }
                GameEvent::CardMoved { card, to: Zone::Battlefield } if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) => {
                    captured.light.get_or_insert(*card);
                    if captured.light_incarnation.is_none() {
                        captured.light_incarnation = game.object(*card).ok().map(|object| object.incarnation);
                    }
                }
                GameEvent::DamagePrevented { source, target: Target::Permanent(target), amount: 4 }
                    if captured.pilot_char == Some(*source) && captured.watchwolf == Some(*target) =>
                {
                    captured.prevention_index = Some(index);
                    captured.target_alive_after_friendly_packet = game.zone_of(*target) == Some(Zone::Battlefield);
                    captured.target_damage_after_friendly_packet = game.object(*target).ok().map(|object| object.damage);
                }
                GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 }
                    if captured.opponent_char == Some(*source) && captured.watchwolf == Some(*permanent) =>
                {
                    captured.enemy_damage_index = Some(index);
                    captured.target_in_graveyard_after_enemy_packet = game.zone_of(*permanent) == Some(Zone::Graveyard);
                }
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount: 2 }
                    if captured.pilot_char == Some(*source) => captured.p0_char_self_damage_index = Some(index),
                GameEvent::AttackersDeclared { player: PlayerId(0), attackers }
                    if captured.finisher_attack_index.is_none() && captured.skyknight.is_some_and(|card| attackers.contains(&card)) => captured.finisher_attack_index = Some(index),
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 }
                    if captured.skyknight == Some(*source) && captured.finisher_damage_index.is_none() => captured.finisher_damage_index = Some(index),
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    Ok(captured)
}

struct LightPolicy { player: PlayerId, light_offered: bool, wolf_offered: bool, char_offered: bool, skyknight_offered: bool }
impl LightPolicy { fn new(player: PlayerId) -> Self { Self { player, light_offered: false, wolf_offered: false, char_offered: false, skyknight_offered: false } } }

impl LightPolicy {
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 { return None; }
        view.hand.iter().find(|card| card.definition == Some("RAV-PLAINS"))
            .or_else(|| view.hand.iter().find(|card| card.definition == Some("RAV-MOUNTAIN")))
            .or_else(|| view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)))
            .map(|land| PolicyAction::PlayLand { card: land.id })
    }
    fn cast(&mut self, view: &GameView, definition: &'static str, cost: ManaCost, target: Option<Target>) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(definition))?;
        if let Some(selection) = select_payment(view, &cost) {
            match definition {
                CARD_ID => self.light_offered = true,
                WATCHWOLF_ID => self.wolf_offered = true,
                CHAR_ID => self.char_offered = true,
                SKYKNIGHT_ID => self.skyknight_offered = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest { card: card.id, targets: target.into_iter().collect(), convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection: selection,
            });
        }
        let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
        let desired = missing.or_else(|| (u16::from(view.mana_pool.total()) < u16::from(cost.generic) + cost.colored.len() as u16).then(|| cost.colored.first().copied().unwrap_or(Color::White)));
        let land = view.own_battlefield.iter().find(|land| !land.tapped && land.card_types.contains(&CardType::Land) && desired.map_or(true, |color| land.mana_colors.contains(&color)))
            .or_else(|| view.own_battlefield.iter().find(|land| !land.tapped && land.card_types.contains(&CardType::Land) && !land.mana_colors.is_empty()))?;
        let color = desired.filter(|color| land.mana_colors.contains(color)).or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }
}

impl CodePolicy for LightPolicy {
    fn id(&self) -> &'static str { "rav.light-of-sanction-source-damage.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: view.own_battlefield.iter().filter(|card| card.card_types.contains(&CardType::Creature) && card.can_attack).map(|card| card.id).collect() };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared { return PolicyAction::DeclareBlockers { assignments: vec![] }; }
        if view.priority != self.player || view.active_player != self.player { return PolicyAction::PassPriority; }
        if !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        if let Some(land) = self.play_land(view) { return land; }
        if !self.light_offered && let Some(action) = self.cast(view, CARD_ID, ManaCost::with_colors(1, [Color::White, Color::White]), None) { return action; }
        if !self.wolf_offered && let Some(action) = self.cast(view, WATCHWOLF_ID, ManaCost::with_colors(0, [Color::Green, Color::White]), None) { return action; }
        let friendly_target = view.own_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID)).map(|card| Target::Permanent(card.id));
        if !self.char_offered && let Some(target) = friendly_target && let Some(action) = self.cast(view, CHAR_ID, ManaCost::with_colors(2, [Color::Red]), Some(target)) { return action; }
        if !self.skyknight_offered && let Some(action) = self.cast(view, SKYKNIGHT_ID, ManaCost::with_colors(1, [Color::Red, Color::White]), None) { return action; }
        PolicyAction::PassPriority
    }
}

struct CharOpponentPolicy { player: PlayerId, char_offered: bool }
impl CharOpponentPolicy { fn new(player: PlayerId) -> Self { Self { player, char_offered: false } } }
impl CodePolicy for CharOpponentPolicy {
    fn id(&self) -> &'static str { "rav.light-of-sanction-opponent-char.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared { return PolicyAction::DeclareAttackers { attackers: vec![] }; }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared { return PolicyAction::DeclareBlockers { assignments: vec![] }; }
        if view.priority != self.player || view.active_player != self.player { return PolicyAction::PassPriority; }
        if !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        if view.lands_played == 0 {
            if let Some(land) = view.hand.iter().find(|card| card.definition == Some("RAV-MOUNTAIN")) { return PolicyAction::PlayLand { card: land.id }; }
        }
        if !self.char_offered && view.turn >= 8 {
            let target = view.opponent_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID)).map(|card| card.id);
            if let Some(card) = view.hand.iter().find(|card| card.definition == Some(CHAR_ID))
                && let Some(target) = target
            {
                let cost = ManaCost::with_colors(2, [Color::Red]);
                if let Some(selection) = select_payment(view, &cost) {
                    self.char_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest { card: card.id, targets: vec![Target::Permanent(target)], convoke: vec![], payment_mana_abilities: vec![] },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|land| !land.tapped && land.card_types.contains(&CardType::Land) && land.mana_colors.contains(&Color::Red)) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn select_payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: BTreeMap<Color, u8> = Color::MANA_ALL.into_iter().map(|color| (color, view.mana_pool.amount(color))).collect();
    for color in &cost.colored { let amount = available.get_mut(color)?; if *amount == 0 { return None; } *amount -= 1; }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
    for _ in 0..cost.generic {
        let color = available.iter().filter(|(_, amount)| **amount > 0).max_by_key(|(_, amount)| **amount).map(|(color, _)| *color)?;
        *available.get_mut(&color)? -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection { generic, hybrid: vec![] })
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_LIGHT_OF_SANCTION_SEED") {
        let seed = raw.parse::<u64>().map_err(|_| "CARDBENCH_LIGHT_OF_SANCTION_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) && validate_trace(&captured, seed).is_ok() { return Ok((seed, captured)); }
    }
    Err(format!("no full Light of Sanction source-damage trace found for seeds 0..={MAX_SEED}"))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) { Err(_) => true, Ok(captured) => validate_trace(&captured, seed).is_err() })
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) { return Err("Light of Sanction duel did not end in a seat-zero win".to_owned()); }
    let light = captured.light.ok_or("pilot did not cast Light of Sanction")?;
    let incarnation = captured.light_incarnation.ok_or("Light of Sanction battlefield incarnation missing")?;
    let wolf = captured.watchwolf.ok_or("pilot did not cast Watchwolf")?;
    let pilot_char = captured.pilot_char.ok_or("pilot did not cast the friendly Char")?;
    let opponent_char = captured.opponent_char.ok_or("opponent did not cast the unfriendly Char")?;
    let skyknight = captured.skyknight.ok_or("pilot did not cast Skyknight Legionnaire")?;
    if captured.light_payment.len() != 3 || captured.light_payment.iter().filter(|color| **color == Color::White).count() < 2
        || captured.pilot_char_payment.len() != 3 || !captured.pilot_char_payment.contains(&Color::Red)
        || captured.opponent_char_payment.len() != 3 || !captured.opponent_char_payment.contains(&Color::Red)
        || captured.skyknight_payment.len() != 3 || !captured.skyknight_payment.contains(&Color::White) || !captured.skyknight_payment.contains(&Color::Red)
    { return Err(format!("Light, both Char spells, or the flying finisher used an incorrect colored payment: light {:?}, friendly Char {:?}, opponent Char {:?}, Skyknight {:?}", captured.light_payment, captured.pilot_char_payment, captured.opponent_char_payment, captured.skyknight_payment)); }
    if captured.prevention_index.is_none() || !captured.target_alive_after_friendly_packet || captured.target_damage_after_friendly_packet != Some(0) {
        return Err("friendly Char damage was not prevented while its Watchwolf target survived".to_owned());
    }
    if !captured.target_in_graveyard_after_enemy_packet { return Err("opponent Char did not deal lethal damage to the same Watchwolf".to_owned()); }

    let events = &captured.events;
    let light_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == light && colors.len() == 3)).ok_or("Light of Sanction payment missing")?;
    let light_cast = event_index_after(events, light_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == light)).ok_or("Light of Sanction cast missing")?;
    let light_resolved = event_index_after(events, light_cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == light)).ok_or("Light of Sanction did not resolve")?;
    let light_entered = event_index_after(events, light_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == light)).ok_or("Light of Sanction did not enter the battlefield")?;
    let wolf_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf)).ok_or("Watchwolf cast event missing")?;
    let wolf_entered = event_index_after(events, wolf_cast, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf)).ok_or("Watchwolf did not enter the battlefield")?;
    let pilot_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == pilot_char && colors.len() == 3)).ok_or("friendly Char payment missing")?;
    let pilot_cast = event_index_after(events, pilot_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == pilot_char)).ok_or("friendly Char cast missing")?;
    let prevent = event_index_after(events, pilot_cast, |event| matches!(event, GameEvent::DamagePrevented { source, target: Target::Permanent(permanent), amount: 4 } if *source == pilot_char && *permanent == wolf)).ok_or("Light did not prevent its controller's four damage to Watchwolf")?;
    if events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == pilot_char && *permanent == wolf)) {
        return Err("friendly Char damage was also committed after the prevention receipt".to_owned());
    }
    let self_damage = event_index_after(events, prevent, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount: 2 } if *source == pilot_char)).ok_or("Char's separate controller damage receipt missing")?;
    let pilot_resolved = event_index_after(events, self_damage, |event| matches!(event, GameEvent::SpellResolved { card } if *card == pilot_char)).ok_or("friendly Char did not resolve")?;
    let opponent_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == opponent_char && colors.len() == 3)).ok_or("opponent Char payment missing")?;
    let opponent_cast = event_index_after(events, opponent_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == opponent_char)).ok_or("opponent Char cast missing")?;
    let enemy_damage = event_index_after(events, opponent_cast, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 } if *source == opponent_char && *permanent == wolf)).ok_or("opponent Char damage to the same Watchwolf was not committed")?;
    let opponent_resolved = event_index_after(events, enemy_damage, |event| matches!(event, GameEvent::SpellResolved { card } if *card == opponent_char)).ok_or("opponent Char did not resolve")?;
    let wolf_died = event_index_after(events, enemy_damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == wolf)).ok_or("lethally damaged Watchwolf did not move to graveyard")?;
    let sky_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == skyknight && colors.len() == 3)).ok_or("Skyknight colored payment missing")?;
    let sky_cast = event_index_after(events, sky_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == skyknight)).ok_or("Skyknight cast missing")?;
    let sky_resolved = event_index_after(events, sky_cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == skyknight)).ok_or("Skyknight did not resolve")?;
    let sky_entered = event_index_after(events, sky_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == skyknight)).ok_or("Skyknight did not enter the battlefield")?;
    let sky_attack = captured.finisher_attack_index.ok_or("Skyknight did not attack")?;
    let sky_damage = captured.finisher_damage_index.ok_or("Skyknight did not damage the opposing player")?;
    let terminal = event_index_after(events, sky_damage, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal PvP win missing after Skyknight combat")?;
    let light_live_at_opponent_damage = light_entered < enemy_damage && game_event_object_on_battlefield(events, light_entered, enemy_damage, light);
    if !light_live_at_opponent_damage { return Err("Light left the battlefield before the opposing damage packet".to_owned()); }
    if !(light_paid < light_cast && light_cast < light_resolved && light_resolved < light_entered
        && light_entered < wolf_entered && wolf_entered < pilot_paid && pilot_paid < pilot_cast && pilot_cast < pilot_resolved
        && pilot_cast < prevent && prevent < self_damage && self_damage < pilot_resolved && pilot_resolved < opponent_paid && opponent_paid < opponent_cast
        && opponent_cast < enemy_damage && enemy_damage < opponent_resolved && enemy_damage < wolf_died && sky_entered < sky_attack && sky_attack < sky_damage && sky_damage < terminal)
    { return Err("Light, prevention, opposing damage, and terminal event order is invalid".to_owned()); }

    let selected = BTreeSet::from([light_paid, light_cast, light_resolved, light_entered,
        wolf_cast, wolf_entered, pilot_paid, pilot_cast, pilot_resolved, prevent, self_damage,
        opponent_paid, opponent_cast, opponent_resolved, enemy_damage, wolf_died,
        sky_paid, sky_cast, sky_resolved, sky_entered, sky_attack, sky_damage, terminal]);
    let projected: Vec<ProjectedEvent> = project_events(events).into_iter().enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event)).collect();
    let event_kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    if event_kinds.len() != 23 { return Err(format!("Light of Sanction event projection length differs: {event_kinds:?}")); }

    Ok(json!({
        "id": "rav-light-of-sanction-friendly-source-prevention",
        "seed": seed,
        "decks": ["rav_light_of_sanction_three_color_pilot", "rav_char_opponent"],
        "policies": ["rav.light-of-sanction-source-damage.v1", "rav.light-of-sanction-opponent-char.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": 0,
        "mechanics": {
            "light_of_sanction": {"object": light.0, "source_incarnation": incarnation, "mana_cost": "{1}{W}{W}", "payment": captured.light_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
            "friendly_char": {"object": pilot_char.0, "mana_cost": "{2}{R}", "payment": captured.pilot_char_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "target": wolf.0, "target_damage": 4, "prevented": true, "target_survived_with_damage": captured.target_damage_after_friendly_packet, "controller_self_damage": 2},
            "opponent_char": {"object": opponent_char.0, "mana_cost": "{2}{R}", "payment": captured.opponent_char_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "target": wolf.0, "target_damage": 4, "damage_committed": true, "target_moved_to_graveyard": captured.target_in_graveyard_after_enemy_packet, "light_still_on_battlefield": light_live_at_opponent_damage},
            "watchwolf": {"object": wolf.0, "power": 3, "toughness": 3, "survived_friendly_char": captured.target_alive_after_friendly_packet, "died_to_opponent_char": captured.target_in_graveyard_after_enemy_packet},
            "finisher": {"object": skyknight.0, "mana_cost": "{1}{R}{W}", "payment": captured.skyknight_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "flying": true, "combat_damage": 2},
            "controller_relative_prevention_consistent": true
        },
        "prevention_sequence": prevent,
        "opponent_damage_sequence": enemy_damage,
        "terminal_sequence": terminal,
        "events": projected,
        "passed": true
    }))
}

fn game_event_object_on_battlefield(events: &[GameEvent], from: usize, to: usize, card: ObjectId) -> bool {
    events.iter().take(to).skip(from).all(|event| !matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Graveyard | Zone::Exile | Zone::Hand | Zone::Library } if *moved == card))
}
fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> { events.iter().position(predicate) }
fn event_index_after(events: &[GameEvent], after: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> { events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index)) }

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), CardSemanticStatus::CatalogOnly { .. } => None };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed {"passed"} else {"not_qualified"},"qualification":if passed {"unreviewed_diagnostic"} else {"not_evaluated"},"credited":false})
}
fn candidate_kind() -> &'static str { if std::env::args().any(|arg| arg == "--candidate") { "agent" } else { "reference" } }
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args_os();
    while let Some(argument) = args.next() { if argument == "--output-root" { return args.next().map(Into::into); } }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), format!("{}\n", serde_json::to_string_pretty(report).expect("report serializes")))?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n")?;
    Ok(())
}
