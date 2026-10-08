//! Full-game RAV Smash authority: destroy an opponent artifact, draw, and win.

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
const CARD_ID: &str = "RAV-SMASH";
const SIGNET_ID: &str = "RAV-BOROS-SIGNET";
const SLASHER_ID: &str = "RAV-VIASHINO-SLASHER";
const LAND_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/smash.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/smash.rs"]
mod candidate_smash;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Smash PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(error) => {
            eprintln!("Smash PvP authority failed: {error}");
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
    if captured.events != replay.events || captured.terminal != replay.terminal || captured.winner != replay.winner || captured.smash != replay.smash || captured.artifact != replay.artifact || captured.drawn != replay.drawn || captured.combat_source != replay.combat_source {
        return Err("Smash full-game replay was not deterministic".to_owned());
    }

    let mut wrong_definition = candidate_smash::definition();
    wrong_definition.effects.swap(0, 1);
    let mutation_detected = !definition_identity_valid(&wrong_definition);
    if !mutation_detected { return Err("Smash effect-order mutation control was ineffective".to_owned()); }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown.iter().filter(|card| card["trace_status"] == "passed").count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 143, "printings": 1, "definition_id": CARD_ID, "name": "Smash"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting an instant and paying its costs",
                "608.2b: target legality on resolution",
                "608.3: resolving spell instructions in printed order",
                "701.7: destroying a permanent",
                "121: drawing a card",
            ],
            "source_contract": "RAV Smash destroys a target artifact, then its controller draws one card",
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
            "artifact_destroyed_by_live_smash": trace["passed"].as_bool() == Some(true),
            "post_destruction_card_draw_observed": captured.drawn.is_some(),
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| format!("could not read Smash source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Smash source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Smash source has an unsupported module shape".to_owned());
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
                if !collect_use_paths(&mut Vec::new(), &item.tree, &mut actual_imports) { return Err("Smash source has an unsupported import".to_owned()); }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !definition_function_is_literal(function) {
                    return Err(format!("Smash function `{name}` must return a crate-private literal definition"));
                }
            }
            _ => return Err("Smash source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Smash source is missing its declarative definition".to_owned());
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
                [kind, variant] if kind == "CardType" => variant == "Instant",
                [kind, variant] if kind == "Effect" => ["DestroyTargetArtifact", "DrawController"].contains(&variant.as_str()),
                _ => false,
            }
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
    let candidate = candidate_smash::definition();
    let registered = card_definitions().into_iter().find(|card| card.id == CARD_ID).ok_or("Smash is absent from the RAV definition registry")?;
    if candidate != registered || !definition_identity_valid(&candidate) {
        return Err("candidate Smash definition differs from its RAV identity".to_owned());
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == "Smash"
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(2, [Color::Red])
        && definition.colors == BTreeSet::from([Color::Red])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Instant])
        && !definition.is_basic_land
        && definition.supported_rules == ["full-rules-fidelity", "artifact-destruction", "draw"]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects == [Effect::DestroyTargetArtifact, Effect::DrawController]
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    smash: Option<ObjectId>,
    artifact: Option<ObjectId>,
    artifact_is_signet: bool,
    drawn: Option<ObjectId>,
    combat_source: Option<ObjectId>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SMASH_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_SMASH_SEED must be an unsigned integer".to_owned())?;
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
    Err(format!("no clean Smash full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![DeckEntry { card: CARD_ID.to_owned(), count: 4 }, DeckEntry { card: SLASHER_ID.to_owned(), count: 4 }, DeckEntry { card: LAND_ID.to_owned(), count: 52 }],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry { card: SIGNET_ID.to_owned(), count: 4 }, DeckEntry { card: LAND_ID.to_owned(), count: 56 }],
        sideboard: vec![],
    };
    for (label, deck) in [("Smash pilot", &pilot_deck), ("Boros Signet opponent", &opponent_deck)] {
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

    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(SmashPolicy { player: PlayerId(0) }), Box::new(SignetPolicy { player: PlayerId(1) })];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > 50 || accepted_moves >= 4000 { return Err(format!("Smash game exceeded its action bound at turn {}", game.turn)); }
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
    let smash = events.iter().find_map(|event| match event {
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if colors.len() == 3 && colors.iter().all(|color| *color == Color::Red) && game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let artifact = smash.and_then(|source| events.iter().find_map(|event| match event {
        GameEvent::CardDestroyed { source: event_source, card } if *event_source == source => Some(*card),
        _ => None,
    }));
    let artifact_is_signet = artifact.is_some_and(|card| game.card_definition(card).is_ok_and(|definition| definition.id == SIGNET_ID));
    let drawn = smash.and_then(|spell| {
        let resolved = events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))?;
        let artifact = events.iter().find_map(|event| match event {
            GameEvent::CardDestroyed { source, card } if *source == spell => Some(*card),
            _ => None,
        })?;
        let artifact_graveyard = events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == artifact))?;
        events.iter().enumerate().skip(artifact_graveyard + 1).take(resolved.saturating_sub(artifact_graveyard + 1)).find_map(|(_, event)| match event {
            GameEvent::CardMoved { card, to: Zone::Hand } if *card != spell => Some(*card),
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
        smash,
        artifact,
        artifact_is_signet,
        drawn,
        combat_source,
    })
}

struct SmashPolicy { player: PlayerId }
struct SignetPolicy { player: PlayerId }

impl CodePolicy for SmashPolicy {
    fn id(&self) -> &'static str { "rav.smash-destroy-artifact-draw.v1" }
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
        let target = view.opponent_battlefield.iter().find(|card| card.definition == Some(SIGNET_ID));
        let smash = view.hand.iter().find(|card| card.definition == Some(CARD_ID));
        if let (Some(smash), Some(target)) = (smash, target) {
            if view.mana_pool.total() >= 3 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest { card: smash.id, targets: vec![Target::Permanent(target.id)], convoke: vec![], payment_mana_abilities: vec![] },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection { generic: vec![Color::Red; 2], hybrid: vec![] },
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
            }
        }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) {
            if let Some(slasher) = view.hand.iter().find(|card| card.definition == Some(SLASHER_ID)) {
                if view.mana_pool.total() >= 2 && view.mana_pool.amount(Color::Red) >= 1 {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest { card: slasher.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection { generic: vec![Color::Red], hybrid: vec![] },
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for SignetPolicy {
    fn id(&self) -> &'static str { "rav.boros-signet-pass.v1" }
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
        let signet_live = view.own_battlefield.iter().any(|card| card.definition == Some(SIGNET_ID));
        if !signet_live && let Some(signet) = view.hand.iter().find(|card| card.definition == Some(SIGNET_ID)) {
            if view.mana_pool.total() >= 2 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest { card: signet.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection { generic: vec![Color::Red; 2], hybrid: vec![] },
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| card.definition == Some(LAND_ID) && !card.tapped) {
                return PolicyAction::ActivateManaAbility { land: land.id, color: Color::Red };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) { return Err("Smash match did not reach a seat-zero terminal win".to_owned()); }
    let smash = captured.smash.ok_or("Smash was not cast with its red cost")?;
    let artifact = captured.artifact.ok_or("Smash did not destroy its targeted artifact")?;
    let drawn = captured.drawn.ok_or("Smash did not move one card from library to hand during resolution")?;
    if artifact == smash || drawn == smash || drawn == artifact { return Err("Smash, artifact target, and drawn card identities are not distinct".to_owned()); }
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == smash && colors == &[Color::Red, Color::Red, Color::Red])).ok_or("Smash {2}{R} payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == smash)).ok_or("Smash SpellCast receipt missing")?;
    let resolved = captured.events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == smash)).ok_or("Smash SpellResolved receipt missing")?;
    let destroyed = captured.events.iter().position(|event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == smash && *card == artifact)).ok_or("Smash did not record its artifact destruction source and target")?;
    let target_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == artifact)).ok_or("destroyed artifact did not move to its owner's graveyard")?;
    let draw = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn)).ok_or("Smash draw-to-hand receipt missing")?;
    let spell_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == smash)).ok_or("resolved Smash did not move to its owner's graveyard")?;
    if !(payment < cast && cast < destroyed && destroyed < target_graveyard && target_graveyard < draw && draw < resolved && resolved < spell_graveyard) {
        return Err(format!("Smash printed resolution order differs: payment={payment}, cast={cast}, resolved={resolved}, destroyed={destroyed}, target_graveyard={target_graveyard}, draw={draw}, spell_graveyard={spell_graveyard}"));
    }
    if !captured.artifact_is_signet { return Err("Smash did not destroy the opponent's Boros Signet target".to_owned()); }
    let slasher = captured.combat_source.ok_or("Smash pilot did not win through Viashino Slasher combat")?;
    let combat = captured.events.iter().enumerate().find_map(|(index, event)| match event {
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == slasher && *amount > 0 => Some((index, *source, *amount)),
        _ => None,
    }).ok_or("Smash pilot did not complete a combat win")?;
    let terminal = captured.events.iter().position(|event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("Smash terminal event missing")?;
    if !(spell_graveyard < combat.0 && combat.0 < terminal) { return Err("Smash resolution and draw did not precede the terminal combat win".to_owned()); }
    let order = [payment, cast, destroyed, target_graveyard, draw, resolved, spell_graveyard, combat.0, terminal];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events: Vec<ProjectedEvent> = project_events(&captured.events).into_iter().filter(|event| selected.contains(&(event.sequence as usize))).collect();
    Ok(json!({
        "id": "rav-smash-destroys-artifact-then-draws",
        "seed": seed,
        "decks": ["rav-smash-pilot-60", "rav-boros-signet-opponent-60"],
        "policies": ["rav.smash-destroy-artifact-draw.v1", "rav.boros-signet-pass.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "smash_object": smash.0,
        "artifact_object": artifact.0,
        "artifact_definition": SIGNET_ID,
        "drawn_card_object": drawn.0,
        "artifact_destroyed_by_smash": true,
        "cards_drawn_after_destruction": 1,
        "combat_damage_source": combat.1.0,
        "combat_damage_amount": combat.2,
        "ordered_event_sequence": order,
        "events": events,
        "passed": true,
        "combat_source_is_viashino_slasher": combat.1 == slasher
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
