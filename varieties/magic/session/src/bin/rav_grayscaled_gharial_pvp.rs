//! Full-game diagnostic authority for RAV Grayscaled Gharial.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Zone,
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
const CARD_ID: &str = "RAV-GRAYSCALED-GHARIAL";
const BLOCKER_ID: &str = "RAV-DRIFT-OF-PHANTASMS";
const ISLAND_ID: &str = "RAV-ISLAND";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/grayscaled_gharial.rs";
const MAX_TRACE_SEED: u64 = 64;
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/grayscaled_gharial.rs"]
mod candidate_grayscaled_gharial;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Grayscaled Gharial result: {error}");
                return ExitCode::FAILURE;
            }
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
            eprintln!("Grayscaled Gharial PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveIslandwalk,
    CostChanged,
    PowerChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    candidate: Option<ObjectId>,
    payment: Vec<Color>,
    exact_blue_cost: bool,
    spell_resolved: bool,
    battlefield_entry: bool,
    power: Option<i32>,
    toughness: Option<i32>,
    islandwalk: bool,
    opponent_island: Option<ObjectId>,
    blocker: Option<ObjectId>,
    blocker_resolved: bool,
    blocker_entered: bool,
    blocker_can_block: bool,
    attempted_block: bool,
    block_rejection: Option<String>,
    rejection_preserved_state: bool,
    empty_block_declared: bool,
    unblocked_damage: i32,
    attack_turns: Vec<u32>,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Grayscaled Gharial full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "islandwalk_removed_mutation_detected_in_block_legality": mutation_detected(seed, Mutation::RemoveIslandwalk)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::CostChanged)?,
        "power_changed_mutation_detected_in_combat": mutation_detected(seed, Mutation::PowerChanged)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "islandwalk_removed_mutation_detected_in_block_legality",
        "printed_cost_changed_mutation_detected",
        "power_changed_mutation_detected_in_combat",
    ] {
        if mutations[key] != true {
            return Err(format!("Grayscaled Gharial gameplay mutant escaped: {key}"));
        }
    }

    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 52, "printings": 1, "definition_id": CARD_ID, "name": "Grayscaled Gharial"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["702.14: landwalk", "509.1: declare blockers", "510.1: combat damage", "601.2: cast creature spells"],
            "source_contract": "RAV Grayscaled Gharial #52 is a blue 1/1 creature costing {U} with islandwalk.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_pilot_trace_passed; independent review pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_grayscaled_gharial::definition();
    match mutation {
        Mutation::None => {}
        Mutation::RemoveIslandwalk => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Landwalk(BasicLandType::Island)),
        Mutation::CostChanged => definition.mana_cost = ManaCost::new(1),
        Mutation::PowerChanged => definition.power = Some(2),
    }
    definition
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Grayscaled Gharial",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "islandwalk",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Landwalk(BasicLandType::Island)],
        effects: vec![],
    }
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_grayscaled_gharial::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Grayscaled Gharial is missing from the RAV registry")?;
    if registered != definition || definition != expected_definition() {
        return Err("Grayscaled Gharial candidate definition differs from the exact RAV #52 source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Grayscaled Gharial source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Grayscaled Gharial source is not valid Rust".to_owned())?;
    if file.items.len() != 3 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Grayscaled Gharial module must contain only private imports and one literal function"
                .to_owned(),
        );
    }
    let mut definition_seen = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(&item.vis, Visibility::Inherited) && safe_use_tree(&item.tree) => {}
            Item::Fn(function) if function.sig.ident == "definition" && !definition_seen => {
                definition_seen = true;
                if !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err("Grayscaled Gharial definition must be a direct literal".to_owned());
                }
            }
            _ => {
                return Err(
                    "Grayscaled Gharial source contains executable or unsupported top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if !definition_seen {
        return Err("Grayscaled Gharial definition() is missing".to_owned());
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet"
                | "BasicLandType"
                | "CardDefinition"
                | "CardType"
                | "Color"
                | "Keyword"
                | "ManaCost"
        ),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(
            value.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments = path
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>();
            matches!(segments.as_slice(), [a, b] if (a == "Color" && b == "Blue") || (a == "CardType" && b == "Creature") || (a == "BasicLandType" && b == "Island"))
                || segments.as_slice() == ["None"]
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments = path
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>();
            (matches!(segments.as_slice(), [a, b] if (a == "ManaCost" && (b == "with_colors" || b == "new")) || (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "Keyword" && b == "Landwalk"))
                || segments.as_slice() == ["Some"])
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            literal.path.is_ident("CardDefinition")
                && literal.rest.is_none()
                && literal
                    .fields
                    .iter()
                    .all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            use syn::parse::Parser;
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Grayscaled Gharial is missing from the RAV registry")? =
        candidate_definition(mutation);
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GRAYSCALED_GHARIAL_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_GRAYSCALED_GHARIAL_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Grayscaled Gharial landwalk trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (ISLAND_ID, 56)]);
    let opponent = deck(&[(BLOCKER_ID, 4), (ISLAND_ID, 56)]);
    if [&pilot, &opponent].iter().any(|list| {
        list.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err(
            "Grayscaled Gharial decks must each contain legal 60-card mainboards".to_owned(),
        );
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GharialPolicy::new(PlayerId(0), mutation)),
        Box::new(PhantasmPolicy::new(PlayerId(1))),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        candidate: None,
        payment: vec![],
        exact_blue_cost: false,
        spell_resolved: false,
        battlefield_entry: false,
        power: None,
        toughness: None,
        islandwalk: false,
        opponent_island: None,
        blocker: None,
        blocker_resolved: false,
        blocker_entered: false,
        blocker_can_block: false,
        attempted_block: false,
        block_rejection: None,
        rejection_preserved_state: false,
        empty_block_declared: false,
        unblocked_damage: 0,
        attack_turns: vec![],
        life: [20, 20],
    };
    let mut probe_done = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!(
                "Grayscaled Gharial duel exceeded its run bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && !view.blockers_declared
            && !probe_done
        {
            let attacker = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
            if let (Some(attacker), Some(blocker), Some(island)) = (
                attacker,
                blocker,
                view.own_battlefield.iter().find(|card| {
                    card.definition == Some(ISLAND_ID) && card.card_types.contains(&CardType::Land)
                }),
            ) {
                capture.candidate = Some(attacker.id);
                capture.blocker = Some(blocker.id);
                capture.opponent_island = Some(island.id);
                capture.blocker_can_block = blocker.can_block;
                capture.attempted_block = true;
                let before_events = game.event_log.len();
                let before_view = view.clone();
                let attempted = PolicyAction::DeclareBlockers {
                    assignments: vec![CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
                    }],
                };
                match game.submit_policy_move(player, "rav.grayscaled-gharial-islandwalk-legality-probe.v1", attempted) {
                    Ok(()) => return Err("a creature was allowed to block Grayscaled Gharial while its controller controlled an Island".to_owned()),
                    Err(error) => {
                        let message = error.to_string();
                        if !message.contains("landwalk attacker cannot be blocked while defender controls its land type") {
                            return Err(format!("unexpected Islandwalk legality-probe rejection: {message}"));
                        }
                        let after_view = game.view_for_player(player).map_err(|error| error.to_string())?;
                        let unchanged = game.event_log.len() == before_events && after_view == before_view;
                        if !unchanged {
                            return Err("rejected Islandwalk probe changed the live game state".to_owned());
                        }
                        capture.block_rejection = Some(message);
                        capture.rejection_preserved_state = unchanged;
                        probe_done = true;
                        continue;
                    }
                }
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Grayscaled Gharial policy seat missing")?;
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
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
            && capture.candidate.is_none()
        {
            capture.candidate = Some(request.card);
        }
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == BLOCKER_ID)
            && capture.blocker.is_none()
        {
            capture.blocker = Some(request.card);
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Grayscaled Gharial policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move Grayscaled Gharial invariant failure: {error}"))?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == capture.candidate => capture.payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == capture.candidate => {
                capture.spell_resolved = true
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == capture.candidate => {
                capture.battlefield_entry = true;
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                capture.power = characteristics.power;
                capture.toughness = characteristics.toughness;
                capture.islandwalk = characteristics
                    .keywords
                    .contains(&Keyword::Landwalk(BasicLandType::Island));
                capture.exact_blue_cost = game.card_definition(*card).is_ok_and(|definition| {
                    definition.mana_cost == ManaCost::with_colors(0, [Color::Blue])
                });
            }
            GameEvent::SpellResolved { card } if Some(*card) == capture.blocker => {
                capture.blocker_resolved = true
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == capture.blocker => capture.blocker_entered = true,
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == ISLAND_ID)
                && game
                    .object(*card)
                    .is_ok_and(|object| object.owner == PlayerId(1)) =>
            {
                capture.opponent_island.get_or_insert(*card);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if capture
                .candidate
                .is_some_and(|candidate| attackers.contains(&candidate)) =>
            {
                capture.attack_turns.push(game.turn)
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if assignments.is_empty() && capture.attempted_block => {
                capture.empty_block_declared = true
            }
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                amount,
                source,
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.unblocked_damage += *amount
            }
            _ => {}
        }
    }
    Ok(())
}

struct GharialPolicy {
    player: PlayerId,
    mutation: Mutation,
}
struct PhantasmPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl GharialPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self { player, mutation }
    }
}
impl PhantasmPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast_once: false,
        }
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn land_action(view: &GameView) -> Option<PolicyAction> {
    if view.lands_played != 0 {
        return None;
    }
    let land = view
        .hand
        .iter()
        .find(|card| card.definition == Some(ISLAND_ID))?;
    Some(PolicyAction::PlayLand { card: land.id })
}

fn mana_action(view: &GameView, wanted: Color) -> Option<PolicyAction> {
    let land = view
        .own_battlefield
        .iter()
        .find(|card| card.definition == Some(ISLAND_ID) && !card.tapped)?;
    if !land.mana_colors.contains(&wanted) {
        return None;
    }
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color: wanted,
    })
}

impl CodePolicy for GharialPolicy {
    fn id(&self) -> &'static str {
        "rav.grayscaled-gharial-islandwalk-attack.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
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
        if view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = land_action(view) {
            return action;
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = candidate_definition(self.mutation).mana_cost;
            if let Some(selection) = payment(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(missing) = cost
                .colored
                .iter()
                .find(|color| view.mana_pool.amount(**color) == 0)
                .copied()
                && let Some(action) = mana_action(view, missing)
            {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for PhantasmPolicy {
    fn id(&self) -> &'static str {
        "rav.drift-of-phantasms-island-blocker.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
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
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = land_action(view) {
            return action;
        }
        if !self.cast_once
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID))
        {
            let cost = ManaCost::with_colors(2, [Color::Blue]);
            if let Some(selection) = payment(view, &cost) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(missing) = cost
                .colored
                .iter()
                .find(|color| view.mana_pool.amount(**color) == 0)
                .copied()
                && let Some(action) = mana_action(view, missing)
            {
                return action;
            }
            if let Some(action) = mana_action(view, Color::Blue) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) {
        return Err("Grayscaled Gharial full game did not end in a pilot win".to_owned());
    }
    let candidate = capture.candidate.ok_or("Grayscaled Gharial was not cast")?;
    let blocker = capture
        .blocker
        .ok_or("opponent Drift of Phantasms was not cast")?;
    let island = capture
        .opponent_island
        .ok_or("the defending player did not control an Island")?;
    if !capture.spell_resolved
        || !capture.battlefield_entry
        || !capture.exact_blue_cost
        || capture.payment != [Color::Blue]
        || capture.power != Some(1)
        || capture.toughness != Some(1)
        || !capture.islandwalk
    {
        return Err(format!(
            "Grayscaled Gharial did not cast for {{U}} as a live blue 1/1 with Islandwalk: payment={:?}, P/T={:?}/{:?}, Islandwalk={}",
            capture.payment, capture.power, capture.toughness, capture.islandwalk
        ));
    }
    if !capture.blocker_resolved
        || !capture.blocker_entered
        || !capture.blocker_can_block
        || !capture.attempted_block
        || !capture.rejection_preserved_state
        || !capture.empty_block_declared
        || !capture.block_rejection.as_deref().is_some_and(|message| {
            message.contains(
                "landwalk attacker cannot be blocked while defender controls its land type",
            )
        })
    {
        return Err(format!(
            "Islandwalk did not reject an attempted block by a live Drift while its controller had an Island: blocker={blocker:?}, can_block={}, attempted={}, rejection={:?}, state_unchanged={}, empty_declaration={}",
            capture.blocker_can_block,
            capture.attempted_block,
            capture.block_rejection,
            capture.rejection_preserved_state,
            capture.empty_block_declared
        ));
    }
    if capture.attack_turns.is_empty()
        || capture.unblocked_damage < 20
        || capture.life[1] > 0
        || capture.life[0] <= 0
    {
        return Err(
            "Grayscaled Gharial pilot did not finish the full game through combat".to_owned(),
        );
    }
    let paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == candidate && colors == &[Color::Blue])).ok_or("Grayscaled Gharial exact blue payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == candidate)).ok_or("Grayscaled Gharial SpellCast missing")?;
    let resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == candidate),
    )
    .ok_or("Grayscaled Gharial SpellResolved missing")?;
    let entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == candidate)).ok_or("Grayscaled Gharial battlefield entry missing")?;
    let land = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == island)).ok_or("opponent Island entry missing")?;
    let blocker_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)).ok_or("Drift of Phantasms SpellCast missing")?;
    let blocker_resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == blocker),
    )
    .ok_or("Drift of Phantasms SpellResolved missing")?;
    let blocker_entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blocker)).ok_or("Drift of Phantasms battlefield entry missing")?;
    let attack = event_index_after(&capture.events, blocker_entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&candidate))).ok_or("Grayscaled Gharial did not attack after the blocker entered")?;
    let blocks = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("defender did not submit the legal empty block declaration")?;
    let damage = event_index_after(&capture.events, blocks, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if *source == candidate)).ok_or("unblocked Grayscaled Gharial combat damage receipt missing")?;
    let ended = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal pilot GameEnded receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && land < attack
        && blocker_cast < blocker_resolved
        && blocker_resolved < blocker_entered
        && blocker_entered < attack
        && attack < blocks
        && blocks < damage
        && damage < ended)
    {
        return Err(format!(
            "Grayscaled Gharial cast, defending Island/blocker, evasion combat, and terminal event order differs: payment={paid},cast={cast},resolved={resolved},entered={entered}, island={land}, blocker=({blocker_cast},{blocker_resolved},{blocker_entered}), combat=({attack},{blocks},{damage},{ended})"
        ));
    }
    let selected: BTreeSet<_> = [
        paid,
        cast,
        resolved,
        entered,
        land,
        blocker_cast,
        blocker_resolved,
        blocker_entered,
        attack,
        blocks,
        damage,
        ended,
    ]
    .into_iter()
    .collect();
    let projected = project_events(&capture.events);
    let events: Vec<_> = selected
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "id": "rav-grayscaled-gharial-islandwalk-block-legality-terminal",
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": 0,
        "grayscaled_gharial": {"object": candidate.0, "collector_number": 52, "mana_cost": "{U}", "mana_paid": capture.payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "color": "Blue", "power": capture.power, "toughness": capture.toughness, "islandwalk": capture.islandwalk},
        "evasion_interaction": {"defending_island_object": island.0, "defending_player_controls_island": true, "blocker_definition": BLOCKER_ID, "blocker_object": blocker.0, "blocker_can_block": capture.blocker_can_block, "attempted_block_rejected": capture.attempted_block, "rejection": capture.block_rejection, "match_state_unchanged": capture.rejection_preserved_state, "legal_block_assignments": [], "empty_block_declared": capture.empty_block_declared},
        "combat": {"attack_turns": capture.attack_turns, "unblocked_gharial_damage": capture.unblocked_damage, "life_at_terminal": capture.life},
        "ordered_event_sequence": selected.iter().copied().collect::<Vec<_>>(),
        "events": events,
        "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut found = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
