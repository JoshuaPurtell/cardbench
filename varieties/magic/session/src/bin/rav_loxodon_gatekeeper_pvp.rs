//! Full-game RAV Loxodon Gatekeeper entry-replacement authority.
//!
//! A legal constructed match resolves the candidate-owned Gatekeeper, verifies
//! that the opponent's next Plains enters tapped from this exact source, and
//! completes the game through creature combat.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, StaticEntryRestriction,
    StaticEntryRestrictionBinding, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, new_rav_game,
    rav_main_set_catalog, rav_static_entry_restriction_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LOXODON-GATEKEEPER";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_gatekeeper.rs";
const MAX_TRACE_SEED: u64 = 32;
const LAND_ID: &str = "RAV-PLAINS";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/loxodon_gatekeeper.rs"]
mod candidate_loxodon_gatekeeper;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Loxodon Gatekeeper PvP result: {error}");
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
            eprintln!("Loxodon Gatekeeper PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_identity()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.gatekeeper != replay.gatekeeper
        || captured.opponent_land != replay.opponent_land
        || captured.entry_source != replay.entry_source
    {
        return Err("Loxodon Gatekeeper full-game replay was not deterministic".to_owned());
    }

    let mut wrong_definition = candidate_loxodon_gatekeeper::definition();
    wrong_definition.power = Some(3);
    let mut wrong_binding = candidate_loxodon_gatekeeper::static_entry_restriction_binding();
    wrong_binding.restriction = StaticEntryRestriction::SourceEntersTapped;
    let mutation_detected = !definition_identity_valid(&wrong_definition)
        && !binding_identity_valid(&wrong_binding);
    if !mutation_detected {
        return Err("Loxodon Gatekeeper identity mutation control was ineffective".to_owned());
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
        "card": {"collector_number": 25, "printings": 1, "definition_id": CARD_ID, "name": "Loxodon Gatekeeper"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: white colored costs and payment",
                "117: timing and priority",
                "302.1-302.2: casting a creature spell and battlefield entry",
                "601.2: casting a spell and paying its costs",
                "608.3a: resolving a nontargeted permanent spell",
                "614: replacement effects",
                "614.12: effects that modify how permanents enter the battlefield",
            ],
            "source_contract": "RAV Loxodon Gatekeeper identity and its opponent artifact, creature, and land entry-tapped restriction",
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
        "qualification_status": qualification_status(),
        "mutation_sensitivity": {
            "definition_power_mutation_detected": !definition_identity_valid(&wrong_definition),
            "entry_restriction_mutation_detected": !binding_identity_valid(&wrong_binding),
            "opponent_land_entered_tapped_from_live_source": trace["passed"].as_bool() == Some(true),
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Loxodon Gatekeeper source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Loxodon Gatekeeper source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 4 {
        return Err("Loxodon Gatekeeper source has an unsupported module shape".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "StaticEntryRestriction"],
        vec!["cardbench_magic_engine", "StaticEntryRestrictionBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                if !collect_use_paths(&mut Vec::new(), &item.tree, &mut actual_imports) {
                    return Err("Loxodon Gatekeeper source has an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !function_is_literal_only(function, &name) {
                    return Err(format!(
                        "Loxodon Gatekeeper function `{name}` must return a crate-private literal"
                    ));
                }
            }
            _ => return Err("Loxodon Gatekeeper source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "static_entry_restriction_binding".to_owned()])
    {
        return Err("Loxodon Gatekeeper source is missing its declarative definition and binding".to_owned());
    }
    Ok(())
}

fn collect_use_paths(prefix: &mut Vec<String>, tree: &UseTree, output: &mut BTreeSet<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let valid = collect_use_paths(prefix, &path.tree, output);
            prefix.pop();
            valid
        }
        UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            output.insert(prefix.clone());
            prefix.pop();
            true
        }
        UseTree::Group(group) => group.items.iter().all(|item| collect_use_paths(prefix, item, output)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn function_is_literal_only(function: &syn::ItemFn, expected_name: &str) -> bool {
    let (return_type, struct_type) = match expected_name {
        "definition" => ("CardDefinition", "CardDefinition"),
        "static_entry_restriction_binding" => (
            "StaticEntryRestrictionBinding",
            "StaticEntryRestrictionBinding",
        ),
        _ => return false,
    };
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_private = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    if !attrs_ok
        || !crate_private
        || function.sig.ident != expected_name
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || !function.sig.inputs.is_empty()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_name(ty).as_deref() == Some(return_type))
        || function.block.stmts.len() != 1
    {
        return false;
    }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
        return false;
    };
    type_name_path(&literal.path).as_deref() == Some(struct_type)
        && literal.rest.is_none()
        && !literal.attrs.iter().any(|_| true)
        && literal.fields.iter().all(|field| {
            matches!(&field.member, syn::Member::Named(_))
                && field.colon_token.is_some()
                && field.attrs.is_empty()
                && safe_expression(&field.expr)
        })
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => literal.attrs.is_empty() && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(path) => {
            let segments: Vec<String> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            match segments.as_slice() {
                [single] => single == "None",
                [kind, variant] if kind == "Color" => ["White", "Blue", "Black", "Red", "Green", "Colorless"].contains(&variant.as_str()),
                [kind, variant] if kind == "CardType" => ["Creature", "Land", "Instant", "Sorcery", "Enchantment", "Artifact"].contains(&variant.as_str()),
                [kind, variant] if kind == "StaticEntryRestriction" => [
                    "OpponentsArtifactsCreaturesAndLandsEnterTapped",
                    "SourceEntersTapped",
                    "SourceEntersWithPlusOneCountersEqualToControllerGraveyardCreatureCards",
                ].contains(&variant.as_str()),
                _ => false,
            }
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return false; };
            let segments: Vec<String> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            let allowed = matches!(segments.as_slice(), [ty, method] if (ty == "ManaCost" && method == "with_colors") || (ty == "BTreeSet" && (method == "from" || method == "new")) || (ty == "Some" && method == "new"))
                || segments.as_slice() == ["Some"];
            allowed && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) => mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty(),
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(literal.lit, Lit::Str(_))))
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None; };
    path.path.segments.last().map(|segment| segment.ident.to_string())
}

fn type_name_path(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|segment| segment.ident.to_string())
}

fn validate_candidate_identity() -> Result<(), String> {
    let definition = candidate_loxodon_gatekeeper::definition();
    let binding = candidate_loxodon_gatekeeper::static_entry_restriction_binding();
    let registered = card_definitions().into_iter().find(|card| card.id == CARD_ID)
        .ok_or("Loxodon Gatekeeper is absent from the RAV definition registry")?;
    let registered_binding = rav_static_entry_restriction_bindings().into_iter()
        .find(|candidate| candidate.card_definition == CARD_ID)
        .ok_or("Loxodon Gatekeeper entry restriction is absent from the RAV binding registry")?;
    if definition != registered || !definition_identity_valid(&definition) || binding != registered_binding || !binding_identity_valid(&binding) {
        return Err("candidate Loxodon Gatekeeper definition or entry restriction differs from its RAV identity".to_owned());
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == "Loxodon Gatekeeper"
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(2, [Color::White, Color::White])
        && definition.colors == BTreeSet::from([Color::White])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Creature])
        && !definition.is_basic_land
        && definition.supported_rules == [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-opponents-artifacts-creatures-lands-enter-tapped",
        ]
        && definition.power == Some(2)
        && definition.toughness == Some(3)
        && definition.keywords.is_empty()
        && definition.effects.is_empty()
}

fn binding_identity_valid(binding: &StaticEntryRestrictionBinding) -> bool {
    binding.card_definition == CARD_ID
        && binding.restriction == StaticEntryRestriction::OpponentsArtifactsCreaturesAndLandsEnterTapped
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    gatekeeper: Option<ObjectId>,
    opponent_land: Option<ObjectId>,
    entry_source: Option<ObjectId>,
    tapped_receipt: bool,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_LOXODON_GATEKEEPER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_LOXODON_GATEKEEPER_SEED must be an unsigned integer".to_owned())?;
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
    Err(format!("no clean Loxodon Gatekeeper full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry { card: CARD_ID.to_owned(), count: 4 },
            DeckEntry { card: LAND_ID.to_owned(), count: 56 },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry { card: LAND_ID.to_owned(), count: 60 }],
        sideboard: vec![],
    };
    for (label, deck) in [("Loxodon Gatekeeper pilot", &pilot_deck), ("Plains opponent", &opponent_deck)] {
        let cards: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if cards != 60 { return Err(format!("{label} deck has {cards} cards; expected 60")); }
    }
    let mut game = new_rav_game(2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GatekeeperPolicy { player: PlayerId(0) }),
        Box::new(PlainsPolicy { player: PlayerId(1) }),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > 60 || accepted_moves >= 4000 {
            return Err(format!("Loxodon Gatekeeper game exceeded its action bound at turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("policy move rejected: {error}"))?;
        accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    let gatekeeper = events.iter().find_map(|event| match event {
        GameEvent::PermanentEnteredTapped { source, controller: PlayerId(1), .. }
            if game.card_definition(*source).is_ok_and(|definition| definition.id == CARD_ID)
                && game.controller_of(*source).is_ok_and(|controller| controller == PlayerId(0)) => Some(*source),
        _ => None,
    });
    let land_receipt = events.iter().find_map(|event| match event {
        GameEvent::PermanentEnteredTapped { permanent, source, controller: PlayerId(1), .. }
            if Some(*source) == gatekeeper
                && game.card_definition(*permanent).is_ok_and(|definition| definition.id == LAND_ID) => Some((*permanent, *source)),
        _ => None,
    });
    let opponent_land = land_receipt.map(|(land, _)| land);
    let entry_source = land_receipt.map(|(_, source)| source);
    let tapped_receipt = land_receipt.is_some();
    Ok(CapturedMatch {
        events,
        terminal: game.winner().map_or_else(|| "Draw".to_owned(), |winner| format!("Winner(PlayerId({}))", winner.0)),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves,
        gatekeeper,
        opponent_land,
        entry_source,
        tapped_receipt,
    })
}

struct GatekeeperPolicy { player: PlayerId }
struct PlainsPolicy { player: PlayerId }

impl CodePolicy for GatekeeperPolicy {
    fn id(&self) -> &'static str { "rav.loxodon-gatekeeper-entry-tapped.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view.own_battlefield.iter().filter(|card| card.can_attack).map(|card| card.id).collect(),
            };
        }
        if view.player == self.player
            && view.decision_player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && let Some(land) = view.hand.iter().find(|card| card.definition == Some(LAND_ID))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            let gatekeeper = view.hand.iter().find(|card| card.definition == Some(CARD_ID));
            if let Some(gatekeeper) = gatekeeper {
                if view.mana_pool.amount(Color::White) >= 4 && view.mana_pool.total() >= 4 {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest { card: gatekeeper.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection { generic: vec![Color::White; 2], hybrid: vec![] },
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: Color::White };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for PlainsPolicy {
    fn id(&self) -> &'static str { "rav.plains-pass-with-land-play.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.decision_player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| card.definition == Some(LAND_ID))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        match view.step {
            Step::DeclareAttackers if view.active_player == view.player && !view.attackers_declared => PolicyAction::DeclareAttackers { attackers: vec![] },
            Step::DeclareBlockers if view.active_player != view.player && !view.blockers_declared => PolicyAction::DeclareBlockers { assignments: vec![] },
            _ => PolicyAction::PassPriority,
        }
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Loxodon Gatekeeper match did not reach a seat-zero terminal win".to_owned());
    }
    let gatekeeper = captured.gatekeeper.ok_or("Loxodon Gatekeeper did not establish the opponent entry restriction")?;
    let opponent_land = captured.opponent_land.ok_or("no opponent Plains entered tapped from Loxodon Gatekeeper")?;
    if captured.entry_source != Some(gatekeeper) || !captured.tapped_receipt {
        return Err("opponent Plains has no entry-tapped receipt from the live Loxodon Gatekeeper source".to_owned());
    }
    let hand = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == gatekeeper)).ok_or("Loxodon Gatekeeper hand receipt missing")?;
    let paid = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == gatekeeper && colors.len() == 4 && colors.iter().all(|color| *color == Color::White))).ok_or("Loxodon Gatekeeper {2}{W}{W} payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == gatekeeper)).ok_or("Loxodon Gatekeeper SpellCast receipt missing")?;
    let resolved = captured.events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == gatekeeper)).ok_or("Loxodon Gatekeeper SpellResolved receipt missing")?;
    let entered = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == gatekeeper)).ok_or("Loxodon Gatekeeper battlefield entry missing")?;
    let land_entry = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == opponent_land)).ok_or("opponent Plains battlefield entry missing")?;
    let tapped = captured.events.iter().position(|event| matches!(event, GameEvent::PermanentEnteredTapped { permanent, source, controller: PlayerId(1), .. } if *permanent == opponent_land && *source == gatekeeper)).ok_or("opponent Plains was not replaced into the battlefield tapped by this Gatekeeper")?;
    if !(hand < paid && paid < cast && cast < resolved && resolved < entered && entered < land_entry && land_entry < tapped) {
        return Err(format!("Gatekeeper spell and opponent tapped-land event order differs: hand={hand}, paid={paid}, cast={cast}, resolved={resolved}, entered={entered}, land_entry={land_entry}, tapped={tapped}"));
    }
    let combat_damage = captured.events.iter().enumerate().find_map(|(index, event)| match event {
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *amount > 0 && captured.events[..index].iter().any(|prior| matches!(prior, GameEvent::SpellCast { player: PlayerId(0), card } if card == source)) => Some(index),
        _ => None,
    }).ok_or("Loxodon Gatekeeper pilot did not win through creature combat")?;
    let terminal = captured.events.iter().position(|event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("Loxodon Gatekeeper match terminal event missing")?;
    if !(tapped < combat_damage && combat_damage < terminal) {
        return Err("Gatekeeper's opponent land entry did not precede the terminal combat win".to_owned());
    }
    let ordered = [hand, paid, cast, resolved, entered, land_entry, tapped, combat_damage, terminal];
    let required: BTreeSet<usize> = ordered.into_iter().collect();
    let projections = project_events(&captured.events);
    let relevant: Vec<ProjectedEvent> = projections.into_iter().filter(|event| required.contains(&(event.sequence as usize))).collect();
    Ok(json!({
        "id": "rav-loxodon-gatekeeper-opponent-land-enters-tapped",
        "seed": seed,
        "decks": ["rav-loxodon-gatekeeper-pilot-60", "rav-plains-opponent-60"],
        "policies": ["rav.loxodon-gatekeeper-entry-tapped.v1", "rav.plains-pass-with-land-play.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "gatekeeper_object": gatekeeper.0,
        "opponent_land_object": opponent_land.0,
        "entry_restriction_source_object": captured.entry_source.map(|object| object.0),
        "opponent_land_tapped": captured.tapped_receipt,
        "ordered_event_sequence": ordered,
        "events": relevant,
        "passed": true
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
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
fn qualification_status() -> &'static str { if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" } }
fn output_root() -> Option<std::path::PathBuf> {
    let mut args=std::env::args().skip(1);
    while let Some(argument)=args.next() { if argument=="--output-root" { return args.next().map(std::path::PathBuf::from); } }
    None
}
fn write_outputs(root:&std::path::Path,report:&Value)->std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(report).expect("report serializes"))?;
    std::fs::write(root.join("reward.txt"),"0.0\n")
}
