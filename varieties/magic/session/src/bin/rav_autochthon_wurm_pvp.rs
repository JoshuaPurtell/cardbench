//! Full-game diagnostic authority for RAV Autochthon Wurm.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment,
    DecisionKind, DecisionSelection, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_main_set_catalog,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-AUTOCHTHON-WURM";
const BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const CAREGIVER_ID: &str = "RAV-CAREGIVER";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/autochthon_wurm.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/autochthon_wurm.rs"]
mod candidate_autochthon_wurm;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Autochthon Wurm result: {error}");
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
            eprintln!("Autochthon Wurm PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    NoTrample,
    HigherCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    spell: Option<ObjectId>,
    spell_zone: Option<Zone>,
    paid_colors: Vec<Color>,
    convokers: Vec<(ObjectId, Option<Color>, bool, &'static str, usize)>,
    development_spells: Vec<(ObjectId, &'static str)>,
    candidate_power: Option<i32>,
    candidate_toughness: Option<i32>,
    candidate_has_convoke: bool,
    candidate_has_trample: bool,
    blocker_casts: Vec<ObjectId>,
    blocker_resolved: Vec<ObjectId>,
    blocker_entered: Vec<ObjectId>,
    blocker_definitions: Vec<(ObjectId, &'static str)>,
    blockers: Vec<ObjectId>,
    blocker_zones: Vec<Zone>,
    blocker_assigned: Vec<(ObjectId, ObjectId)>,
    damage_to_blockers: Vec<(ObjectId, i32)>,
    trample_damage: Vec<i32>,
    combat_order: Vec<ObjectId>,
    combat_order_index: Option<usize>,
    blocker_sbas: Vec<ObjectId>,
    blocker_graves: Vec<ObjectId>,
    resolved: bool,
    opponent_life: Option<i64>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
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
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    if !same_match(&reference, &run_match(seed, Mutation::None)?) {
        return Err("Autochthon Wurm shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke)?,
        "trample_removed_mutation_detected_in_combat": mutation_detected(seed, Mutation::NoTrample)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::HigherCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "convoke_removed_mutation_detected_in_game",
        "trample_removed_mutation_detected_in_combat",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Autochthon Wurm gameplay mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 191, "printings": 1, "definition_id": CARD_ID, "name": "Autochthon Wurm"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting creature spells and paying costs",
                "702.50: Convoke",
                "702.19: Trample",
                "508.1: declaring attackers",
                "509.1: declaring blockers",
                "509.2: attacker orders multiple blockers",
                "510.1: combat damage assignment",
                "704.5g: lethal-damage state-based actions"
            ],
            "source_contract": "RAV Autochthon Wurm #191 is a green-white 9/14 creature costing {10}{G}{G}{G}{W}{W} with Convoke and Trample.",
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
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Autochthon Wurm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(
            10,
            [
                Color::Green,
                Color::Green,
                Color::Green,
                Color::White,
                Color::White,
            ],
        ),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "trample",
            "multi-block-trample-combat-damage",
        ],
        power: Some(9),
        toughness: Some(14),
        keywords: vec![Keyword::Convoke, Keyword::Trample],
        effects: vec![],
    }
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_autochthon_wurm::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Autochthon Wurm candidate or registry differs from exact RAV #191 identity")?;
    if candidate != expected_definition() || registered != candidate {
        return Err(
            "Autochthon Wurm candidate or registry differs from exact RAV #191 identity".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Autochthon Wurm source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Autochthon Wurm source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err("Autochthon Wurm module must contain only private imports and one direct-literal definition".to_owned());
    }
    let mut imports = 0;
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
                if !safe_use_tree(&item.tree) {
                    return Err("Autochthon Wurm source contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                definitions += 1;
                if !function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) || function.sig.ident != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None)
                        if literal.path.segments.last().is_some_and(|segment| segment.ident == "CardDefinition")
                            && literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err("Autochthon Wurm must expose one crate-private direct-literal CardDefinition".to_owned());
                }
            }
            _ => {
                return Err(
                    "Autochthon Wurm source contains an executable or unsupported item".to_owned(),
                );
            }
        }
    }
    if imports != 2 || definitions != 1 {
        return Err(
            "Autochthon Wurm imports or definition count differs from the source contract"
                .to_owned(),
        );
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
            "BTreeSet" | "CardDefinition" | "CardType" | "Color" | "Keyword" | "ManaCost"
        ),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["None"])
                || path_is(&segments, &["Color", "Green"])
                || path_is(&segments, &["Color", "White"])
                || path_is(&segments, &["CardType", "Creature"])
                || path_is(&segments, &["Keyword", "Convoke"])
                || path_is(&segments, &["Keyword", "Trample"])
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (path_is(&segments, &["ManaCost", "with_colors"])
                || path_is(&segments, &["BTreeSet", "from"])
                || path_is(&segments, &["BTreeSet", "new"])
                || path_is(&segments, &["Some"]))
                && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
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

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definition = candidate_autochthon_wurm::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::NoTrample => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Trample),
        Mutation::HigherCost => {
            definition.mana_cost = ManaCost::with_colors(
                11,
                [
                    Color::Green,
                    Color::Green,
                    Color::Green,
                    Color::White,
                    Color::White,
                ],
            )
        }
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Autochthon Wurm definition missing during game construction")? = definition;
    Game::new(definitions, 2).map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = deck(&[
        (BIRD_ID, 4),
        ("RAV-SELESNYA-EVANGEL", 4),
        ("RAV-TRANSLUMINANT", 4),
        ("RAV-SCATTER-THE-SEEDS", 4),
        (CARD_ID, 4),
        ("RAV-FOREST", 32),
        ("RAV-PLAINS", 8),
    ]);
    let opponent = deck(&[
        (BIRD_ID, 4),
        (CAREGIVER_ID, 4),
        ("RAV-FOREST", 26),
        ("RAV-PLAINS", 26),
    ]);
    for (label, list) in [
        ("Selesnya Autochthon pilot", &pilot),
        ("Bird and Caregiver blocker opponent", &opponent),
    ] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("{label} fixture must contain exactly 60 cards"));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(AutochthonPilotPolicy::new()),
        Box::new(BlockerPolicy::new()),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        spell: None,
        spell_zone: None,
        paid_colors: vec![],
        convokers: vec![],
        development_spells: vec![],
        candidate_power: None,
        candidate_toughness: None,
        candidate_has_convoke: false,
        candidate_has_trample: false,
        blocker_casts: vec![],
        blocker_resolved: vec![],
        blocker_entered: vec![],
        blocker_definitions: vec![],
        blockers: vec![],
        blocker_zones: vec![],
        blocker_assigned: vec![],
        damage_to_blockers: vec![],
        trample_damage: vec![],
        combat_order: vec![],
        combat_order_index: None,
        blocker_sbas: vec![],
        blocker_graves: vec![],
        resolved: false,
        opponent_life: None,
    };
    while !game.is_game_over() {
        if game.turn > 60 || captured.accepted >= 12_000 {
            let view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            return Err(format!(
                "Autochthon Wurm PvP exceeded bound at turn {} (wurm={:?}, convoke={}, blockers={:?}, battlefield={:?}, hand={:?}, pool={:?}, opponent life={})",
                game.turn,
                captured.spell,
                captured.convokers.len(),
                captured.blockers,
                view.own_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.mana_pool,
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Autochthon policy seat missing")?;
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
        let requested_action = format!("{action:?}");
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("Autochthon Wurm move rejected on turn {} at {:?} ({requested_action}): {error}", view.turn, view.step))?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], before, &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.spell_zone = captured.spell.and_then(|spell| game.zone_of(spell));
    captured.opponent_life = game.player(PlayerId(1)).ok().map(|player| player.life);
    Ok(captured)
}

fn observe(
    game: &Game,
    events: &[GameEvent],
    event_offset: usize,
    captured: &mut Captured,
) -> Result<(), String> {
    let mut pending_convoke = Vec::new();
    for (local_index, event) in events.iter().enumerate() {
        let absolute_index = event_offset + local_index;
        match event {
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution,
            } => {
                let tapped = game
                    .object(*creature)
                    .map_err(|error| error.to_string())?
                    .tapped;
                let definition = game
                    .card_definition(*creature)
                    .map(|card| card.id)
                    .or_else(|_| {
                        game.object(*creature)
                            .ok()
                            .and_then(|object| object.token.as_ref().map(|token| token.name))
                            .ok_or_else(|| {
                                "Convoke contributor has no card or token identity".to_owned()
                            })
                    })?;
                pending_convoke.push((
                    *creature,
                    *contribution,
                    tapped,
                    definition,
                    absolute_index,
                ));
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.spell = Some(*card);
                captured.paid_colors = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.spell = Some(*card);
                captured.convokers = pending_convoke.clone();
                pending_convoke.clear();
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } => {
                if let Ok(definition) = game.card_definition(*card) {
                    captured.development_spells.push((*card, definition.id));
                }
                pending_convoke.clear();
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| matches!(definition.id, BIRD_ID | CAREGIVER_ID)) =>
            {
                captured.blocker_casts.push(*card);
                let definition = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id;
                captured.blocker_definitions.push((*card, definition));
            }
            GameEvent::SpellResolved { card } if captured.blocker_casts.contains(card) => {
                captured.blocker_resolved.push(*card)
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.spell => {
                captured.resolved = true
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.spell => {
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                captured.candidate_power = characteristics.power;
                captured.candidate_toughness = characteristics.toughness;
                captured.candidate_has_convoke =
                    characteristics.keywords.contains(&Keyword::Convoke);
                captured.candidate_has_trample =
                    characteristics.keywords.contains(&Keyword::Trample);
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if captured.blocker_casts.contains(card) => captured.blocker_entered.push(*card),
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if captured.spell.is_some_and(|spell| {
                assignments.iter().any(|(attacker, _)| *attacker == spell)
            }) =>
            {
                captured.blocker_assigned = assignments
                    .iter()
                    .copied()
                    .filter(|(attacker, _)| Some(*attacker) == captured.spell)
                    .collect();
                captured.blockers = captured
                    .blocker_assigned
                    .iter()
                    .map(|(_, blocker)| *blocker)
                    .collect();
            }
            GameEvent::CombatDamageOrderChosen {
                player: PlayerId(0),
                attacker,
                blockers,
            } if Some(*attacker) == captured.spell => {
                captured.combat_order = blockers.clone();
                captured.combat_order_index = Some(absolute_index);
            }
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if Some(*source) == captured.spell => {
                captured.damage_to_blockers.push((*permanent, *amount))
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == captured.spell => captured.trample_damage.push(*amount),
            GameEvent::StateBasedAction {
                card,
                reason: "creature has lethal damage",
            } if captured.blockers.contains(card) => captured.blocker_sbas.push(*card),
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if captured.blockers.contains(card) => {
                captured.blocker_graves.push(*card);
                captured.blocker_zones.push(Zone::Graveyard);
            }
            _ => {}
        }
    }
    Ok(())
}

struct AutochthonPilotPolicy {
    candidate: Option<ObjectId>,
}
impl AutochthonPilotPolicy {
    fn new() -> Self {
        Self { candidate: None }
    }
}
impl CodePolicy for AutochthonPilotPolicy {
    fn id(&self) -> &'static str {
        "rav.autochthon-wurm-convoke-multiblock.v1"
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind == DecisionKind::CombatDamageOrder {
            let ids: Vec<_> = decision.candidates.iter().map(|card| card.id).collect();
            if ids.len() >= 2 {
                return Some(PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::Objects(ids),
                });
            }
        }
        None
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(0)
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| Some(card.id) == self.candidate && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.player != PlayerId(0)
            || view.decision_player != PlayerId(0)
            || view.priority != PlayerId(0)
            || view.active_player != PlayerId(0)
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let has_white = view.own_battlefield.iter().any(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            });
            let needs_white = !has_white
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some("RAV-SELESNYA-EVANGEL"));
            let preferred = if needs_white {
                "RAV-PLAINS"
            } else {
                "RAV-FOREST"
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(preferred))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.candidate.is_none() {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                && view.mana_pool.amount(Color::Green) >= 6
                && view.mana_pool.total() >= 6
                && let Some(convoke) = full_wurm_convoke(view)
            {
                self.candidate = Some(card.id);
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke,
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Green; 5],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-BIRDS-OF-PARADISE"))
                && view.mana_pool.amount(Color::Green) >= 1
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-TRANSLUMINANT"))
                && can_pay_colored_generic(view, 1, 0, 1)
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-SELESNYA-EVANGEL"))
                && view.mana_pool.amount(Color::Green) >= 1
                && view.mana_pool.amount(Color::White) >= 1
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
                && can_pay_colored_generic(view, 2, 0, 3)
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            let creature_count = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature) && !card.tapped)
                .count();
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
                && creature_count >= 9
                && view.mana_pool.amount(Color::Green) < 6
            {
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    !card.tapped
                        && card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
                && view.mana_pool.amount(Color::Green) == 0
                && view.hand.iter().any(|card| {
                    matches!(
                        card.definition,
                        Some(
                            "RAV-BIRDS-OF-PARADISE" | "RAV-TRANSLUMINANT" | "RAV-SELESNYA-EVANGEL"
                        )
                    )
                })
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            }) && view
                .hand
                .iter()
                .any(|card| card.definition == Some("RAV-TRANSLUMINANT"))
                && !can_pay_colored_generic(view, 1, 0, 1)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            }) && view
                .hand
                .iter()
                .any(|card| card.definition == Some("RAV-SELESNYA-EVANGEL"))
                && view.mana_pool.amount(Color::Green) > 0
                && view.mana_pool.amount(Color::White) == 0
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            }) && view
                .hand
                .iter()
                .any(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
                && !can_pay_colored_generic(view, 2, 0, 3)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn can_pay_colored_generic(view: &GameView, green: u8, white: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= green
        && view.mana_pool.amount(Color::White) >= white
        && view
            .mana_pool
            .total()
            .saturating_sub(green.saturating_add(white))
            >= generic
}

fn full_wurm_convoke(view: &GameView) -> Option<Vec<ConvokePayment>> {
    if view.mana_pool.amount(Color::Green) < 6 || view.mana_pool.total() < 6 {
        return None;
    }
    let creatures: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
        .collect();
    let green: Vec<_> = creatures
        .iter()
        .filter(|card| card.colors.contains(&Color::Green) && !card.colors.contains(&Color::White))
        .take(2)
        .collect();
    if green.len() != 2 {
        return None;
    }
    let used: HashSet<_> = green.iter().map(|card| card.id).collect();
    let white: Vec<_> = creatures
        .iter()
        .filter(|card| !used.contains(&card.id) && card.colors.contains(&Color::White))
        .take(2)
        .collect();
    if white.len() != 2 {
        return None;
    }
    let used: HashSet<_> = green
        .iter()
        .chain(white.iter())
        .map(|card| card.id)
        .collect();
    let generic: Vec<_> = creatures
        .iter()
        .filter(|card| !used.contains(&card.id))
        .take(5)
        .collect();
    if generic.len() != 5 {
        return None;
    }
    Some(
        green
            .into_iter()
            .map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Color(Color::Green),
            })
            .chain(white.into_iter().map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Color(Color::White),
            }))
            .chain(generic.into_iter().map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Generic,
            }))
            .collect(),
    )
}

struct BlockerPolicy {
    cast_bird: bool,
    cast_caregiver: bool,
}
impl BlockerPolicy {
    fn new() -> Self {
        Self {
            cast_bird: false,
            cast_caregiver: false,
        }
    }
}
impl CodePolicy for BlockerPolicy {
    fn id(&self) -> &'static str {
        "rav.autochthon-wurm-two-blockers.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != PlayerId(1) || view.decision_player != PlayerId(1) {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(1)
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != PlayerId(1)
            && !view.blockers_declared
        {
            let attacker = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let blockers: Vec<_> = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_block)
                .take(2)
                .collect();
            return PolicyAction::DeclareBlockers {
                assignments: match (attacker, blockers.len()) {
                    (Some(attacker), 2) => blockers
                        .iter()
                        .map(|blocker| CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        })
                        .collect(),
                    _ => vec![],
                },
            };
        }
        if view.priority != PlayerId(1)
            || view.active_player != PlayerId(1)
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some("RAV-PLAINS"))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.cast_bird
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BIRD_ID))
        {
            if view.mana_pool.amount(Color::Green) > 0 {
                self.cast_bird = true;
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
        }
        if !self.cast_caregiver
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CAREGIVER_ID))
        {
            if view.mana_pool.amount(Color::White) > 0 {
                self.cast_caregiver = true;
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-PLAINS"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_AUTOCHTHON_WURM_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_AUTOCHTHON_WURM_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Autochthon Wurm Convoke/multi-block Trample full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 || captured.accepted == 0 {
        return Err("Autochthon Wurm duel did not end in a clean P0 terminal win".to_owned());
    }
    let spell = captured
        .spell
        .ok_or("Autochthon Wurm was not cast from the shuffled pilot deck")?;
    if captured.spell_zone != Some(Zone::Battlefield) || !captured.resolved {
        return Err("Autochthon Wurm did not resolve and remain on the battlefield".to_owned());
    }
    if candidate_autochthon_wurm::definition() != expected_definition() {
        return Err(
            "Autochthon Wurm candidate or registry differs from exact RAV #191 identity".to_owned(),
        );
    }
    if (captured.candidate_power, captured.candidate_toughness) != (Some(9), Some(14))
        || !captured.candidate_has_convoke
        || !captured.candidate_has_trample
    {
        return Err("live Autochthon Wurm characteristics or keywords differ from the 9/14 Convoke/Trample source contract".to_owned());
    }
    if captured.convokers.len() != 9
        || captured
            .convokers
            .iter()
            .map(|(id, _, _, _, _)| *id)
            .collect::<BTreeSet<_>>()
            .len()
            != 9
        || captured
            .convokers
            .iter()
            .any(|(_, _, tapped, _, _)| !tapped)
    {
        return Err(format!(
            "Autochthon Wurm must tap nine distinct creatures for Convoke: {:?}",
            captured.convokers
        ));
    }
    if !captured
        .development_spells
        .iter()
        .any(|(_, definition)| *definition == "RAV-SCATTER-THE-SEEDS")
    {
        return Err(
            "the shuffled pilot deck must cast Scatter the Seeds to create Convoke bodies"
                .to_owned(),
        );
    }
    if captured
        .convokers
        .iter()
        .filter(|(_, color, _, _, _)| *color == Some(Color::Green))
        .count()
        != 2
        || captured
            .convokers
            .iter()
            .filter(|(_, color, _, _, _)| *color == Some(Color::White))
            .count()
            != 2
        || captured
            .convokers
            .iter()
            .filter(|(_, color, _, _, _)| color.is_none())
            .count()
            != 5
        || captured.paid_colors != vec![Color::Green; 6]
    {
        return Err(format!(
            "exact two green/two white/five generic Convoke and six pool mana must pay Autochthon Wurm: pool={:?}, convoke={:?}",
            captured.paid_colors, captured.convokers
        ));
    }
    if captured.blockers.len() != 2
        || captured.blocker_assigned.len() != 2
        || captured.blockers.iter().collect::<BTreeSet<_>>().len() != 2
    {
        return Err(format!(
            "Autochthon Wurm must be blocked by two distinct creatures: {:?}",
            captured.blocker_assigned
        ));
    }
    if captured.blockers.len() != captured.blocker_resolved.len()
        || captured.blockers.len() != captured.blocker_entered.len()
        || !captured.blockers.iter().all(|blocker| {
            captured.blocker_casts.contains(blocker)
                && captured.blocker_resolved.contains(blocker)
                && captured.blocker_entered.contains(blocker)
        })
    {
        return Err(
            "both exact blockers must be cast and resolve from the shuffled opponent deck"
                .to_owned(),
        );
    }
    if captured.combat_order != captured.blockers || captured.combat_order_index.is_none() {
        return Err(format!(
            "attacking player must submit exact order of both Wurm blockers: {:?} vs {:?}",
            captured.combat_order, captured.blockers
        ));
    }
    let mut damage = captured.damage_to_blockers.clone();
    damage.sort_unstable_by_key(|(blocker, _)| *blocker);
    let mut blockers_sorted = captured.blockers.clone();
    blockers_sorted.sort_unstable();
    if damage
        != blockers_sorted
            .iter()
            .map(|blocker| (*blocker, 1))
            .collect::<Vec<_>>()
        || captured.blocker_sbas.iter().collect::<BTreeSet<_>>()
            != blockers_sorted.iter().collect::<BTreeSet<_>>()
        || captured.blocker_graves.iter().collect::<BTreeSet<_>>()
            != blockers_sorted.iter().collect::<BTreeSet<_>>()
        || captured.blocker_zones != vec![Zone::Graveyard; 2]
    {
        return Err(format!(
            "each 1/1 blocker must take lethal damage, die by SBA, and reach its graveyard: damage={damage:?}, SBA={:?}, graves={:?}, zones={:?}",
            captured.blocker_sbas, captured.blocker_graves, captured.blocker_zones
        ));
    }
    if captured.trample_damage.first() != Some(&7)
        || captured
            .trample_damage
            .iter()
            .map(|amount| i64::from(*amount))
            .sum::<i64>()
            < 20
        || !captured.opponent_life.is_some_and(|life| life <= 0)
    {
        return Err(format!(
            "Autochthon Wurm must trample for exactly seven after the two blockers and continue to terminal lethal: damage={:?}, life={:?}",
            captured.trample_damage, captured.opponent_life
        ));
    }
    let events = &captured.events;
    let convoke_indices: Vec<_> = captured
        .convokers
        .iter()
        .map(|(_, _, _, _, index)| *index)
        .collect();
    if convoke_indices.iter().any(|index| {
        !matches!(
            events.get(*index),
            Some(GameEvent::ConvokeUsed {
                player: PlayerId(0),
                ..
            })
        )
    }) {
        return Err(
            "captured Autochthon contributors do not map to typed ConvokeUsed receipts".to_owned(),
        );
    }
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Autochthon Wurm SpellCast receipt missing")?;
    let mana_paid = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors == &vec![Color::Green; 6])).ok_or("Autochthon Wurm six-green SpellManaPaid receipt missing")?;
    let resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Autochthon Wurm SpellResolved receipt missing")?;
    let entered = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == spell)).ok_or("Autochthon Wurm battlefield entry receipt missing")?;
    let attack = first_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&spell))).ok_or("Autochthon Wurm did not attack")?;
    let blockers = unique_index(events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if captured.blocker_assigned.iter().all(|(_, blocker)| assignments.contains(&(spell, *blocker))))).ok_or("both selected creatures did not block Autochthon Wurm")?;
    let order = captured
        .combat_order_index
        .ok_or("attacker damage order event missing")?;
    if !matches!(events.get(order), Some(GameEvent::CombatDamageOrderChosen { player: PlayerId(0), attacker, blockers: ordered }) if *attacker == spell && ordered == &captured.blockers)
    {
        return Err("public typed combat damage-order receipt differs from selection".to_owned());
    }
    let terminal = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    let scatter = captured
        .development_spells
        .iter()
        .find(|(_, definition)| *definition == "RAV-SCATTER-THE-SEEDS")
        .map(|(card, _)| *card)
        .ok_or("Scatter the Seeds development cast missing")?;
    let scatter_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == scatter)).ok_or("Scatter the Seeds SpellCast receipt missing")?;
    let scatter_resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == scatter),
    )
    .ok_or("Scatter the Seeds SpellResolved receipt missing")?;
    if !(scatter_cast < scatter_resolved && scatter_resolved < cast) {
        return Err(
            "Scatter the Seeds must resolve from the shuffled deck before Autochthon Wurm"
                .to_owned(),
        );
    }
    if convoke_indices.iter().any(|index| *index >= mana_paid)
        || !(mana_paid < cast
            && cast < resolved
            && resolved < entered
            && entered < attack
            && attack < blockers
            && blockers < order
            && order < terminal)
    {
        return Err(
            "Autochthon Wurm Convoke/cast/combat-order/terminal event order invalid".to_owned(),
        );
    }
    for blocker in &captured.blockers {
        let blocker_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == *blocker)).ok_or("shuffled opponent blocker SpellCast receipt missing")?;
        let blocker_resolved = unique_index(
            events,
            |event| matches!(event, GameEvent::SpellResolved { card } if *card == *blocker),
        )
        .ok_or("opponent blocker SpellResolved receipt missing")?;
        let blocker_entered = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == *blocker)).ok_or("opponent blocker battlefield receipt missing")?;
        if !(blocker_cast < blocker_resolved
            && blocker_resolved < blocker_entered
            && blocker_entered < blockers)
        {
            return Err(
                "exact two blockers must be cast and enter before Autochthon Wurm combat"
                    .to_owned(),
            );
        }
    }
    let mut selected: BTreeSet<usize> = convoke_indices.into_iter().collect();
    selected.extend([
        mana_paid, cast, resolved, entered, attack, blockers, order, terminal,
    ]);
    selected.extend([scatter_cast, scatter_resolved]);
    for (index, event) in events.iter().enumerate() {
        if matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == spell && captured.blockers.contains(permanent))
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == spell)
            || matches!(event, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if captured.blockers.contains(card))
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if captured.blockers.contains(card))
        {
            selected.insert(index);
        }
    }
    for (card, definition) in &captured.development_spells {
        if matches!(
            *definition,
            BIRD_ID | "RAV-SELESNYA-EVANGEL" | "RAV-TRANSLUMINANT" | "RAV-SCATTER-THE-SEEDS"
        ) {
            for (index, event) in events.iter().enumerate() {
                if matches!(event, GameEvent::SpellCast { player: PlayerId(0), card: candidate } if candidate == card)
                    || matches!(event, GameEvent::SpellResolved { card: candidate } if candidate == card)
                    || matches!(event, GameEvent::CardMoved { card: candidate, to: Zone::Battlefield } if candidate == card)
                {
                    selected.insert(index);
                }
            }
        }
    }
    for (creature, _, _, definition, _) in &captured.convokers {
        if *definition == "Saproling" {
            let token_created = unique_index(events, |event| matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if *token == *creature))
                .ok_or("a Convoke Saproling lacks its shuffled-deck token-creation receipt")?;
            if token_created >= cast {
                return Err("Convoke token must be created from the pilot's shuffled spell before Autochthon Wurm is cast".to_owned());
            }
            selected.insert(token_created);
        }
    }
    for blocker in &captured.blockers {
        for (index, event) in events.iter().enumerate() {
            if matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if card == blocker)
                || matches!(event, GameEvent::SpellResolved { card } if card == blocker)
                || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if card == blocker)
            {
                selected.insert(index);
            }
        }
    }
    let projected = project_selected(events, &selected)?;
    Ok(json!({
        "id": "rav-autochthon-wurm-nine-convoke-multi-block-trample-terminal",
        "seed": seed,
        "decks": ["rav-autochthon-wurm-selesnya-60", "rav-autochthon-wurm-blockers-60"],
        "policies": ["rav.autochthon-wurm-convoke-multiblock.v1", "rav.autochthon-wurm-two-blockers.v1"],
        "terminal": "Winner(PlayerId(0))", "winner_seat": captured.winner, "turns": captured.turns,
        "accepted_moves": captured.accepted, "rejected_moves": captured.rejected,
        "spell_object": spell.0, "spell_zone": format!("{:?}", captured.spell_zone),
        "mana_paid": captured.paid_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "development_spells": captured.development_spells.iter().map(|(object, definition)| json!({"object": object.0, "definition": definition})).collect::<Vec<_>>(),
        "convoke_payments": captured.convokers.iter().map(|(creature, contribution, tapped, definition, _)| json!({"creature": creature.0, "definition": definition, "contribution": contribution.map(|color| format!("{color:?}")).unwrap_or_else(|| "Generic".to_owned()), "tapped": tapped})).collect::<Vec<_>>(),
        "live_power_toughness": [captured.candidate_power, captured.candidate_toughness],
        "live_convoke": captured.candidate_has_convoke, "live_trample": captured.candidate_has_trample,
        "blockers": captured.blockers.iter().map(|blocker| json!({"object": blocker.0, "definition": captured.blocker_definitions.iter().find(|(object, _)| object == blocker).map(|(_, id)| *id)})).collect::<Vec<_>>(),
        "ordered_blockers": captured.combat_order.iter().map(|object| object.0).collect::<Vec<_>>(),
        "damage_to_blockers": captured.damage_to_blockers.iter().map(|(object, amount)| json!({"object": object.0, "amount": amount})).collect::<Vec<_>>(),
        "trample_damage_to_opponent": captured.trample_damage,
        "blocker_zones_after_combat": captured.blocker_zones.iter().map(|zone| format!("{zone:?}")).collect::<Vec<_>>(),
        "opponent_life_after_match": captured.opponent_life,
        "ordered_event_sequence": selected.iter().copied().collect::<Vec<_>>(), "events": projected, "passed": true
    }))
}

fn project_selected(
    events: &[GameEvent],
    selected: &BTreeSet<usize>,
) -> Result<Vec<Value>, String> {
    let projected = project_events(events);
    let mut output = Vec::new();
    for index in selected {
        let mut value = serde_json::to_value(
            projected
                .get(*index)
                .ok_or("projected event index missing")?,
        )
        .map_err(|error| error.to_string())?;
        match events.get(*index).ok_or("raw event index missing")? {
            GameEvent::ConvokeUsed {
                player,
                creature,
                contribution,
            } => {
                value["facts"] = json!({"seats": [player.0], "objects": [creature.0], "convoke_contribution": contribution.map_or_else(|| "generic".to_owned(), |color| format!("{color:?}"))})
            }
            GameEvent::CombatDamageOrderChosen {
                player,
                attacker,
                blockers,
            } => {
                value["facts"] = json!({"seats": [player.0], "objects": std::iter::once(attacker.0).chain(blockers.iter().map(|object| object.0)).collect::<Vec<_>>(), "ordered_blockers": blockers.iter().map(|object| object.0).collect::<Vec<_>>() })
            }
            GameEvent::TokenCreated { player, token } => {
                value["facts"] = json!({"seats": [player.0], "objects": [token.0]});
            }
            _ => {}
        }
        output.push(value);
    }
    Ok(output)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        .is_err())
}
fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}
fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut indices = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = indices.next()?;
    indices.next().is_none().then_some(first)
}
fn first_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "credited": false, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated"}})
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
