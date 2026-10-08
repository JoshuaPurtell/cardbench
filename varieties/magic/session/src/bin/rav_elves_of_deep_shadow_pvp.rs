//! Full-game diagnostic authority for RAV Elves of Deep Shadow.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Game, GameEvent, GameView,
    Keyword, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const CARD_NAME: &str = "Elves of Deep Shadow";
const ABILITY_ID: &str = "produce-black-and-damage-controller";
const GASP_ID: &str = "RAV-LAST-GASP";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/elves_of_deep_shadow.rs";
const MAX_TRACE_SEED: u64 = 96;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/elves_of_deep_shadow.rs"]
mod candidate_elves_of_deep_shadow;

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
            eprintln!("Elves of Deep Shadow PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
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
        return Err("RAV catalog denominator changed from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.elf != replay.elf
        || captured.gasp != replay.gasp
    {
        return Err("Elves of Deep Shadow full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_green_cast_cost_detected": mutation_detected(seed, Mutation::WrongCastCost),
        "wrong_live_power_detected": mutation_detected(seed, Mutation::WrongPower),
        "wrong_mana_color_detected": mutation_detected(seed, Mutation::WrongManaColor),
        "controller_damage_removed_detected": mutation_detected(seed, Mutation::NoControllerDamage),
        "tap_cost_removed_detected": mutation_detected(seed, Mutation::NoTapCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_green_cast_cost_detected",
        "wrong_live_power_detected",
        "wrong_mana_color_detected",
        "controller_damage_removed_detected",
        "tap_cost_removed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Elves of Deep Shadow semantic mutant escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 161, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["105.2 and 202.1-202.2 colored mana", "117 priority and turn timing", "302.1-302.6 creature characteristics and summoning sickness", "605.1a and 605.3 activated mana abilities", "601.2 casting spells and paying costs", "608.2 resolving spells", "704 state-based actions", "510 combat damage"],
            "source_contract": "RAV Elves of Deep Shadow #161, {G} green 1/1 creature; {T}: Add {B}. Elves of Deep Shadow deals 1 damage to you.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongPower,
    WrongManaColor,
    NoControllerDamage,
    NoTapCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Witness {
    id: ObjectId,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<Keyword>,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    elf: Option<Witness>,
    opponent_elf: Option<ObjectId>,
    opponent_watchwolf: Option<ObjectId>,
    gasp: Option<ObjectId>,
    gasp_zone: Option<Zone>,
    watchwolf: Option<Witness>,
    player_life: [i64; 2],
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ManaAbilityBinding) {
    let mut definition = candidate_elves_of_deep_shadow::definition();
    let mut mana = candidate_elves_of_deep_shadow::mana_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(0, [Color::White]),
        Mutation::WrongPower => definition.power = Some(2),
        Mutation::WrongManaColor => mana.ability.output = ManaAbilityOutput::Fixed(Color::Green),
        Mutation::NoControllerDamage => mana.ability.controller_damage = None,
        Mutation::NoTapCost => mana.ability.tap_cost = false,
    }
    (definition, mana)
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_elves_of_deep_shadow::definition();
    let mana = candidate_elves_of_deep_shadow::mana_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Elves of Deep Shadow is missing from the RAV definition registry")?;
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|item| item.card_definition == CARD_ID)
        .ok_or("Elves of Deep Shadow is missing from the RAV mana registry")?;
    if registered != definition || registered_mana != mana {
        return Err(
            "Elves of Deep Shadow registry entries are not delegated to its candidate module"
                .to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "bound-tap-black-mana-ability",
                "source-aware-controller-damage",
            ]
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || mana.ability.id != ABILITY_ID
        || !mana.ability.tap_cost
        || mana.ability.output != ManaAbilityOutput::Fixed(Color::Black)
        || mana.ability.amount != 1
        || mana.ability.life_payment.is_some()
        || mana.ability.controller_damage != Some(1)
    {
        return Err("Elves of Deep Shadow identity, exact green cost, 1/1 characteristics, tap cost, black output, or controller damage differs from the source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Elves of Deep Shadow source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Elves of Deep Shadow source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 4 {
        return Err("Elves of Deep Shadow source must contain only two imports and two direct-literal functions".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item) if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) => {},
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !["definition", "mana_ability"].contains(&name.as_str())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty() || function.sig.constness.is_some() || function.sig.asyncness.is_some()
                    || function.block.stmts.len() != 1 || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                { return Err("Elves of Deep Shadow module contains unsupported executable source".to_owned()); }
            }
            _ => return Err("Elves of Deep Shadow module permits only private imports and declarative functions".to_owned()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "mana_ability".to_owned()]) {
        return Err("Elves of Deep Shadow candidate functions are incomplete".to_owned());
    }
    Ok(())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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
    let (candidate_definition, candidate_mana) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Elves of Deep Shadow definition missing during game construction")? =
        candidate_definition;
    let mut mana = rav_mana_ability_bindings();
    mana.retain(|binding| binding.card_definition != CARD_ID);
    mana.push(candidate_mana);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        mana,
        rav_basic_land_type_bindings(),
    )
    .map_err(|e| e.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_ELVES_OF_DEEP_SHADOW_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_ELVES_OF_DEEP_SHADOW_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Elves of Deep Shadow full-game mana trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (GASP_ID, 4),
        (WATCHWOLF_ID, 20),
        ("RAV-FOREST", 16),
        ("RAV-PLAINS", 12),
        ("RAV-SWAMP", 4),
    ]);
    let p1 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-FOREST", 26),
        ("RAV-PLAINS", 26),
    ]);
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Elves duel decks must contain 60 cards".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|e| e.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|e| e.to_string())?;
    }
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ElvesPolicy::new(PlayerId(0))),
        Box::new(ElvesPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut elf_live = None;
    while !game.is_game_over() {
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        if game.turn > 40 || accepted >= 5_000 {
            return Err("Elves full-game trace exceeded its bounded match".to_owned());
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Elves policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "Elves policy move rejected on turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("Elves invariant failure: {e}"))?;
        for event in &game.event_log[event_start..] {
            if let GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID)
                && game
                    .controller_of(*card)
                    .is_ok_and(|controller| controller == PlayerId(0))
            {
                let c = game.characteristics(*card).map_err(|e| e.to_string())?;
                elf_live = Some(Witness {
                    id: *card,
                    colors: c.colors.clone(),
                    card_types: c.card_types.clone(),
                    power: c.power,
                    toughness: c.toughness,
                    keywords: c.keywords.clone(),
                });
            }
        }
    }
    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, definition: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == definition) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let elf = elf_live;
    let opponent_elf = find_cast(PlayerId(1), CARD_ID);
    let opponent_watchwolf = find_cast(PlayerId(1), WATCHWOLF_ID);
    let gasp = find_cast(PlayerId(0), GASP_ID);
    let gasp_zone = gasp.and_then(|id| game.zone_of(id));
    let watchwolf_id = find_cast(PlayerId(0), WATCHWOLF_ID);
    let watchwolf = watchwolf_id
        .map(|id| {
            let c = game.characteristics(id).ok()?;
            Some(Witness {
                id,
                colors: c.colors.clone(),
                card_types: c.card_types.clone(),
                power: c.power,
                toughness: c.toughness,
                keywords: c.keywords.clone(),
            })
        })
        .flatten();
    let player_life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        elf,
        opponent_elf,
        opponent_watchwolf,
        gasp,
        gasp_zone,
        watchwolf,
        player_life,
    })
}

struct ElvesPolicy {
    player: PlayerId,
    elf_offered: bool,
    gasped: bool,
    wolf_offered: bool,
    mana_used: bool,
}

impl ElvesPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_offered: false,
            gasped: false,
            wolf_offered: false,
            mana_used: false,
        }
    }
    fn tap_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|c| {
            !c.tapped && c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let preferred = if self.player == PlayerId(0) {
            if !self.elf_offered {
                Color::Green
            } else if !view
                .own_battlefield
                .iter()
                .any(|c| c.mana_colors.contains(&Color::White))
            {
                Color::White
            } else if !view
                .own_battlefield
                .iter()
                .any(|c| c.mana_colors.contains(&Color::Green))
            {
                Color::Green
            } else {
                Color::Black
            }
        } else if !view
            .own_battlefield
            .iter()
            .any(|c| c.mana_colors.contains(&Color::White))
        {
            Color::White
        } else {
            Color::Green
        };
        let land = view
            .hand
            .iter()
            .find(|c| c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land))
            })?;
        Some(PolicyAction::PlayLand { card: land.id })
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|c| (c, view.mana_pool.amount(c)))
            .collect();
        for color in &cost.colored {
            let amount = available.get_mut(color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        if !cost.hybrid.is_empty() {
            return None;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, n)| **n > 0)
                .max_by_key(|(_, n)| **n)
                .map(|(c, _)| *c)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn cast(
        &mut self,
        view: &GameView,
        id: &str,
        cost: &ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        if let Some(mana_selection) = Self::payment(view, cost) {
            match id {
                CARD_ID => self.elf_offered = true,
                GASP_ID => self.gasped = true,
                WATCHWOLF_ID => self.wolf_offered = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let needed = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (u16::from(view.mana_pool.total())
                    < u16::from(cost.generic)
                        + cost.colored.len() as u16
                        + cost.hybrid.len() as u16)
                    .then_some(cost.colored.first().copied().unwrap_or(Color::Green))
            });
        needed
            .and_then(|color| self.tap_land(view, color))
            .or_else(|| {
                Color::ALL
                    .into_iter()
                    .find_map(|color| self.tap_land(view, color))
            })
    }
    fn declare(&self, view: &GameView) -> Option<PolicyAction> {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let id = if self.player == PlayerId(1) {
                WATCHWOLF_ID
            } else {
                WATCHWOLF_ID
            };
            return Some(PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(id) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            });
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return Some(PolicyAction::DeclareBlockers {
                assignments: vec![],
            });
        }
        None
    }
}

impl CodePolicy for ElvesPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.elves-of-deep-shadow-source-aware-mana.v1"
        } else {
            "rav.elves-deep-shadow-watchwolf-pressure.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.declare(view) {
            return action;
        }
        if self.player == PlayerId(0)
            && view.active_player == PlayerId(1)
            && view.step == Step::DeclareAttackers
            && view.attackers_declared
            && !self.gasped
        {
            if let Some(attacker) = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if !self.mana_used {
                    let elf = view.own_battlefield.iter().find(|card| {
                        card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
                    });
                    if let Some(elf) = elf {
                        self.mana_used = true;
                        return PolicyAction::ActivateBoundManaAbility {
                            activation: ManaAbilityActivation {
                                source: elf.id,
                                ability_id: ABILITY_ID,
                                chosen_color: None,
                            },
                        };
                    }
                }
                if view.mana_pool.amount(Color::Black) > 0
                    && view.mana_pool.amount(Color::Green) == 0
                {
                    if let Some(action) = self.tap_land(view, Color::Green) {
                        return action;
                    }
                    if let Some(action) = self.tap_land(view, Color::White) {
                        return action;
                    }
                    if let Some(action) = self.tap_land(view, Color::Black) {
                        return action;
                    }
                }
                if view.mana_pool.amount(Color::Black) > 0 && view.mana_pool.total() >= 2 {
                    let cost = ManaCost::with_colors(1, [Color::Black]);
                    if let Some(action) =
                        self.cast(view, GASP_ID, &cost, vec![Target::Permanent(attacker.id)])
                    {
                        return action;
                    }
                }
            }
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            let has_elf = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !has_elf && !self.elf_offered {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    &ManaCost::with_colors(0, [Color::Green]),
                    vec![],
                ) {
                    return action;
                }
            }
            if self.gasped
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && !self.wolf_offered
            {
                if let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
        } else {
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
                && !self.elf_offered
            {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    &ManaCost::with_colors(0, [Color::Green]),
                    vec![],
                ) {
                    return action;
                }
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
                && !self.wolf_offered
            {
                if let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 || c.player_life[0] <= 0 || c.player_life[1] > 0 {
        return Err(format!(
            "Elves duel did not reach a clean seat-zero win: {:?} {:?}",
            c.winner, c.player_life
        ));
    }
    let elf = c
        .elf
        .as_ref()
        .ok_or("seat zero did not cast Elves of Deep Shadow")?;
    let foe = c.opponent_elf.ok_or("seat one did not cast an Elf")?;
    let enemy_wolf = c
        .opponent_watchwolf
        .ok_or("seat one did not cast a Watchwolf")?;
    let gasp = c.gasp.ok_or("seat zero did not cast Last Gasp")?;
    if c.gasp_zone != Some(Zone::Graveyard) {
        return Err("Last Gasp did not reach its owner's graveyard".to_owned());
    }
    let wolf = c
        .watchwolf
        .as_ref()
        .ok_or("seat zero did not cast a Watchwolf finisher")?;
    if elf.colors != BTreeSet::from([Color::Green])
        || elf.card_types != BTreeSet::from([CardType::Creature])
        || elf.power != Some(1)
        || elf.toughness != Some(1)
        || !elf.keywords.is_empty()
    {
        return Err(
            "live Elves characteristics differ from the green 1/1 Creature contract".to_owned(),
        );
    }
    if wolf.power != Some(3) || wolf.toughness != Some(3) {
        return Err("own Watchwolf did not remain a live 3/3".to_owned());
    }
    let events = &c.events;
    let paid_elf = event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == elf.id && colors == &[Color::Green])).ok_or("Elves did not pay exact green to cast")?;
    let cast_elf = after(
        events,
        paid_elf,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == elf.id),
    )
    .ok_or("Elves SpellCast missing")?;
    let resolve_elf = after(
        events,
        cast_elf,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == elf.id),
    )
    .ok_or("Elves SpellResolved missing")?;
    let enter_elf = after(
        events,
        resolve_elf,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == elf.id),
    )
    .ok_or("Elves battlefield entry missing")?;
    let rival_cast = event(
        events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if *card == foe),
    )
    .ok_or("opponent Elf cast missing")?;
    let rival_payment = event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == foe && colors == &[Color::Green])).ok_or("opponent Elf exact green payment missing")?;
    let rival_resolved = after(
        events,
        rival_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == foe),
    )
    .ok_or("opponent Elf did not resolve")?;
    let rival_entered = after(
        events,
        rival_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == foe),
    )
    .ok_or("opponent Elf entry missing")?;
    let wolf_cast_opp = event(
        events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if *card == enemy_wolf),
    )
    .ok_or("opponent Watchwolf cast missing")?;
    let wolf_payment_opp = event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == enemy_wolf && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("opponent Watchwolf exact green and white payment missing")?;
    let wolf_resolved_opp = after(
        events,
        wolf_cast_opp,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == enemy_wolf),
    )
    .ok_or("opponent Watchwolf did not resolve")?;
    let wolf_enter_opp = after(events, wolf_resolved_opp, |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == enemy_wolf)).ok_or("opponent Watchwolf entry missing")?;
    let attack = after(events, wolf_enter_opp, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&enemy_wolf))).ok_or("opponent Watchwolf did not attack")?;
    let activated = after(events, attack, |e| matches!(e, GameEvent::BoundManaAbilityActivated { player: PlayerId(0), source, ability: ABILITY_ID, color: Color::Black, amount: 1, tapped: true, .. } if *source == elf.id)).ok_or("Elves did not tap for exactly one black mana with its damage drawback")?;
    let added_black = after(events, activated, |e| {
        matches!(
            e,
            GameEvent::ManaAdded {
                player: PlayerId(0),
                color: Color::Black,
                amount: 1
            }
        )
    })
    .ok_or("Elves did not add black to its controller's pool")?;
    let damage = after(events, added_black, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount: 1 } if *source == elf.id)).ok_or("Elves' controller did not take exactly one damage")?;
    let gasp_payment = after(events, damage, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == gasp && colors.len() == 2 && colors.contains(&Color::Black) && colors.iter().any(|color| *color != Color::Black))).ok_or("Last Gasp did not spend Elves' black mana plus generic mana")?;
    let targeted = after(
        events,
        gasp_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == gasp),
    )
    .ok_or("Last Gasp did not cast")?;
    let resolved_gasp = after(
        events,
        targeted,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == gasp),
    )
    .ok_or("Last Gasp did not resolve")?;
    let state_action = after(events, resolved_gasp, |e| matches!(e, GameEvent::StateBasedAction { card, reason: "creature has toughness zero or less" } if *card == enemy_wolf)).ok_or("Last Gasp did not make the opposing Watchwolf a 0/0")?;
    let enemy_died = after(
        events,
        state_action,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == enemy_wolf),
    )
    .ok_or("opposing Watchwolf did not move to graveyard")?;
    let own_wolf_cast = after(
        events,
        enemy_died,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf.id),
    )
    .ok_or("seat zero did not cast the Watchwolf finisher")?;
    let own_wolf_payment = after(events, enemy_died, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == wolf.id && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("own Watchwolf exact green and white payment missing")?;
    let own_wolf_resolved = after(
        events,
        own_wolf_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == wolf.id),
    )
    .ok_or("own Watchwolf did not resolve")?;
    let own_wolf_entry = after(
        events,
        own_wolf_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf.id),
    )
    .ok_or("own Watchwolf did not enter")?;
    let own_attack = after(events, own_wolf_entry, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf.id))).ok_or("own Watchwolf did not attack")?;
    let own_damage = after(events, own_attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == wolf.id)).ok_or("own Watchwolf did not deal three combat damage")?;
    let terminal = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            (i > own_damage
                && matches!(
                    e,
                    GameEvent::GameEnded {
                        winner: Some(PlayerId(0))
                    }
                ))
            .then_some(i)
        })
        .ok_or("terminal win receipt missing")?;
    let total_damage: i64 = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == wolf.id => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    if total_damage < 20
        || !(paid_elf < cast_elf
            && cast_elf < resolve_elf
            && resolve_elf < enter_elf
            && rival_payment < rival_cast
            && rival_cast < rival_resolved
            && rival_resolved < rival_entered
            && wolf_payment_opp < wolf_cast_opp
            && wolf_cast_opp < wolf_resolved_opp
            && wolf_resolved_opp < wolf_enter_opp
            && wolf_enter_opp < attack
            && enter_elf < attack
            && attack < activated
            && activated < added_black
            && added_black < damage
            && damage < gasp_payment
            && gasp_payment < targeted
            && targeted < resolved_gasp
            && resolved_gasp < state_action
            && state_action < enemy_died
            && enemy_died < own_wolf_payment
            && own_wolf_payment < own_wolf_cast
            && own_wolf_cast < own_wolf_resolved
            && own_wolf_resolved < own_wolf_entry
            && own_wolf_entry < own_attack
            && own_attack < own_damage
            && own_damage < terminal)
    {
        return Err("Elves cast, mana output, drawback, Last Gasp removal, combat, or terminal events are out of order".to_owned());
    }
    let selected: BTreeSet<usize> = [
        paid_elf,
        cast_elf,
        resolve_elf,
        enter_elf,
        rival_payment,
        rival_cast,
        rival_resolved,
        rival_entered,
        wolf_payment_opp,
        wolf_cast_opp,
        wolf_resolved_opp,
        wolf_enter_opp,
        attack,
        activated,
        added_black,
        damage,
        gasp_payment,
        targeted,
        resolved_gasp,
        state_action,
        enemy_died,
        own_wolf_payment,
        own_wolf_cast,
        own_wolf_resolved,
        own_wolf_entry,
        own_attack,
        own_damage,
        terminal,
    ]
    .into_iter()
    .collect();
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    let kinds: Vec<_> = projected.iter().map(|e| e.kind.as_str()).collect();
    for kind in [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "BoundManaAbilityActivated",
        "ManaAdded",
        "DamageDealtToPlayer",
        "AttackersDeclared",
        "StateBasedAction",
        "GameEnded",
    ] {
        if !kinds.contains(&kind) {
            return Err(format!("projected Elves trace omitted {kind}: {kinds:?}"));
        }
    }
    if projected
        .windows(2)
        .any(|pair| pair[0].sequence >= pair[1].sequence)
    {
        return Err("projected Elves event sequence is not ordered".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-elves-of-deep-shadow-black-mana-drawback-full-game",
        "shuffle_seed": seed,
        "decks": ["rav_elves_last_gasp_watchwolf", "rav_watchwolf_pressure"],
        "policies": ["rav.elves-of-deep-shadow-source-aware-mana.v1", "rav.elves-deep-shadow-watchwolf-pressure.v1"],
        "terminal": "Winner(PlayerId(0))", "winner_seat": 0, "turns": c.turns, "accepted_moves": c.accepted, "rejected_moves": c.rejected,
        "elves": {"object": elf.id.0, "collector_number": 161, "mana_cost": "{G}", "mana_paid": ["Green"], "colors": ["Green"], "card_types": ["Creature"], "power": 1, "toughness": 1, "zone": "Battlefield"},
        "mana_ability": {"id": ABILITY_ID, "source": elf.id.0, "mana_added": ["Black"], "tap_cost_paid": true, "controller_damage": 1, "controller_life_after_damage": c.player_life[0]},
        "last_gasp": {"object": gasp.0, "mana_cost": "{1}{B}", "target": enemy_wolf.0, "target_zone_after_resolution": "Graveyard", "spell_zone_after_resolution": "Graveyard", "state_action": "creature has toughness zero or less"},
        "opponent_elf_object": foe.0,
        "combat": {"enemy_watchwolf": enemy_wolf.0, "own_watchwolf": wolf.id.0, "own_watchwolf_power": 3, "own_watchwolf_toughness": 3, "total_damage_to_opponent": total_damage, "terminal_life": c.player_life},
        "events": projected, "passed": true
    }))
}

fn event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn after(events: &[GameEvent], i: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(i + 1)
        .find_map(|(n, e)| predicate(e).then_some(n))
}
fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|d| d.name == card.name)
        .map(|d| d.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_empty(){"catalog_only"}else{"executable_compatibility_slice"},"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
