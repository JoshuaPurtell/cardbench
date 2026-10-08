//! Full-game diagnostic authority for RAV Festival of the Guildpact.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FESTIVAL-OF-THE-GUILDPACT";
const CHAR_ID: &str = "RAV-CHAR";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/festival_of_the_guildpact.rs";
const TRACE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/festival_of_the_guildpact.rs"]
mod candidate_festival;

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
            eprintln!("Festival of the Guildpact PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let reference = run_match(TRACE_SEED, Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(TRACE_SEED, Mutation::None)?;
    if reference != replay {
        return Err("Festival of the Guildpact full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "chosen_x_amount_lowered_mutation_detected_in_game": mutation_detected(Mutation::LowerX),
        "controller_shield_removed_mutation_detected_in_game": mutation_detected(Mutation::NoShield),
        "draw_instruction_removed_mutation_detected_in_game": mutation_detected(Mutation::NoDraw),
        "sealed_mutation_authority": false
    });
    if mutations["chosen_x_amount_lowered_mutation_detected_in_game"] != true
        || mutations["controller_shield_removed_mutation_detected_in_game"] != true
        || mutations["draw_instruction_removed_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Festival semantic mutation escaped the gameplay oracle: {mutations}"
        ));
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 17, "definition_id": CARD_ID, "name": "Festival of the Guildpact"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["601.2: casting spells and choosing X", "608.2: resolving spells and abilities", "609.8: damage prevention", "120: damage", "121: drawing cards", "514: cleanup step and end of turn"],
            "source_contract": "RAV Festival of the Guildpact is a white Instant with cost {X}{W}; its controller prevents the next X damage that would be dealt to them this turn, then draws a card.",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [trace_json(&reference)],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Festival source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Festival source is not valid Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut function_count = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Festival source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.sig.ident == "definition"
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.block.stmts.len() == 1
                    && matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, "CardDefinition"))
                    && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none() && path_is_type_path(&value.path, "CardDefinition")) =>
            {
                function_count += 1
            }
            _ => {
                return Err(
                    "Festival module structure differs from its literal source contract".to_owned(),
                );
            }
        }
    }
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || imports != expected_imports
        || function_count != 1
        || file.items.len() != 3
    {
        return Err(
            "Festival module structure differs from its literal source contract".to_owned(),
        );
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, output: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, output)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            output.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, output)),
        _ => false,
    }
}

fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == name)
}

fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none() && path.segments.len() == 1 && path.segments[0].ident == name
}

fn expected_definition() -> CardDefinition {
    let mut definition = candidate_festival::definition();
    definition.effects = expected_effects(Mutation::None);
    definition
}

fn expected_effects(mutation: Mutation) -> Vec<Effect> {
    let mut effects = Vec::new();
    if mutation != Mutation::NoShield {
        effects.push(Effect::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn);
    }
    if mutation != Mutation::NoDraw {
        effects.push(Effect::DrawController);
    }
    effects
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_festival::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Festival definition is missing from the RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != "Festival of the Guildpact"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(0, [Color::White])
        || candidate.colors != BTreeSet::from([Color::White])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || candidate.effects != expected_effects(Mutation::None)
    {
        return Err("Festival definition is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from source contract".to_owned());
    }
    Ok(catalog)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    LowerX,
    NoShield,
    NoDraw,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    festival: Option<cardbench_magic_engine::ObjectId>,
    char_spell: Option<cardbench_magic_engine::ObjectId>,
    chosen_x: Option<u8>,
    paid: Vec<Color>,
    shield: Option<i32>,
    prevented: Option<i32>,
    damage_to_controller: i64,
    hand_before_cast: usize,
    hand_after_resolution: usize,
    drawn_card: Option<String>,
    drawn_object: Option<cardbench_magic_engine::ObjectId>,
    char_resolved: bool,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let mut definition = expected_definition();
    definition.effects = expected_effects(mutation);
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Festival definition missing from registry")? = definition;
    let triggers = {
        let mut bindings = rav_triggered_ability_bindings();
        bindings.extend(rav_attachment_triggered_ability_bindings());
        bindings
    };
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;

    let deck0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-FOREST", 26),
        ("RAV-PLAINS", 26),
    ]);
    let deck1 = deck(&[(CHAR_ID, 4), ("RAV-MOUNTAIN", 56)]);
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 90,
        max_policy_moves: 9000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let x = match mutation {
        Mutation::LowerX => 3,
        Mutation::NoShield => 0,
        Mutation::None | Mutation::NoDraw => 4,
    };
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FestivalPolicy::new(PlayerId(0), x)),
        Box::new(FestivalPolicy::new(PlayerId(1), x)),
    ];
    let (mut accepted, mut attempted) = (0_u32, 0_u32);
    let mut festival = None;
    let mut char_spell = None;
    let mut chosen_x = None;
    let mut paid = Vec::new();
    let mut shield = None;
    let mut prevented = None;
    let mut hand_before_cast = 0;
    let mut hand_after_resolution = 0;
    let mut before_hand = BTreeSet::new();
    let mut drawn_card = None;
    let mut drawn_object = None;
    let mut char_resolved = false;
    let mut festival_draw_captured = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Festival duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if let Some(action) = policies[player.0].propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policies[player.0].propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policies[player.0].propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policies[player.0].propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policies[player.0].propose_private_opponent_library_choice(&view)
        } else {
            policies[player.0].propose_move(&view)
        };
        if let PolicyAction::CastWithPayment {
            request,
            chosen_x: action_x,
            ..
        } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|card| card.id == CARD_ID)
        {
            hand_before_cast = game
                .player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .hand
                .len();
            before_hand = game
                .player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .hand
                .iter()
                .copied()
                .collect();
            chosen_x = *action_x;
        }
        attempted += 1;
        game.submit_policy_move(player, policies[player.0].id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        for event in &game.event_log {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    festival = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CHAR_ID) =>
                {
                    char_spell = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == festival => paid.clone_from(colors),
                GameEvent::DamageShieldCreated {
                    source,
                    target: Target::Player(PlayerId(0)),
                    amount,
                } if Some(*source) == festival => shield = Some(*amount),
                GameEvent::DamagePrevented {
                    source,
                    target: Target::Player(PlayerId(0)),
                    amount,
                } if Some(*source) == char_spell => prevented = Some(*amount),
                GameEvent::SpellResolved { card }
                    if Some(*card) == festival && !festival_draw_captured =>
                {
                    let state = game
                        .player(PlayerId(0))
                        .map_err(|error| error.to_string())?;
                    hand_after_resolution = state.hand.len();
                    drawn_object = state
                        .hand
                        .iter()
                        .find(|card| !before_hand.contains(card))
                        .copied();
                    drawn_card = drawn_object.and_then(|card| {
                        game.card_definition(card)
                            .ok()
                            .map(|definition| definition.id.to_owned())
                    });
                    festival_draw_captured = true;
                }
                GameEvent::SpellResolved { card } if Some(*card) == char_spell => {
                    char_resolved = true
                }
                _ => {}
            }
        }
    }
    let winner = game.winner().map(|seat| seat.0);
    let damage_to_controller = game
        .player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .life;
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        festival,
        char_spell,
        chosen_x,
        paid,
        shield,
        prevented,
        damage_to_controller,
        hand_before_cast,
        hand_after_resolution,
        drawn_card,
        drawn_object,
        char_resolved,
    })
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

struct FestivalPolicy {
    player: PlayerId,
    x: u8,
    festival_cast: bool,
    char_cast: bool,
}
impl FestivalPolicy {
    fn new(player: PlayerId, x: u8) -> Self {
        Self {
            player,
            x,
            festival_cast: false,
            char_cast: false,
        }
    }
}

impl FestivalPolicy {
    fn land_to_tap(
        &self,
        view: &GameView,
        color: Color,
    ) -> Option<cardbench_magic_engine::ObjectId> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| card.id)
    }
    fn make_festival_mana(&self, view: &GameView) -> Option<PolicyAction> {
        if view.mana_pool.amount(Color::White) == 0 {
            return self.land_to_tap(view, Color::White).map(|land| {
                PolicyAction::ActivateManaAbility {
                    land,
                    color: Color::White,
                }
            });
        }
        if view.mana_pool.amount(Color::Green) < self.x {
            return self.land_to_tap(view, Color::Green).map(|land| {
                PolicyAction::ActivateManaAbility {
                    land,
                    color: Color::Green,
                }
            });
        }
        None
    }
    fn make_watchwolf_mana(&self, view: &GameView) -> Option<PolicyAction> {
        for color in [Color::Green, Color::White] {
            if view.mana_pool.amount(color) == 0 {
                if let Some(land) = self.land_to_tap(view, color) {
                    return Some(PolicyAction::ActivateManaAbility { land, color });
                }
            }
        }
        None
    }
}

impl CodePolicy for FestivalPolicy {
    fn id(&self) -> &'static str {
        "rav.festival-of-the-guildpact-full-game.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && !self.festival_cast
            && view
                .stack_spells
                .iter()
                .any(|card| card.definition == Some(CHAR_ID))
        {
            if view.mana_pool.amount(Color::White) > 0
                && view.mana_pool.amount(Color::Green) >= self.x
            {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    self.festival_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: (self.x > 0).then_some(self.x),
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::Green; self.x as usize],
                            hybrid: vec![],
                        },
                    };
                }
            }
            if let Some(action) = self.make_festival_mana(view) {
                return action;
            }
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let has_white = view.own_battlefield.iter().any(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            });
            let land = if !has_white {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-PLAINS"))
            } else {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-FOREST"))
            }
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            });
            if let Some(land) = land {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(1)
            && !self.char_cast
            && view
                .opponent_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count()
                >= 5
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CHAR_ID))
            {
                if view.mana_pool.amount(Color::Red) >= 3 {
                    self.char_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![Target::Player(PlayerId(0))],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::Red, Color::Red],
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(land) = self.land_to_tap(view, Color::Red) {
                    return PolicyAction::ActivateManaAbility {
                        land,
                        color: Color::Red,
                    };
                }
            }
        }
        if self.player == PlayerId(0) && self.festival_cast {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0
                    && view.mana_pool.amount(Color::White) > 0
                {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![],
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(action) = self.make_watchwolf_mana(view) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured) -> Result<(), String> {
    if capture.terminal != "Winner(PlayerId(0))" || capture.rejected != 0 {
        return Err("Festival duel did not end in a clean P0 win".to_owned());
    }
    let source = capture.festival.ok_or("Festival was not cast")?;
    let char_card = capture
        .char_spell
        .ok_or("opponent did not cast Char at seat zero")?;
    if capture.chosen_x != Some(4)
        || capture.paid.len() != 5
        || capture
            .paid
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            != 1
        || capture
            .paid
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            != 4
    {
        return Err("Festival did not choose X=4 and pay exactly {4}{W}".to_owned());
    }
    if capture.shield != Some(4)
        || capture.prevented != Some(4)
        || capture.damage_to_controller != 20
        || !capture.char_resolved
    {
        return Err(
            "Festival did not prevent the full four-point Char packet to its controller".to_owned(),
        );
    }
    if capture.hand_before_cast == 0
        || capture.hand_after_resolution != capture.hand_before_cast
        || capture.drawn_card.is_none()
    {
        return Err("Festival did not draw one card after its shield instruction".to_owned());
    }
    let cast_index = capture.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Festival cast receipt missing")?;
    let resolved_index = capture
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == source))
        .ok_or("Festival resolution receipt missing")?;
    let shield_index = capture.events.iter().position(|event| matches!(event, GameEvent::DamageShieldCreated { source: shield_source, target: Target::Player(PlayerId(0)), amount: 4 } if *shield_source == source)).ok_or("Festival shield receipt missing")?;
    let draw_move_index = capture.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if Some(*card) == capture.drawn_object));
    let prevented_index = capture.events.iter().position(|event| matches!(event, GameEvent::DamagePrevented { source: damage_source, target: Target::Player(PlayerId(0)), amount: 4 } if *damage_source == char_card)).ok_or("Festival did not prevent the targeted Char damage event")?;
    if !(cast_index < shield_index
        && shield_index
            < draw_move_index.ok_or("Festival's drawn card movement receipt is missing")?
        && draw_move_index.is_some_and(|draw| draw < resolved_index)
        && resolved_index < prevented_index)
    {
        return Err("Festival shield and draw did not resolve before Char damage".to_owned());
    }
    let damage_from_char_to_p0 = capture.events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(0), .. } if *damage_source == char_card));
    if damage_from_char_to_p0 {
        return Err("Char damage reached seat zero despite the full prevention shield".to_owned());
    }
    if draw_move_index.is_none() {
        return Err(
            "Festival's drawn card move is not present in the public event trace".to_owned(),
        );
    }
    if !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
                ..
            }
        )
    }) {
        return Err("Festival duel lacks a terminal P0 game-end receipt".to_owned());
    }
    Ok(())
}

fn mutation_detected(mutation: Mutation) -> bool {
    run_match(TRACE_SEED, mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}

fn trace_json(capture: &Captured) -> Value {
    let festival = capture.festival;
    let char_card = capture.char_spell;
    let indices: Vec<usize> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card }
                if Some(*card) == festival || Some(*card) == char_card =>
            {
                Some(index)
            }
            GameEvent::DamageShieldCreated { source, .. } if Some(*source) == festival => {
                Some(index)
            }
            GameEvent::DamagePrevented { source, .. } if Some(*source) == char_card => Some(index),
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if Some(*card) == capture.drawn_object => Some(index),
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if !attackers.is_empty() => Some(index),
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
            | GameEvent::GameEnded { .. } => Some(index),
            _ => None,
        })
        .collect();
    let projected = project_events(&capture.events);
    let events: Vec<Value> = indices.into_iter().filter_map(|index| {
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": "rav-festival-x4-prevents-char-and-draws",
        "terminal": capture.terminal,
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted,
        "rejected_policy_moves": capture.rejected,
        "mechanics": {
            "mana_cost": "{X}{W}", "chosen_x": capture.chosen_x,
            "paid_mana_colors": capture.paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "paid_generic_x": 4, "paid_white": 1,
            "controller_damage_shield": capture.shield,
            "damage_prevented_from_char": capture.prevented,
            "char_resolved": capture.char_resolved,
            "controller_life_after_char": capture.damage_to_controller,
            "hand_before_cast": capture.hand_before_cast,
            "hand_after_resolution": capture.hand_after_resolution,
            "drawn_card_definition": capture.drawn_card,
            "terminal_combat_win": capture.winner == Some(0)
        },
        "events": events
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 17;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": if passed { definition_id } else { None }, "passed": passed, "credited": false, "reward": 0.0})
}
