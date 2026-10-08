//! Full-game RAV Vigor Mortis graveyard-return and spent-green authority.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, CounterKind, DeckEntry, DeckList,
    Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VIGOR-MORTIS";
const WOLF_ID: &str = "RAV-WATCHWOLF";
const MAX_SEED: u64 = 64;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/vigor_mortis.rs"]
mod candidate_vigor_mortis;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Vigor Mortis result: {error}");
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
            eprintln!("Vigor Mortis PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GenericSpend {
    Green,
    Black,
}

impl GenericSpend {
    const fn color(self) -> Color {
        match self {
            Self::Green => Color::Green,
            Self::Black => Color::Black,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Black => "black",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    WrongCost,
    RemoveReturnEffect,
    WrongSpentColor,
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_vigor_mortis::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Black, Color::Black]);
        }
        Mutation::RemoveReturnEffect => definition.effects.clear(),
        Mutation::WrongSpentColor => {
            definition.effects = vec![
                cardbench_magic_engine::Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                    color: Color::Blue,
                },
            ];
        }
    }
    definition
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/vigor_mortis.rs");
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read Vigor Mortis candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Vigor Mortis candidate module is not valid Rust".to_owned())?;
    let functions: BTreeSet<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    let required = [
        "id:\"RAV-VIGOR-MORTIS\"",
        "name:\"VigorMortis\"",
        "mana_cost:ManaCost::with_colors(2,[Color::Black,Color::Black])",
        "colors:BTreeSet::from([Color::Black])",
        "card_types:BTreeSet::from([CardType::Sorcery])",
        "Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent{color:Color::Green,}",
    ];
    let direct_constructor = file.items.iter().any(|item| {
        matches!(item, syn::Item::Fn(function)
            if function.sig.ident == "definition"
                && matches!(&function.vis, syn::Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                && function.block.stmts.len() == 1
                && matches!(&function.block.stmts[0], syn::Stmt::Expr(syn::Expr::Struct(value), None)
                    if value.path.is_ident("CardDefinition") && value.rest.is_none()))
    });
    if functions != BTreeSet::from(["definition".to_owned()])
        || !direct_constructor
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Vigor Mortis candidate definition is outside its literal source contract".into(),
        );
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Vigor Mortis",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "return-target-creature-card-from-graveyard-to-battlefield",
            "spent-green-plus-one-plus-one-counter",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![cardbench_magic_engine::Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
            color: Color::Green,
        }],
    }
}

fn validate_candidate_runtime() -> Result<(), String> {
    let candidate = candidate_vigor_mortis::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Vigor Mortis is missing from the RAV definition registry")?;
    if candidate != expected_definition() || candidate != registered {
        return Err(
            "Vigor Mortis candidate definition is not delegated to the exact RAV card contract"
                .into(),
        );
    }
    Ok(())
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 111 && card.name == "Vigor Mortis")
    {
        return Err("RAV catalog denominator or Vigor Mortis identity differs".into());
    }

    let (green_seed, green_capture) = find_trace(GenericSpend::Green)?;
    let green_trace = validate_trace(&green_capture, green_seed, GenericSpend::Green)?;
    let green_replay = run_match(green_seed, GenericSpend::Green, Mutation::None)?;
    if green_capture.events != green_replay.events || green_capture.winner != green_replay.winner {
        return Err("Vigor Mortis green-spent duel replay differs".into());
    }

    let (black_seed, black_capture) = find_trace(GenericSpend::Black)?;
    let black_trace = validate_trace(&black_capture, black_seed, GenericSpend::Black)?;
    let black_replay = run_match(black_seed, GenericSpend::Black, Mutation::None)?;
    if black_capture.events != black_replay.events || black_capture.winner != black_replay.winner {
        return Err("Vigor Mortis non-green control duel replay differs".into());
    }

    let mutations = json!({
        "printed_cost_mutation_detected_in_game": mutation_detected(green_seed, Mutation::WrongCost),
        "return_effect_removal_detected_in_game": mutation_detected(green_seed, Mutation::RemoveReturnEffect),
        "spent_color_mutation_detected_in_game": mutation_detected(green_seed, Mutation::WrongSpentColor),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cost_mutation_detected_in_game",
        "return_effect_removal_detected_in_game",
        "spent_color_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Vigor Mortis semantic mutation escaped: {key}"));
        }
    }

    let definitions = card_definitions();
    let breakdown: Vec<_> = catalog
        .iter()
        .map(|card| {
            let id = definitions
                .iter()
                .find(|definition| definition.name == card.name)
                .map(|definition| definition.id)
                .unwrap_or("");
            json!({
                "collector_number": card.collector_number,
                "name": card.name,
                "definition_id": id,
                "engine_status": if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
                "trace_status": if id == CARD_ID { "passed" } else { "not_qualified" },
                "qualification": if id == CARD_ID { "unreviewed_diagnostic" } else { "not_evaluated" },
                "credited": false
            })
        })
        .collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 111, "definition_id": CARD_ID, "name": "Vigor Mortis"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV Vigor Mortis #111 costs {2}{B}{B}; return target creature card from your graveyard to the battlefield; if Green mana was spent to cast it, put a +1/+1 counter on that creature.",
            "checked_sections": ["400 Zones", "601 Casting spells", "608 Resolving spells", "120 Counters", "704 State-based actions", "510 Combat damage"],
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "mutation_sensitivity": mutations,
        "traces": [green_trace, black_trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReturnObservation {
    spell: ObjectId,
    target: ObjectId,
    payment: Vec<Color>,
    target_was_in_own_graveyard: bool,
    paid_index: usize,
    cast_index: usize,
    enter_index: Option<usize>,
    counter_index: Option<usize>,
    resolved_index: Option<usize>,
    returned_power: Option<i32>,
    returned_toughness: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    returns: Vec<ReturnObservation>,
    life: [i64; 2],
}

fn find_trace(spend: GenericSpend) -> Result<(u64, Captured), String> {
    let env_key = match spend {
        GenericSpend::Green => "CARDBENCH_VIGOR_MORTIS_GREEN_SEED",
        GenericSpend::Black => "CARDBENCH_VIGOR_MORTIS_BLACK_SEED",
    };
    if let Ok(raw) = std::env::var(env_key) {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| format!("{env_key} must be an unsigned integer"))?;
        let capture = run_match(seed, spend, Mutation::None)?;
        validate_trace(&capture, seed, spend)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, spend, Mutation::None)
            && validate_trace(&capture, seed, spend).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Vigor Mortis {} trace found for seeds 0..={MAX_SEED}",
        spend.label()
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, GenericSpend::Green, mutation) {
        Ok(capture) => validate_trace(&capture, seed, GenericSpend::Green).is_err(),
        Err(_) => true,
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let target = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Vigor Mortis is missing from the RAV definition registry")?;
    *target = candidate_definition(mutation);
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        vec![],
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn run_match(seed: u64, spend: GenericSpend, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: WOLF_ID.into(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 8,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WOLF_ID.into(),
                count: 8,
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
    if [p0_deck.mainboard.as_slice(), p1_deck.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| usize::from(entry.count))
                .sum::<usize>()
                != 60
        })
    {
        return Err("Vigor Mortis PvP decks must each contain exactly 60 cards".into());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies = [
        VigorPolicy::new(PlayerId(0), spend),
        VigorPolicy::new(PlayerId(1), spend),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut paid_by_spell = BTreeMap::<ObjectId, (Vec<Color>, usize)>::new();
    let mut returns = Vec::<ReturnObservation>::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            let p0 = game
                .view_for_player(PlayerId(0))
                .map(|view| {
                    (
                        view.own_life,
                        view.own_battlefield
                            .iter()
                            .map(|card| (card.id, card.definition, card.tapped, card.can_attack))
                            .collect::<Vec<_>>(),
                        view.hand
                            .iter()
                            .map(|card| card.definition)
                            .collect::<Vec<_>>(),
                    )
                })
                .ok();
            let p1 = game
                .view_for_player(PlayerId(1))
                .map(|view| {
                    (
                        view.own_life,
                        view.own_battlefield
                            .iter()
                            .map(|card| (card.id, card.definition, card.tapped, card.can_attack))
                            .collect::<Vec<_>>(),
                        view.hand
                            .iter()
                            .map(|card| card.definition)
                            .collect::<Vec<_>>(),
                    )
                })
                .ok();
            let tail = game
                .event_log
                .iter()
                .rev()
                .take(24)
                .rev()
                .map(|event| format!("{event:?}"))
                .collect::<Vec<_>>();
            let casts = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::SpellCast { player, card } => Some((
                        player.0,
                        game.card_definition(*card)
                            .ok()
                            .map(|definition| definition.id),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            return Err(format!(
                "Vigor Mortis duel exceeded its bounded move count at turn {}; p0={p0:?}, p1={p1:?}, casts={casts:?}, recent={tail:?}",
                game.turn,
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Vigor Mortis policy seat missing")?;
        let action = if view.pending_decision.is_some() {
            policy
                .propose_pending_decision(&view)
                .ok_or("Vigor Mortis policy could not answer a mandatory decision")?
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
        let action_description = format!("{action:?}");
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Vigor Mortis policy move rejected at turn {} {:?}, action={action_description}, mana={:?}: {error}",
                    view.turn, view.step, view.mana_pool,
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Vigor Mortis invariant failed: {error}"))?;

        for (offset, event) in game.event_log[event_start..].iter().enumerate() {
            let index = event_start + offset;
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    paid_by_spell.insert(*card, (colors.clone(), index));
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    let stack = game
                        .stack
                        .iter()
                        .find(|item| item.card == *card)
                        .ok_or("Vigor Mortis spell cast has no stack object")?;
                    let Some(Target::Permanent(target)) = stack.targets.first().copied() else {
                        return Err(
                            "Vigor Mortis stack object did not retain its creature target".into(),
                        );
                    };
                    let (payment, paid_index) = paid_by_spell
                        .get(card)
                        .cloned()
                        .ok_or("Vigor Mortis payment receipt is missing")?;
                    returns.push(ReturnObservation {
                        spell: *card,
                        target,
                        payment,
                        target_was_in_own_graveyard: game.zone_of(target) == Some(Zone::Graveyard)
                            && game
                                .player(PlayerId(0))
                                .is_ok_and(|seat| seat.graveyard.contains(&target)),
                        paid_index,
                        cast_index: index,
                        enter_index: None,
                        counter_index: None,
                        resolved_index: None,
                        returned_power: None,
                        returned_toughness: None,
                    });
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } => {
                    if let Some(observation) = returns
                        .iter_mut()
                        .find(|entry| entry.target == *card && entry.enter_index.is_none())
                    {
                        observation.enter_index = Some(index);
                        if let Ok(characteristics) = game.characteristics(*card) {
                            observation.returned_power = characteristics.power;
                            observation.returned_toughness = characteristics.toughness;
                        }
                    }
                }
                GameEvent::CounterPlaced {
                    source,
                    card,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 1,
                } if let Some(observation) = returns
                    .iter_mut()
                    .find(|entry| entry.spell == *source && entry.target == *card) =>
                {
                    observation.counter_index = Some(index);
                    if let Ok(characteristics) = game.characteristics(*card) {
                        observation.returned_power = characteristics.power;
                        observation.returned_toughness = characteristics.toughness;
                    }
                }
                GameEvent::SpellResolved { card } => {
                    if let Some(observation) = returns
                        .iter_mut()
                        .find(|entry| entry.spell == *card && entry.resolved_index.is_none())
                    {
                        observation.resolved_index = Some(index);
                    }
                }
                _ => {}
            }
        }
        for observer in &mut policies {
            observer.observe_events(&game.event_log[event_start..], &game)?;
        }
    }
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        returns,
        life,
    })
}

struct VigorPolicy {
    player: PlayerId,
    spend: GenericSpend,
    own_wolves: BTreeSet<ObjectId>,
    graveyard_wolves: BTreeSet<ObjectId>,
    vigor_cast: bool,
}

impl VigorPolicy {
    fn new(player: PlayerId, spend: GenericSpend) -> Self {
        Self {
            player,
            spend,
            own_wolves: BTreeSet::new(),
            graveyard_wolves: BTreeSet::new(),
            vigor_cast: false,
        }
    }

    fn observe_events(&mut self, events: &[GameEvent], game: &Game) -> Result<(), String> {
        for event in events {
            match event {
                GameEvent::SpellCast { player, card }
                    if *player == self.player
                        && game
                            .card_definition(*card)
                            .is_ok_and(|definition| definition.id == WOLF_ID) =>
                {
                    self.own_wolves.insert(*card);
                }
                GameEvent::SpellCast { player, card }
                    if *player == PlayerId(0)
                        && game
                            .card_definition(*card)
                            .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    self.vigor_cast = true;
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if self.own_wolves.contains(card) => {
                    self.graveyard_wolves.insert(*card);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if self.own_wolves.contains(card) => {
                    self.graveyard_wolves.remove(card);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let count_sources = |color| {
            view.own_battlefield
                .iter()
                .filter(|field| {
                    field.card_types.contains(&CardType::Land) && field.mana_colors.contains(&color)
                })
                .count()
        };
        let preferred = if self.player == PlayerId(0)
            && !self.graveyard_wolves.is_empty()
            && !self.vigor_cast
            && view
                .hand
                .iter()
                .any(|held| held.definition == Some(CARD_ID))
        {
            let needs: Vec<(Color, usize)> = match self.spend {
                GenericSpend::Green => vec![(Color::Black, 2), (Color::Green, 2)],
                GenericSpend::Black => vec![(Color::Black, 4)],
            };
            needs
                .into_iter()
                .find(|(color, needed)| count_sources(*color) < *needed)
                .map(|(color, _)| color)
                .unwrap_or(Color::Black)
        } else if self.player == PlayerId(0) {
            let needs = [(Color::Green, 1), (Color::White, 1)];
            needs
                .into_iter()
                .find(|(color, needed)| count_sources(*color) < *needed)
                .map(|(color, _)| color)
                .unwrap_or(Color::Black)
        } else {
            let needs = [(Color::Green, 1), (Color::White, 1)];
            needs
                .into_iter()
                .find(|(color, needed)| count_sources(*color) < *needed)
                .map(|(color, _)| color)
                .unwrap_or(Color::Green)
        };
        let land = view
            .hand
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land))
            .find(|card| card.mana_colors.contains(&preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })?;
        Some(PolicyAction::PlayLand { card: land.id })
    }

    fn make_cast(
        &self,
        view: &GameView,
        definition: &str,
        cost: &ManaCost,
        target: Option<ObjectId>,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        let generic_color = if definition == CARD_ID {
            self.spend.color()
        } else {
            Color::Black
        };
        let selection = ManaPaymentSelection {
            generic: vec![generic_color; usize::from(cost.generic)],
            hybrid: vec![],
        };
        if let Some(missing) = missing_payment_color(view, cost, &selection) {
            return self.make_mana(view, missing);
        }
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: target
                    .map(|target| vec![Target::Permanent(target)])
                    .unwrap_or_default(),
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: selection,
        })
    }

    fn make_mana(&self, view: &GameView, preferred: Color) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = if land.mana_colors.contains(&preferred) {
            preferred
        } else {
            *land.mana_colors.iter().next()?
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}

impl CodePolicy for VigorPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.vigor-mortis-pilot.v1"
        } else {
            "rav.watchwolf-opponent.v1"
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
                    .filter(|card| card.definition == Some(WOLF_ID) && card.can_attack)
                    .map(|card| card.id)
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
            if self.player != PlayerId(1) {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![],
                };
            }
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(WOLF_ID) && card.can_block);
            let assignment = blocker.and_then(|blocker| {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(WOLF_ID))?;
                Some(CombatBlock {
                    attacker: attacker.id,
                    blocker: blocker.id,
                })
            });
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        let wolf_on_field = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(WOLF_ID));
        let vigor_cost = ManaCost::with_colors(2, [Color::Black, Color::Black]);
        if self.player == PlayerId(0) {
            if !self.vigor_cast
                && let (Some(target), Some(_)) = (
                    self.graveyard_wolves.iter().next().copied(),
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID)),
                )
            {
                return self
                    .make_cast(view, CARD_ID, &vigor_cost, Some(target))
                    .unwrap_or(PolicyAction::PassPriority);
            }
        }
        let wolf_cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        if !wolf_on_field {
            if let Some(action) = self.make_cast(view, WOLF_ID, &wolf_cost, None) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn missing_payment_color(
    view: &GameView,
    cost: &ManaCost,
    selection: &ManaPaymentSelection,
) -> Option<Color> {
    let mut required = BTreeMap::<Color, u8>::new();
    for color in &cost.colored {
        *required.entry(*color).or_default() += 1;
    }
    for color in &selection.generic {
        *required.entry(*color).or_default() += 1;
    }
    for color in Color::MANA_ALL {
        let amount = required.get(&color).copied().unwrap_or(0);
        if view.mana_pool.amount(color) < amount {
            return Some(color);
        }
    }
    None
}

fn validate_trace(capture: &Captured, seed: u64, spend: GenericSpend) -> Result<Value, String> {
    if capture.winner != Some(0)
        || capture.terminal != "Winner(PlayerId(0))"
        || capture.rejected != 0
    {
        return Err(format!(
            "Vigor Mortis {} duel did not reach a clean seat-zero win",
            spend.label()
        ));
    }
    let observation = capture
        .returns
        .first()
        .ok_or("seat zero never cast Vigor Mortis")?;
    if capture.returns.len() != 1 {
        return Err(format!(
            "expected one Vigor Mortis return trace, found {}",
            capture.returns.len()
        ));
    }
    if !observation.target_was_in_own_graveyard {
        return Err(
            "Vigor Mortis target was not a card in its controller's graveyard at cast time".into(),
        );
    }
    if observation.payment.len() != 4
        || observation
            .payment
            .iter()
            .filter(|color| **color == Color::Black)
            .count()
            < 2
        || observation
            .payment
            .iter()
            .filter(|color| **color == spend.color())
            .count()
            < 2
        || (spend == GenericSpend::Green
            && observation
                .payment
                .iter()
                .filter(|color| **color == Color::Green)
                .count()
                != 2)
        || (spend == GenericSpend::Black
            && observation
                .payment
                .iter()
                .any(|color| *color == Color::Green))
    {
        return Err(format!(
            "Vigor Mortis did not retain the required payment allocation: {:?}",
            observation.payment
        ));
    }
    let target_died = event_index(&capture.events, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == observation.target)
    })
    .ok_or("Vigor Mortis target did not first die into the caster's graveyard")?;
    let entered = observation
        .enter_index
        .ok_or("Vigor Mortis target did not return to the battlefield")?;
    let resolved = observation
        .resolved_index
        .ok_or("Vigor Mortis did not resolve")?;
    let counter = observation.counter_index;
    if target_died >= observation.paid_index
        || observation.paid_index >= observation.cast_index
        || observation.cast_index >= entered
        || entered >= resolved
    {
        return Err("Vigor Mortis target death, payment, cast, return, and resolution events are out of order".into());
    }
    match spend {
        GenericSpend::Green
            if counter.is_none()
                || observation.returned_power != Some(4)
                || observation.returned_toughness != Some(4) =>
        {
            return Err("green spent on Vigor Mortis did not add exactly one +1/+1 counter".into());
        }
        GenericSpend::Black
            if counter.is_some()
                || observation.returned_power != Some(3)
                || observation.returned_toughness != Some(3) =>
        {
            return Err("nongreen Vigor Mortis control must return the unmodified 3/3".into());
        }
        _ => {}
    }
    if let Some(counter_index) = counter {
        if !(entered < counter_index && counter_index < resolved) {
            return Err(
                "Vigor Mortis counter receipt is not after return and before resolution".into(),
            );
        }
    }
    let ended = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Vigor Mortis duel has no terminal win event")?;
    if capture.life[0] <= 0 || capture.life[1] > 0 || ended <= resolved {
        return Err("Vigor Mortis duel terminal life or event ordering is inconsistent".into());
    }
    let mut selected = BTreeSet::from([
        target_died,
        observation.paid_index,
        observation.cast_index,
        entered,
        resolved,
        ended,
    ]);
    if let Some(counter_index) = counter {
        selected.insert(counter_index);
    }
    let projected_all = project_events(&capture.events);
    let projected: Vec<ProjectedEvent> = projected_all
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "id": format!("rav-vigor-mortis-{}-spent-control", spend.label()),
        "seed": seed,
        "decks": ["rav_vigor_mortis_returning_watchwolves", "rav_watchwolf_blocking_control"],
        "policies": ["rav.vigor-mortis-pilot.v1", "rav.watchwolf-opponent.v1"],
        "passed": true,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": capture.rejected,
        "life_at_terminal": capture.life,
        "mechanics": {
            "generic_spend_case": spend.label(),
            "spell_object": observation.spell.0,
            "spell_mana_cost": "{2}{B}{B}",
            "payment": observation.payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "target_object": observation.target.0,
            "target_definition": WOLF_ID,
            "target_owner": 0,
            "target_was_in_own_graveyard": observation.target_was_in_own_graveyard,
            "returned_to_battlefield": true,
            "counter_placed": counter.is_some(),
            "counter_amount": if counter.is_some() { 1 } else { 0 },
            "returned_power": observation.returned_power,
            "returned_toughness": observation.returned_toughness
        },
        "event_indices": {"target_death": target_died, "payment": observation.paid_index, "cast": observation.cast_index, "return": entered, "counter": counter, "resolution": resolved, "terminal": ended},
        "events": projected
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
