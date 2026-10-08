//! Full-game diagnostic authority for RAV Excruciator.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, CastRequest, Color, CombatBlock,
    DeckEntry, DeckList, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-EXCRUCIATOR";
const COMMANDO_ID: &str = "RAV-ORDRUUN-COMMANDO";
const PLAINS_ID: &str = "RAV-PLAINS";
const MOUNTAIN_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/excruciator.rs";
const MAX_TRACE_SEED: u64 = 64;
const MAX_TURNS: u32 = 50;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/excruciator.rs"]
mod candidate_excruciator;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() && let Err(error) = write_outputs(&root, &report) {
                eprintln!("could not write Excruciator result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(error) => { eprintln!("Excruciator PvP authority failed: {error}"); ExitCode::FAILURE }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation { None, RemovePreventionKeyword, PrintedCostChanged, PowerChanged }

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    spell: Option<ObjectId>,
    payment: Vec<Color>,
    resolved: bool,
    entered: bool,
    power: Option<i32>,
    toughness: Option<i32>,
    damage_cannot_be_prevented: bool,
    commando: Option<ObjectId>,
    commando_resolved: bool,
    commando_entered: bool,
    shield_source: Option<ObjectId>,
    shield_amount: Option<i32>,
    commando_ability_resolved: bool,
    blocked: bool,
    excruciator_damage_to_commando: Vec<i32>,
    prevented_from_excruciator: Vec<i32>,
    commando_graveyard: bool,
    attack_turns: Vec<u32>,
    damage_to_opponent: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Excruciator full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "damage_prevention_keyword_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::RemovePreventionKeyword)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::PrintedCostChanged)?,
        "power_changed_mutation_detected_in_combat": mutation_detected(seed, Mutation::PowerChanged)?,
        "sealed_mutation_authority": false
    });
    for key in ["damage_prevention_keyword_removed_mutation_detected_in_game", "printed_cost_changed_mutation_detected", "power_changed_mutation_detected_in_combat"] {
        if mutations[key] != true { return Err(format!("Excruciator gameplay mutant escaped: {key}")); }
    }
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings.iter().copied().filter(|card| names.insert(card.name)).collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT || catalog.len() != 291 || printings.len() != 306 {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": TASK_ID, "expansion": "RAV",
        "card": {"collector_number": 121, "printings": 1, "definition_id": CARD_ID, "name": "Excruciator"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID}, "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["702.65: damage can't be prevented", "510.1: combat damage", "704.5: state-based actions", "601.2: cast creature spells", "508.1 and 509.1: combat declarations"],
            "source_contract": "RAV Excruciator #121 is a red 7/7 creature costing {6}{R}{R} with damage that can't be prevented.",
            "authority_cutoff": "2026-09-24", "no_provider_trial": true
        },
        "passed": true, "pilot_card_passed": true, "deterministic_replay": true,
        "cards_passed": 1, "cards_credited": 0, "cards_total": catalog.len(), "printings_total": printings.len(),
        "coverage_complete": false, "full_reward": false, "harbor_reward": 0.0, "pilot_reward": 0.0,
        "qualification_status": "diagnostic_pilot_trace_passed; independent review pending",
        "mutation_sensitivity": mutations, "traces": [trace], "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_excruciator::definition();
    match mutation {
        Mutation::None => {},
        Mutation::RemovePreventionKeyword => definition.keywords.clear(),
        Mutation::PrintedCostChanged => definition.mana_cost = ManaCost::with_colors(5, [Color::Red, Color::Red]),
        Mutation::PowerChanged => definition.power = Some(6),
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_excruciator::definition();
    let registered = card_definitions().into_iter().find(|card| card.id == CARD_ID).ok_or("Excruciator is missing from the RAV registry")?;
    if registered != definition || definition != expected_definition() {
        return Err("Excruciator candidate definition differs from the exact RAV #121 source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| format!("could not read Excruciator source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Excruciator source is not valid Rust".to_owned())?;
    if file.items.len() != 3 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Excruciator module must contain only private imports and one literal function".to_owned());
    }
    let mut definition_seen = false;
    for item in &file.items {
        match item {
            Item::Use(item) if matches!(&item.vis, Visibility::Inherited) && safe_use_tree(&item.tree) => {},
            Item::Fn(function) if function.sig.ident == "definition" && !definition_seen => {
                definition_seen = true;
                if !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty() || function.sig.asyncness.is_some() || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty() || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr))) {
                    return Err("Excruciator definition must be a direct literal".to_owned());
                }
            }
            _ => return Err("Excruciator source contains executable or unsupported top-level code".to_owned()),
        }
    }
    if !definition_seen { return Err("Excruciator definition() is missing".to_owned()); }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => matches!(path.ident.to_string().as_str(), "std" | "collections" | "cardbench_magic_engine") && safe_use_tree(&path.tree),
        UseTree::Name(name) => matches!(name.ident.to_string().as_str(), "BTreeSet" | "CardDefinition" | "CardType" | "Color" | "Keyword" | "ManaCost"),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(value.lit, syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)),
        Expr::Path(path) => {
            let segments = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>();
            matches!(segments.as_slice(), [a, b] if (a == "Color" && b == "Red") || (a == "CardType" && b == "Creature") || (a == "Keyword" && b == "DamageCannotBePrevented")) || segments.as_slice() == ["None"]
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return false; };
            let segments = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>();
            (matches!(segments.as_slice(), [a, b] if (a == "ManaCost" && (b == "with_colors" || b == "new")) || (a == "BTreeSet" && (b == "from" || b == "new"))) || segments.as_slice() == ["Some"])
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => literal.path.is_ident("CardDefinition") && literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)),
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            use syn::parse::Parser;
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated.parse2(mac.mac.tokens.clone()).is_ok_and(|items| items.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID, name: "Excruciator", set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]), mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]), is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "colored-cost-casting", "base-characteristics", "damage-cannot-be-prevented"],
        power: Some(7), toughness: Some(7), keywords: vec![Keyword::DamageCannotBePrevented], effects: vec![],
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions.iter_mut().find(|definition| definition.id == CARD_ID).ok_or("Excruciator is missing from the RAV registry")? = candidate_definition(mutation);
    Game::new_with_all_bindings(definitions, 2, rav_mana_ability_bindings(), rav_basic_land_type_bindings(), rav_additional_spell_cost_bindings(), rav_activated_ability_bindings()).map_err(|error| error.to_string())
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList { mainboard: entries.iter().map(|(card, count)| DeckEntry { card: (*card).to_owned(), count: *count }).collect(), sideboard: vec![] }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_EXCRUCIATOR_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_EXCRUCIATOR_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None).and_then(|capture| validate_trace(&capture, seed).map(|_| capture)) {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!("no Excruciator combat-prevention trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (MOUNTAIN_ID, 32), (PLAINS_ID, 24)]);
    let opponent = deck(&[(COMMANDO_ID, 4), (MOUNTAIN_ID, 28), (PLAINS_ID, 28)]);
    if [&pilot, &opponent].iter().any(|list| list.mainboard.iter().map(|entry| usize::from(entry.count)).sum::<usize>() != 60) { return Err("Excruciator decks must each contain legal 60-card mainboards".to_owned()); }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig { shuffle_seed: seed, max_turns: MAX_TURNS, max_policy_moves: MAX_MOVES, ..DeckMatchConfig::default() };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] { game.load_deck_into_library(player, list).map_err(|error| error.to_string())?; game.draw_opening_hand(player, config.opening_hand_size).map_err(|error| error.to_string())?; }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(ExcruciatorPolicy::new(PlayerId(0), mutation)), Box::new(CommandoPolicy::new(PlayerId(1)))];
    let mut capture = Captured { events: vec![], winner: None, turns: 0, accepted: 0, spell: None, payment: vec![], resolved: false, entered: false, power: None, toughness: None, damage_cannot_be_prevented: false, commando: None, commando_resolved: false, commando_entered: false, shield_source: None, shield_amount: None, commando_ability_resolved: false, blocked: false, excruciator_damage_to_commando: vec![], prevented_from_excruciator: vec![], commando_graveyard: false, attack_turns: vec![], damage_to_opponent: 0, life: [20, 20] };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves { return Err(format!("Excruciator duel exceeded run bound on turn {}", game.turn)); }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("Excruciator policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) { action }
            else if view.optional_triggered_ability_choice.is_some() { policy.propose_optional_triggered_ability(&view) }
            else if view.draw_replacement_pending { policy.propose_draw_replacement(&view) }
            else if view.private_library_choice.is_some() { policy.propose_private_library_choice(&view) }
            else if view.private_opponent_library_choice.is_some() { policy.propose_private_opponent_library_choice(&view) }
            else if view.library_search_choice.is_some() { policy.propose_library_search_choice(&view) }
            else { policy.propose_move(&view) };
        if let PolicyAction::CastWithPayment { request, .. } = &action && game.card_definition(request.card).is_ok_and(|definition| definition.id == CARD_ID) { capture.spell = Some(request.card); }
        if let PolicyAction::CastWithPayment { request, .. } = &action && game.card_definition(request.card).is_ok_and(|definition| definition.id == COMMANDO_ID) { capture.commando = Some(request.card); }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("Excruciator policy move rejected at turn {} {:?}: {error}", view.turn, view.step))?;
        capture.accepted += 1;
        game.validate_invariants().map_err(|error| format!("post-move Excruciator invariant failure: {error}"))?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone(); capture.winner = game.winner().map(|player| player.0); capture.turns = game.turn;
    capture.life = [game.player(PlayerId(0)).map_err(|error| error.to_string())?.life, game.player(PlayerId(1)).map_err(|error| error.to_string())?.life];
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if Some(*card) == capture.spell => capture.payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == capture.spell => capture.resolved = true,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if Some(*card) == capture.spell => { capture.entered = true; let ch = game.characteristics(*card).map_err(|error| error.to_string())?; capture.power = ch.power; capture.toughness = ch.toughness; capture.damage_cannot_be_prevented = ch.keywords.contains(&Keyword::DamageCannotBePrevented); }
            GameEvent::SpellResolved { card } if Some(*card) == capture.commando => capture.commando_resolved = true,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if Some(*card) == capture.commando => capture.commando_entered = true,
            GameEvent::AbilityActivated { player: PlayerId(1), source, definition, ability, .. } if Some(*source) == capture.commando && *definition == COMMANDO_ID && *ability == "prevent-one-damage-to-self" => capture.shield_source = Some(*source),
            GameEvent::AbilityResolved { source, ability, .. } if Some(*source) == capture.commando && *ability == "prevent-one-damage-to-self" => {
                capture.commando_ability_resolved = true;
                capture.shield_amount = game.object(*source).ok().map(|object| i32::from(object.damage_shield));
            }
            GameEvent::BlockersDeclared { assignments, .. } if assignments.iter().any(|(attacker, blocker)| Some(*attacker) == capture.spell && Some(*blocker) == capture.commando) => capture.blocked = true,
            GameEvent::DamageDealtToPermanent { source, permanent, amount } if Some(*source) == capture.spell && Some(*permanent) == capture.commando => capture.excruciator_damage_to_commando.push(*amount),
            GameEvent::DamagePrevented { source, target: Target::Permanent(target), amount } if Some(*source) == capture.spell && Some(*target) == capture.commando => capture.prevented_from_excruciator.push(*amount),
            GameEvent::CardMoved { card, to: Zone::Graveyard } if Some(*card) == capture.commando => capture.commando_graveyard = true,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if capture.spell.is_some_and(|spell| attackers.contains(&spell)) => capture.attack_turns.push(game.turn),
            GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } => capture.damage_to_opponent += *amount,
            _ => {}
        }
    }
    Ok(())
}

struct ExcruciatorPolicy { player: PlayerId, mutation: Mutation }
struct CommandoPolicy { player: PlayerId, shield_issued: bool, commando_cast: bool }

impl ExcruciatorPolicy { fn new(player: PlayerId, mutation: Mutation) -> Self { Self { player, mutation } } }
impl CommandoPolicy { fn new(player: PlayerId) -> Self { Self { player, shield_issued: false, commando_cast: false } } }

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL.into_iter().map(|color| (color, view.mana_pool.amount(color))).collect();
    for color in &cost.colored { let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?; if *amount == 0 { return None; } *amount -= 1; }
    let mut generic = Vec::new();
    for _ in 0..cost.generic { let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?; if *amount == 0 { return None; } *amount -= 1; generic.push(*color); }
    Some(ManaPaymentSelection { generic, hybrid: vec![] })
}

fn mana_action(view: &GameView, color: Option<Color>) -> Option<PolicyAction> {
    let land = view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && color.is_some_and(|color| card.mana_colors.contains(&color)))
        .or_else(|| view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && !card.mana_colors.is_empty()))?;
    let color = color.filter(|color| land.mana_colors.contains(color)).or_else(|| land.mana_colors.iter().next().copied())?;
    Some(PolicyAction::ActivateManaAbility { land: land.id, color })
}

fn land_action(view: &GameView, preferred: &[&str]) -> Option<PolicyAction> {
    if view.lands_played != 0 { return None; }
    let land = preferred.iter().find_map(|id| view.hand.iter().find(|card| card.definition == Some(id)))
        .or_else(|| view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)))?;
    Some(PolicyAction::PlayLand { card: land.id })
}

impl CodePolicy for ExcruciatorPolicy {
    fn id(&self) -> &'static str { "rav.excruciator-cast-attack.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            let attackers = if view.own_battlefield.iter().any(|card| card.definition == Some(CARD_ID)) { view.own_battlefield.iter().filter(|card| card.definition == Some(CARD_ID) && card.can_attack).map(|card| card.id).collect() } else { vec![] };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared { return PolicyAction::DeclareBlockers { assignments: vec![] }; }
        if view.priority != self.player || view.active_player != self.player || !view.step.is_main() || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        if let Some(action) = land_action(view, &[MOUNTAIN_ID, PLAINS_ID]) { return action; }
        if !view.own_battlefield.iter().any(|card| card.definition == Some(CARD_ID)) && view.hand.iter().any(|card| card.definition == Some(CARD_ID)) {
            let cost = candidate_definition(self.mutation).mana_cost;
            if let Some(selection) = payment(view, &cost) {
                let card = view.hand.iter().find(|card| card.definition == Some(CARD_ID)).expect("found in hand");
                return PolicyAction::CastWithPayment { request: CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }, chosen_x: None, mana_selection: selection };
            }
            let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
            if let Some(action) = mana_action(view, missing) { return action; }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for CommandoPolicy {
    fn id(&self) -> &'static str { "rav.ordruun-commando-prevention-blocker.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared { return PolicyAction::DeclareAttackers { attackers: vec![] }; }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared {
            let attacker = view.combat_attackers.iter().find(|card| card.definition == Some(CARD_ID));
            let blocker = view.own_battlefield.iter().find(|card| card.definition == Some(COMMANDO_ID) && card.can_block);
            let assignments = match (attacker, blocker) { (Some(attacker), Some(blocker)) => vec![CombatBlock { attacker: attacker.id, blocker: blocker.id }], _ => vec![] };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        let excruciator_live = view.opponent_battlefield.iter().any(|card| card.definition == Some(CARD_ID));
        let commando = view.own_battlefield.iter().find(|card| card.definition == Some(COMMANDO_ID));
        if excruciator_live && !self.shield_issued && let Some(source) = commando {
            if view.mana_pool.amount(Color::White) == 0 { if let Some(action) = mana_action(view, Some(Color::White)) { return action; } }
            if view.mana_pool.amount(Color::White) > 0 {
                self.shield_issued = true;
                return PolicyAction::ActivateAbility { activation: AbilityActivation { source: source.id, ability_id: "prevent-one-damage-to-self", sacrifice_sources: vec![], additional_tap_creatures: vec![], discard_cards: vec![], targets: vec![] } };
            }
        }
        if view.active_player == self.player && view.step.is_main() {
            if let Some(action) = land_action(view, &[PLAINS_ID, MOUNTAIN_ID]) { return action; }
            if !self.commando_cast && !view.own_battlefield.iter().any(|card| card.definition == Some(COMMANDO_ID)) && view.hand.iter().any(|card| card.definition == Some(COMMANDO_ID)) {
                let cost = ManaCost::with_colors(3, [Color::Red]);
                if let Some(selection) = payment(view, &cost) {
                    let card = view.hand.iter().find(|card| card.definition == Some(COMMANDO_ID)).expect("found Commando in hand");
                    self.commando_cast = true;
                    return PolicyAction::CastWithPayment { request: CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }, chosen_x: None, mana_selection: selection };
                }
                let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
                if let Some(action) = mana_action(view, missing) { return action; }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) { return Err("Excruciator full game did not end in a pilot win".to_owned()); }
    let source = capture.spell.ok_or("Excruciator was not cast")?;
    let blocker = capture.commando.ok_or("Ordruun Commando was not cast by the opponent")?;
    if !capture.resolved || !capture.entered || capture.payment.len() != 8 || capture.payment.iter().filter(|color| **color == Color::Red).count() < 2
        || capture.power != Some(7) || capture.toughness != Some(7) || !capture.damage_cannot_be_prevented {
        return Err(format!("Excruciator did not cast for {{6}}{{R}}{{R}} as a live 7/7 with its prevention exception: payment={:?}, P/T={:?}/{:?}, keyword={}", capture.payment, capture.power, capture.toughness, capture.damage_cannot_be_prevented));
    }
    if !capture.commando_resolved || !capture.commando_entered || capture.shield_source != Some(blocker) || capture.shield_amount != Some(1) || !capture.commando_ability_resolved || !capture.blocked
        || capture.excruciator_damage_to_commando != [7] || !capture.prevented_from_excruciator.is_empty() || !capture.commando_graveyard {
        return Err(format!("Ordruun's one-point shield did not fail to prevent Excruciator's full 7 combat damage: shield={:?}, source={:?}, blocker={:?}, ability_resolved={}, blocked={}, damage={:?}, prevented={:?}, graveyard={}", capture.shield_amount, capture.shield_source, capture.commando, capture.commando_ability_resolved, capture.blocked, capture.excruciator_damage_to_commando, capture.prevented_from_excruciator, capture.commando_graveyard));
    }
    if capture.attack_turns.is_empty() || capture.damage_to_opponent < 20 || capture.life[1] > 0 { return Err("Excruciator pilot did not finish the game through combat".to_owned()); }
    let spell_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 8 && colors.iter().filter(|color| **color == Color::Red).count() >= 2)).ok_or("Excruciator exact spell payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Excruciator SpellCast missing")?;
    let resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == source)).ok_or("Excruciator SpellResolved missing")?;
    let entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Excruciator battlefield entry missing")?;
    let commando_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)).ok_or("Ordruun Commando SpellCast missing")?;
    let commando_resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == blocker)).ok_or("Ordruun Commando resolution missing")?;
    let commando_entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blocker)).ok_or("Ordruun Commando entry missing")?;
    let activation = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(1), source: known, definition: COMMANDO_ID, ability: "prevent-one-damage-to-self", .. } if *known == blocker)).ok_or("Ordruun Commando activation receipt missing")?;
    let ability_resolved = event_index_after(&capture.events, activation, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability: "prevent-one-damage-to-self", .. } if *known == blocker)).ok_or("Ordruun Commando shield ability did not resolve")?;
    let attack = event_index_after(&capture.events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).ok_or("Excruciator did not attack after the prevention shield was created")?;
    let blocks = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { assignments, .. } if assignments.contains(&(source, blocker)))).ok_or("Ordruun Commando did not block Excruciator")?;
    let damage = event_index_after(&capture.events, blocks, |event| matches!(event, GameEvent::DamageDealtToPermanent { source: known, permanent, amount: 7 } if *known == source && *permanent == blocker)).ok_or("Excruciator's full seven combat damage was not committed")?;
    let graveyard = event_index_after(&capture.events, damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == blocker)).ok_or("Ordruun Commando was not moved to its graveyard")?;
    if capture.events.iter().any(|event| matches!(event, GameEvent::DamagePrevented { source: known, target: Target::Permanent(target), .. } if *known == source && *target == blocker)) { return Err("damage was prevented from Excruciator despite its keyword".to_owned()); }
    let damage_to_player = event_index_after(&capture.events, graveyard, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: known, player: PlayerId(1), amount } if *known == source && *amount > 0)).ok_or("Excruciator dealt no later combat damage to the opponent")?;
    let ended = unique_index(&capture.events, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal pilot GameEnded receipt missing")?;
    if !(spell_paid < cast && cast < resolved && resolved < entered && commando_cast < commando_resolved && commando_resolved < commando_entered && entered < activation && activation < ability_resolved && ability_resolved < attack && attack < blocks && blocks < damage && damage < graveyard && graveyard < damage_to_player && damage_to_player < ended) {
        return Err(format!("Excruciator cast, shield, combat, and terminal event ordering differs: ({spell_paid},{cast},{resolved},{entered}), Commando=({commando_cast},{commando_resolved},{commando_entered},{activation},{ability_resolved}), combat=({attack},{blocks},{damage},{graveyard},{damage_to_player},{ended})"));
    }
    let include: BTreeSet<_> = [spell_paid, cast, resolved, entered, commando_cast, commando_resolved, commando_entered, activation, ability_resolved, attack, blocks, damage, graveyard, damage_to_player, ended].into_iter().collect();
    let projected = project_events(&capture.events);
    let events: Vec<_> = include.iter().map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string())).collect::<Result<_, _>>()?;
    Ok(json!({
        "id": "rav-excruciator-seven-combat-damage-ignores-shield-terminal",
        "seed": seed, "terminal": "Winner(PlayerId(0))", "winner_seat": 0, "turns": capture.turns, "accepted_moves": capture.accepted, "rejected_moves": 0,
        "excruciator": {"object": source.0, "collector_number": 121, "mana_cost": "{6}{R}{R}", "mana_paid": capture.payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": capture.power, "toughness": capture.toughness, "damage_cannot_be_prevented": capture.damage_cannot_be_prevented},
        "prevention_interaction": {"opponent_card": COMMANDO_ID, "opponent_object": blocker.0, "shield_amount_before_combat": capture.shield_amount, "blocked": capture.blocked, "damage_dealt_to_blocker": capture.excruciator_damage_to_commando, "damage_prevented_from_excruciator": capture.prevented_from_excruciator, "blocker_moved_to_graveyard": capture.commando_graveyard},
        "combat": {"attack_turns": capture.attack_turns, "damage_to_opponent": capture.damage_to_opponent, "life_at_terminal": capture.life},
        "ordered_event_sequence": include.iter().copied().collect::<Vec<_>>(), "events": events, "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) { Err(_) => Ok(true), Ok(capture) => Ok(validate_trace(&capture, seed).is_err()) }
}
fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> { let mut found = events.iter().enumerate().filter_map(|(i, e)| predicate(e).then_some(i)); let first = found.next()?; found.next().is_none().then_some(first) }
fn event_index_after(events: &[GameEvent], after: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> { events.iter().enumerate().skip(after + 1).find_map(|(i, e)| predicate(e).then_some(i)) }
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), CardSemanticStatus::CatalogOnly { .. } => None };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
}
fn candidate_kind() -> &'static str { if std::env::args().any(|argument| argument == "--candidate") { "agent" } else { "reference" } }
fn output_root() -> Option<std::path::PathBuf> { let mut args = std::env::args().skip(1); while let Some(arg) = args.next() { if arg == "--output-root" { return args.next().map(std::path::PathBuf::from); } } None }
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> { std::fs::create_dir_all(root)?; std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?; std::fs::write(root.join("reward.txt"), "0.0\n") }
