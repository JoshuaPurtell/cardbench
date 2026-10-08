//! Full-game RAV Seismic Spike authority: destroy a land, add two red mana, and win.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, new_rav_game,
    rav_main_set_catalog,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, Item, Lit, Meta, Stmt, Token, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SEISMIC-SPIKE";
const TARGET_LAND_ID: &str = "RAV-MOUNTAIN";
const SLASHER_ID: &str = "RAV-VIASHINO-SLASHER";
const LAND_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs"]
mod candidate_seismic_spike;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Seismic Spike PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(error) => {
            eprintln!("Seismic Spike PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_identity()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings.iter().copied().filter(|card| names.insert(card.name)).collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT {
        return Err(format!("RAV catalog has {} names / {} printings; expected {} / {}", catalog.len(), printings.len(), RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT));
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed)?;
    if captured.events != replay.events || captured.terminal != replay.terminal || captured.winner != replay.winner || captured.spike != replay.spike || captured.land != replay.land || captured.land_is_mountain != replay.land_is_mountain || captured.mana_added != replay.mana_added || captured.followup_slasher != replay.followup_slasher || captured.combat_source != replay.combat_source {
        return Err("Seismic Spike full-game replay was not deterministic".to_owned());
    }

    let mut wrong_definition = candidate_seismic_spike::definition();
    wrong_definition.effects.swap(0, 1);
    let mutation_detected = !definition_identity_valid(&wrong_definition);
    if !mutation_detected { return Err("Seismic Spike effect-order mutation control was ineffective".to_owned()); }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown.iter().filter(|card| card["trace_status"] == "passed").count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 141, "printings": 1, "definition_id": CARD_ID, "name": "Seismic Spike"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting a sorcery and paying its costs",
                "608.2b: target legality on resolution",
                "608.3: resolving spell instructions in printed order",
                "701.7: destroying a permanent",
                "106.4: mana added during spell resolution",
            ],
            "source_contract": "RAV Seismic Spike destroys a target land, then adds two red mana",
            "authority_cutoff": "2026-09-23"
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" },
        "mutation_sensitivity": {
            "effect_order_mutation_detected": !definition_identity_valid(&wrong_definition),
            "land_destroyed_by_live_spike": trace["passed"].as_bool() == Some(true),
            "two_red_mana_added_by_live_spike": captured.mana_added,
            "followup_spell_paid_with_spike_mana": captured.followup_slasher.is_some(),
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| format!("could not read Seismic Spike source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Seismic Spike source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Seismic Spike source has an unsupported module shape".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ].into_iter().map(|path| path.into_iter().map(str::to_owned).collect()).collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {
                if !collect_use_paths(&mut Vec::new(), &item.tree, &mut actual_imports) { return Err("Seismic Spike source has an unsupported import".to_owned()); }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !definition_function_is_literal(function) {
                    return Err(format!("Seismic Spike function `{name}` must return a crate-private literal definition"));
                }
            }
            _ => return Err("Seismic Spike source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Seismic Spike source is missing its declarative definition".to_owned());
    }
    Ok(())
}

fn collect_use_paths(prefix: &mut Vec<String>, tree: &UseTree, output: &mut BTreeSet<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => { prefix.push(path.ident.to_string()); let valid = collect_use_paths(prefix, &path.tree, output); prefix.pop(); valid }
        UseTree::Name(name) => { prefix.push(name.ident.to_string()); output.insert(prefix.clone()); prefix.pop(); true }
        UseTree::Group(group) => group.items.iter().all(|item| collect_use_paths(prefix, item, output)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn definition_function_is_literal(function: &syn::ItemFn) -> bool {
    let attrs_ok = function.attrs.iter().all(|attribute| is_doc_attribute(attribute) || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_)));
    let crate_private = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    if !attrs_ok || !crate_private || function.sig.ident != "definition" || function.sig.asyncness.is_some() || function.sig.unsafety.is_some() || !function.sig.inputs.is_empty() || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_name(ty).as_deref() == Some("CardDefinition")) || function.block.stmts.len() != 1 { return false; }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else { return false; };
    literal.path.segments.last().is_some_and(|segment| segment.ident == "CardDefinition")
        && literal.rest.is_none()
        && literal.fields.iter().all(|field| matches!(&field.member, syn::Member::Named(_)) && field.colon_token.is_some() && field.attrs.is_empty() && safe_expression(&field.expr))
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => literal.attrs.is_empty() && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(path) => {
            let segments: Vec<String> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            match segments.as_slice() {
                [single] => single == "None",
                [kind, variant] if kind == "Color" => variant == "Red",
                [kind, variant] if kind == "CardType" => variant == "Sorcery",
                [kind, variant] if kind == "Effect" => variant == "DestroyTargetLand",
                _ => false,
            }
        }
        Expr::Struct(structure) => {
            let path: Vec<String> = structure.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            let field_names: BTreeSet<String> = structure.fields.iter().filter_map(|field| match &field.member {
                syn::Member::Named(name) => Some(name.to_string()),
                syn::Member::Unnamed(_) => None,
            }).collect();
            path == ["Effect", "AddManaController"]
                && structure.rest.is_none()
                && structure.fields.len() == 2
                && field_names == BTreeSet::from(["amount".to_owned(), "color".to_owned()])
                && structure.fields.iter().all(|field| {
                    let syn::Member::Named(name) = &field.member else { return false; };
                    match name.to_string().as_str() {
                        "color" => safe_expression(&field.expr) && matches!(&field.expr, Expr::Path(path) if path.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>() == ["Color", "Red"]),
                        "amount" => matches!(&field.expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(value) if value.base10_parse::<u8>().is_ok_and(|amount| amount == 2))),
                        _ => false,
                    }
                })
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return false; };
            let segments: Vec<String> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            matches!(segments.as_slice(), [ty, method] if (ty == "ManaCost" && method == "with_colors") || (ty == "BTreeSet" && (method == "from" || method == "new"))) && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
            parser.parse2(mac.mac.tokens.clone()).is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(literal.lit, Lit::Str(_))))
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None; };
    path.path.segments.last().map(|segment| segment.ident.to_string())
}

fn validate_candidate_identity() -> Result<(), String> {
    let candidate = candidate_seismic_spike::definition();
    let registered = card_definitions().into_iter().find(|card| card.id == CARD_ID).ok_or("Seismic Spike is absent from the RAV definition registry")?;
    if candidate != registered || !definition_identity_valid(&candidate) {
        return Err("candidate Seismic Spike definition differs from its RAV identity".to_owned());
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == "Seismic Spike"
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(2, [Color::Red, Color::Red])
        && definition.colors == BTreeSet::from([Color::Red])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Sorcery])
        && !definition.is_basic_land
        && definition.supported_rules == ["full-rules-fidelity", "colored-cost-casting", "destroy-target-land", "add-two-red-mana"]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects == [Effect::DestroyTargetLand, Effect::AddManaController { color: Color::Red, amount: 2 }]
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    spike: Option<ObjectId>,
    land: Option<ObjectId>,
    land_is_mountain: bool,
    mana_added: bool,
    followup_slasher: Option<ObjectId>,
    combat_source: Option<ObjectId>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SEISMIC_SPIKE_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_SEISMIC_SPIKE_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed).and_then(|captured| validate_trace(&captured, seed).map(|_| captured)) {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!("no clean Seismic Spike full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![DeckEntry { card: CARD_ID.to_owned(), count: 4 }, DeckEntry { card: SLASHER_ID.to_owned(), count: 4 }, DeckEntry { card: LAND_ID.to_owned(), count: 52 }],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry { card: TARGET_LAND_ID.to_owned(), count: 60 }],
        sideboard: vec![],
    };
    for (label, deck) in [("Seismic Spike pilot", &pilot_deck), ("Mountain opponent", &opponent_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 { return Err(format!("{label} deck has {count} cards; expected 60")); }
    }
    let mut game = new_rav_game(2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(SeismicSpikePolicy { player: PlayerId(0), spike_cast: false }), Box::new(LandPolicy { player: PlayerId(1) })];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > 50 || accepted_moves >= 4000 { return Err(format!("Seismic Spike game exceeded its action bound at turn {}", game.turn)); }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) { action }
            else if view.optional_triggered_ability_choice.is_some() { policy.propose_optional_triggered_ability(&view) }
            else if view.draw_replacement_pending { policy.propose_draw_replacement(&view) }
            else if view.private_library_choice.is_some() { policy.propose_private_library_choice(&view) }
            else if view.private_opponent_library_choice.is_some() { policy.propose_private_opponent_library_choice(&view) }
            else if view.library_search_choice.is_some() { policy.propose_library_search_choice(&view) }
            else { policy.propose_move(&view) };
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("policy move rejected: {error}"))?;
        accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    let spike = events.iter().find_map(|event| match event {
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if colors.len() == 4 && colors.iter().all(|color| *color == Color::Red) && game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let land = spike.and_then(|source| events.iter().find_map(|event| match event {
        GameEvent::CardDestroyed { source: event_source, card } if *event_source == source => Some(*card),
        _ => None,
    }));
    let land_is_mountain = land.is_some_and(|card| game.card_definition(card).is_ok_and(|definition| definition.id == TARGET_LAND_ID));
    let mana_added = spike.is_some_and(|spell| {
        let destroyed = events.iter().position(|event| matches!(event, GameEvent::CardDestroyed { source, .. } if *source == spell));
        let resolved = events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell));
        matches!((destroyed, resolved), (Some(start), Some(end)) if events.iter().enumerate().any(|(index, event)| index > start && index < end && matches!(event, GameEvent::ManaAdded { player: PlayerId(0), color: Color::Red, amount: 2 })))
    });
    let followup_slasher = spike.and_then(|spell| {
        let resolved = events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))?;
        events.iter().enumerate().skip(resolved + 1).find_map(|(_, event)| match event {
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                if colors.len() == 2 && colors.iter().all(|color| *color == Color::Red) && game.card_definition(*card).is_ok_and(|definition| definition.id == SLASHER_ID) => Some(*card),
            _ => None,
        })
    });
    let combat_source = events.iter().find_map(|event| match event {
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount }
            if *amount > 0 && game.card_definition(*source).is_ok_and(|definition| definition.id == SLASHER_ID) => Some(*source),
        _ => None,
    });
    Ok(CapturedMatch {
        events,
        terminal: game.winner().map_or_else(|| "Draw".to_owned(), |winner| format!("Winner(PlayerId({}))", winner.0)),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves,
        spike,
        land,
        land_is_mountain,
        mana_added,
        followup_slasher,
        combat_source,
    })
}

struct SeismicSpikePolicy { player: PlayerId, spike_cast: bool }
struct LandPolicy { player: PlayerId }

impl CodePolicy for SeismicSpikePolicy {
    fn id(&self) -> &'static str { "rav.seismic-spike-destroy-add-mana.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: view.own_battlefield.iter().filter(|card| card.can_attack).map(|card| card.id).collect() };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| card.definition == Some(LAND_ID)) {
            return PolicyAction::PlayLand { card: land.id };
        }
        let main_phase = view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain);
        if main_phase && !self.spike_cast {
            let target = view.opponent_battlefield.iter().find(|card| card.definition == Some(TARGET_LAND_ID));
            let spike = view.hand.iter().find(|card| card.definition == Some(CARD_ID));
            if let (Some(spike), Some(target)) = (spike, target) {
                if view.mana_pool.amount(Color::Red) >= 4 {
                    self.spike_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest { card: spike.id, targets: vec![Target::Permanent(target.id)], convoke: vec![], payment_mana_abilities: vec![] },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection { generic: vec![Color::Red; 2], hybrid: vec![] },
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
                }
            }
        }
        if main_phase {
            if let Some(slasher) = view.hand.iter().find(|card| card.definition == Some(SLASHER_ID)) {
                // Preserve a Viashino Slasher until Spike can use its extra mana, then spend that mana immediately.
                let spike_waiting = !self.spike_cast && view.hand.iter().any(|card| card.definition == Some(CARD_ID))
                    && view.opponent_battlefield.iter().any(|card| card.definition == Some(TARGET_LAND_ID));
                if !spike_waiting && view.mana_pool.amount(Color::Red) >= 2 {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest { card: slasher.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection { generic: vec![Color::Red], hybrid: vec![] },
                    };
                }
                if !spike_waiting && let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for LandPolicy {
    fn id(&self) -> &'static str { "rav.mountain-pass.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.active_player != self.player || view.priority != self.player || view.stack_depth != 0 || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 && let Some(land) = view.hand.iter().find(|card| card.definition == Some(LAND_ID)) {
            return PolicyAction::PlayLand { card: land.id };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) { return Err("Seismic Spike match did not reach a seat-zero terminal win".to_owned()); }
    let spike = captured.spike.ok_or("Seismic Spike was not cast with its red cost")?;
    let land = captured.land.ok_or("Seismic Spike did not destroy its targeted land")?;
    let slasher = captured.followup_slasher.ok_or("Seismic Spike's added mana was not used to cast a Viashino Slasher")?;
    if land == spike || slasher == spike || slasher == land { return Err("Seismic Spike, land target, and follow-up Slasher identities are not distinct".to_owned()); }
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spike && colors == &[Color::Red, Color::Red, Color::Red, Color::Red])).ok_or("Seismic Spike {2}{R}{R} payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spike)).ok_or("Seismic Spike SpellCast receipt missing")?;
    let resolved = captured.events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spike)).ok_or("Seismic Spike SpellResolved receipt missing")?;
    let destroyed = captured.events.iter().position(|event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == spike && *card == land)).ok_or("Seismic Spike did not record its land destruction source and target")?;
    let target_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == land)).ok_or("destroyed land did not move to its owner's graveyard")?;
    let mana = captured.events.iter().enumerate().find_map(|(index, event)| {
        (index > target_graveyard && index < resolved && matches!(event, GameEvent::ManaAdded { player: PlayerId(0), color: Color::Red, amount: 2 })).then_some(index)
    }).ok_or("Seismic Spike did not add exactly two red mana during resolution")?;
    let spell_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spike)).ok_or("resolved Seismic Spike did not move to its owner's graveyard")?;
    let slasher_payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == slasher && colors == &[Color::Red, Color::Red])).ok_or("follow-up Viashino Slasher was not paid with two red mana")?;
    let slasher_cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == slasher)).ok_or("follow-up Viashino Slasher SpellCast receipt missing")?;
    let slasher_resolved = captured.events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == slasher)).ok_or("follow-up Viashino Slasher did not resolve")?;
    let slasher_entered = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == slasher)).ok_or("follow-up Viashino Slasher did not enter the battlefield")?;
    if !(payment < cast && cast < destroyed && destroyed < target_graveyard && target_graveyard < mana && mana < resolved && resolved < spell_graveyard && spell_graveyard < slasher_payment && slasher_payment < slasher_cast && slasher_cast < slasher_resolved && slasher_resolved < slasher_entered) {
        return Err(format!("Seismic Spike printed resolution/sequence order differs: payment={payment}, cast={cast}, destroyed={destroyed}, target_graveyard={target_graveyard}, mana={mana}, resolved={resolved}, spell_graveyard={spell_graveyard}, slasher_payment={slasher_payment}, slasher_cast={slasher_cast}, slasher_resolved={slasher_resolved}, slasher_entered={slasher_entered}"));
    }
    if !captured.land_is_mountain { return Err("Seismic Spike did not destroy an opposing Mountain target".to_owned()); }
    let combat = captured.events.iter().enumerate().find_map(|(index, event)| match event {
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == slasher && *amount > 0 => Some((index, *source, *amount)),
        _ => None,
    }).ok_or("Seismic Spike pilot did not complete a Viashino Slasher combat win")?;
    let terminal = captured.events.iter().position(|event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("Seismic Spike terminal event missing")?;
    if !(slasher_entered < combat.0 && combat.0 < terminal) { return Err("Seismic Spike's follow-up Slasher did not precede the terminal combat win".to_owned()); }
    let order = [payment, cast, destroyed, target_graveyard, mana, resolved, spell_graveyard, slasher_payment, slasher_cast, slasher_resolved, slasher_entered, combat.0, terminal];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events: Vec<ProjectedEvent> = project_events(&captured.events).into_iter().filter(|event| selected.contains(&(event.sequence as usize))).collect();
    Ok(json!({
        "id": "rav-seismic-spike-destroys-land-adds-two-red",
        "seed": seed,
        "decks": ["rav-seismic-spike-pilot-60", "rav-mountain-opponent-60"],
        "policies": ["rav.seismic-spike-destroy-add-mana.v1", "rav.mountain-pass.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "spike_object": spike.0,
        "land_object": land.0,
        "land_definition": TARGET_LAND_ID,
        "mana_added_by_spike": {"color": "red", "amount": 2},
        "followup_slasher_object": slasher.0,
        "land_destroyed_by_spike": true,
        "followup_spell_paid_with_added_mana": true,
        "combat_damage_source": combat.1.0,
        "combat_damage_amount": combat.2,
        "ordered_event_sequence": order,
        "events": events,
        "passed": true,
        "combat_source_is_followup_slasher": combat.1 == slasher
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), CardSemanticStatus::CatalogOnly { .. } => None };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str { if std::env::args().any(|argument| argument == "--candidate") { "candidate" } else { "reference" } }
fn output_root() -> Option<std::path::PathBuf> { let mut args = std::env::args().skip(1); while let Some(arg) = args.next() { if arg == "--output-root" { return args.next().map(std::path::PathBuf::from); } } None }
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> { std::fs::create_dir_all(root)?; std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?; std::fs::write(root.join("reward.txt"), "0.0\n") }
