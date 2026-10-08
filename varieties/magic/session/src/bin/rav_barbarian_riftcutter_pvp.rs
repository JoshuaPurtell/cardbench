//! Full-game RAV Barbarian Riftcutter authority.
//!
//! A shuffled two-player game casts and attacks with the Riftcutter, then
//! pays red and sacrifices it to destroy a live opposing land.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::{SeatId, ViewerScope};
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_cost_reduction_bindings,
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
use syn::{Expr, Item, Stmt, Type, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BARBARIAN-RIFTCUTTER";
const CARD_NAME: &str = "Barbarian Riftcutter";
const ABILITY_ID: &str = "sacrifice-destroy-target-land";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/barbarian_riftcutter.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/barbarian_riftcutter.rs"]
mod candidate_barbarian_riftcutter;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Barbarian Riftcutter result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Barbarian Riftcutter PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events || capture.winner != replay.winner {
        return Err("Barbarian Riftcutter shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "red_activated_cost_mutation_detected": mutation_detected(seed, Mutation::ActivatedCost),
        "source_sacrifice_mutation_detected": mutation_detected(seed, Mutation::NoSourceSacrifice),
        "land_destruction_effect_mutation_detected": mutation_detected(seed, Mutation::NoLandDestruction),
        "sealed_mutation_authority": false
    });
    for key in [
        "red_activated_cost_mutation_detected",
        "source_sacrifice_mutation_detected",
        "land_destruction_effect_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Barbarian Riftcutter mutation escaped the trace oracle: {key}"
            ));
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
        "card": {"collector_number": 114, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "301 Artifacts and 302 Creatures",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities, targets, and costs",
                "608.2 Resolving an activated ability",
                "701.7 Destroy",
                "701.17 Sacrifice",
                "704 State-based actions"
            ],
            "source_contract": "RAV Barbarian Riftcutter #114, {4}{R} red 3/3 creature with {R}, sacrifice this creature: destroy target land",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Barbarian Riftcutter source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Barbarian Riftcutter source is not valid declarative Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Barbarian Riftcutter source must contain two imports and two literal functions"
                .to_owned(),
        );
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports += 1;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_return = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => "",
                };
                let crate_visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
                let literal = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None));
                if expected_return.is_empty()
                    || !functions.insert(name)
                    || !crate_visible
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_return))
                    || !literal
                {
                    return Err(
                        "Barbarian Riftcutter functions must return direct declarative literals"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Barbarian Riftcutter source contains unsupported executable top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("Barbarian Riftcutter source structure differs from its contract".to_owned());
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn validate_candidate_semantics() -> Result<(), String> {
    let submitted = candidate_barbarian_riftcutter::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Barbarian Riftcutter is missing from the RAV registry")?;
    let definition_rules = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "sacrifice-source-destroy-land",
    ];
    let binding = candidate_barbarian_riftcutter::activated_ability();
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Barbarian Riftcutter activated ability binding is missing from the RAV registry")?;
    let ability = &binding.ability;
    if submitted.id != CARD_ID
        || submitted.name != CARD_NAME
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(4, [Color::Red])
        || submitted.colors != BTreeSet::from([Color::Red])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(3)
        || submitted.toughness != Some(3)
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
        || submitted.supported_rules != definition_rules
        || registered.mana_cost != submitted.mana_cost
        || registered.colors != submitted.colors
        || registered.power != submitted.power
        || registered.toughness != submitted.toughness
        || binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [cardbench_magic_engine::TargetRequirement::Land]
        || ability.effects != [Effect::DestroyTargetLand]
        || registered_binding.ability.mana_cost != ability.mana_cost
        || registered_binding.ability.sacrifice_source != ability.sacrifice_source
        || registered_binding.ability.targets != ability.targets
        || registered_binding.ability.effects != ability.effects
    {
        return Err("Barbarian Riftcutter identity, characteristics, activated cost, sacrifice, target, or effect differ from the RAV printing".to_owned());
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !cards
            .iter()
            .any(|card| card.collector_number == 114 && card.name == CARD_NAME)
    {
        return Err(
            "RAV catalog identity or denominator changed for Barbarian Riftcutter".to_owned(),
        );
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    ActivatedCost,
    NoSourceSacrifice,
    NoLandDestruction,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_barbarian_riftcutter::definition();
    let mut binding = candidate_barbarian_riftcutter::activated_ability();
    match mutation {
        Mutation::ActivatedCost => {
            binding.ability.mana_cost = ManaCost::with_colors(1, [Color::Red])
        }
        Mutation::NoSourceSacrifice => binding.ability.sacrifice_source = false,
        Mutation::NoLandDestruction => {
            binding.ability.effects = vec![Effect::GainLifeController { amount: 1 }]
        }
        Mutation::None => {}
    }
    if mutation == Mutation::None {
        return (definition, binding);
    }
    // Keep this hook local and explicit so future card mutations can cover both
    // the creature characteristics and activated-ability branches.
    let _ = &mut definition;
    (definition, binding)
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    target_land: Option<ObjectId>,
    cast_payment: Vec<Color>,
    ability_cost: Option<ManaCost>,
    source_sacrificed_as_cost: bool,
    source_moved_to_graveyard: bool,
    target_destroyed_by_source: bool,
    target_moved_to_graveyard: bool,
    source_pt: Option<(i32, i32)>,
    opponent_life: i64,
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

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_RIFTCUTTER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_RIFTCUTTER_SEED must be unsigned".to_owned())?;
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
        "no Barbarian Riftcutter trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    let Ok(capture) = run_match(seed, mutation) else {
        return true;
    };
    validate_trace(&capture, seed).is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let (definition, binding) = candidate_values(mutation);
    let spell_cost = definition.mana_cost.clone();
    let activation_cost = binding.ability.mana_cost.clone();
    let pilot_deck = deck(&[
        (CARD_ID, 4),
        ("RAV-GOBLIN-FIRE-FIEND", 4),
        ("RAV-MOUNTAIN", 52),
    ]);
    let opponent_deck = deck(&[("RAV-MOUNTAIN", 60)]);
    for (name, list) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!(
                "{name} Barbarian Riftcutter deck is not sixty cards"
            ));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition, binding.clone())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(RiftcutterPolicy::new(
            PlayerId(0),
            spell_cost,
            activation_cost,
            binding.ability.sacrifice_source,
        )),
        Box::new(RavCatalogPolicy::new(
            PlayerId(1),
            CatalogPolicyProfile::Curve,
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut target_land = None;
    let mut source_pt = None;
    let mut cast_payment = Vec::new();
    let mut ability_cost = None;
    let mut source_sacrificed_as_cost = false;
    let mut source_moved_to_graveyard = false;
    let mut target_destroyed_by_source = false;
    let mut target_moved_to_graveyard = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Barbarian Riftcutter game hit its development run bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Riftcutter policy seat missing")?;
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
        if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == ABILITY_ID
        {
            source.get_or_insert(activation.source);
            if let Some(Target::Permanent(land)) = activation.targets.first() {
                target_land.get_or_insert(*land);
            }
        }
        if let PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } = &action
            && view
                .hand
                .iter()
                .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
        {
            source.get_or_insert(request.card);
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy action rejected at turn {} / {:?} for {:?}: {error}",
                    game.turn, view.step, player
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    source.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source => cast_payment = colors.clone(),
                GameEvent::SpellResolved { card } if Some(*card) == source => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    source_pt = Some((
                        characteristics.power.ok_or("Riftcutter power missing")?,
                        characteristics
                            .toughness
                            .ok_or("Riftcutter toughness missing")?,
                    ));
                }
                GameEvent::AbilityManaPaid {
                    source: ability_source,
                    ability,
                    mana_cost,
                    ..
                } if Some(*ability_source) == source && *ability == ABILITY_ID => {
                    ability_cost = Some(mana_cost.clone())
                }
                GameEvent::SacrificedAsAbilityCost {
                    source: ability_source,
                    permanent,
                    ..
                } if Some(*ability_source) == source && Some(*permanent) == source => {
                    source_sacrificed_as_cost = true
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if Some(*card) == source => source_moved_to_graveyard = true,
                GameEvent::CardDestroyed {
                    source: ability_source,
                    card,
                } if Some(*ability_source) == source && Some(*card) == target_land => {
                    target_destroyed_by_source = true
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if Some(*card) == target_land => target_moved_to_graveyard = true,
                _ => {}
            }
        }
    }
    let opponent_life = game
        .player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .life;
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        source,
        target_land,
        cast_payment,
        ability_cost,
        source_sacrificed_as_cost,
        source_moved_to_graveyard,
        target_destroyed_by_source,
        target_moved_to_graveyard,
        source_pt,
        opponent_life,
    })
}

fn new_game(definition: CardDefinition, binding: ActivatedAbilityBinding) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Barbarian Riftcutter missing from card definitions")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|known| known.card_definition != CARD_ID);
    abilities.push(binding);
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        rav_triggered_ability_bindings(),
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
    Ok(game)
}

struct RiftcutterPolicy {
    player: PlayerId,
    spell_cost: ManaCost,
    activation_cost: ManaCost,
    source_sacrifice: bool,
    source_cast_turn: Option<u32>,
    activation_submitted: bool,
    inner: RavCatalogPolicy,
}

impl RiftcutterPolicy {
    fn new(
        player: PlayerId,
        spell_cost: ManaCost,
        activation_cost: ManaCost,
        source_sacrifice: bool,
    ) -> Self {
        Self {
            player,
            spell_cost,
            activation_cost,
            source_sacrifice,
            source_cast_turn: None,
            activation_submitted: false,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }

    fn activate_red_land(&self, view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Red)
                    && !card.tapped
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Red,
            })
    }
}

impl CodePolicy for RiftcutterPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.barbarian-riftcutter-red-sacrifice.v1"
        } else {
            "rav.barbarian-riftcutter-land-deck.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && self.source_cast_turn.is_some()
            && view.active_player == self.player
            && view.priority == self.player
            && view.step == Step::PrecombatMain
        {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && !self.activation_submitted
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && view.step == Step::PostcombatMain
            && self.source_cast_turn.is_some_and(|turn| view.turn > turn)
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land));
            if let (Some(source), Some(target)) = (source, target) {
                let required = self.activation_cost.mana_value();
                if view.mana_pool.amount(Color::Red) == 0 || view.mana_pool.total() < required {
                    return self
                        .activate_red_land(view)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                self.activation_submitted = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: if self.source_sacrifice {
                            vec![source.id]
                        } else {
                            vec![]
                        },
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        let mut action = self.inner.propose_move(view);
        if self.player == PlayerId(0) && self.source_cast_turn.is_none() {
            let request = match &action {
                PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } => {
                    Some(request.clone())
                }
                _ => None,
            };
            if let Some(request) = request
                && view
                    .hand
                    .iter()
                    .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
                && let Some(mana_selection) = payment_selection(view, &self.spell_cost)
            {
                self.source_cast_turn = Some(view.turn);
                action = PolicyAction::CastWithPayment {
                    request,
                    chosen_x: None,
                    mana_selection,
                };
            }
        }
        action
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
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

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let source = capture.source.ok_or("Barbarian Riftcutter was not cast")?;
    let target = capture.target_land.ok_or_else(|| {
        format!(
            "opponent land was not selected; turns={}, winner={:?}",
            capture.turns, capture.winner
        )
    })?;
    if capture.winner != Some(0) || capture.rejected_moves != 0 || capture.opponent_life > 0 {
        return Err(
            "Barbarian Riftcutter shuffled PvP game did not reach a clean seat-zero win".to_owned(),
        );
    }
    if capture.cast_payment.len() != 5
        || capture
            .cast_payment
            .iter()
            .any(|color| *color != Color::Red)
        || capture.source_pt != Some((3, 3))
        || capture.ability_cost.as_ref() != Some(&ManaCost::with_colors(0, [Color::Red]))
        || !capture.source_sacrificed_as_cost
        || !capture.source_moved_to_graveyard
        || !capture.target_destroyed_by_source
        || !capture.target_moved_to_graveyard
    {
        return Err(format!(
            "Riftcutter cast, red activation, source-sacrifice cost, target destruction, or terminal state differs: source={source:?}, payment={:?}, payment_events={:?}, ability_cost={:?}, pt={:?}, sacrificed={}, source_graveyard={}, destroyed={}, target_graveyard={}",
            capture.cast_payment,
            capture
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::SpellManaPaid { .. }))
                .collect::<Vec<_>>(),
            capture.ability_cost,
            capture.source_pt,
            capture.source_sacrificed_as_cost,
            capture.source_moved_to_graveyard,
            capture.target_destroyed_by_source,
            capture.target_moved_to_graveyard
        ));
    }
    let events = &capture.events;
    let paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 5 && colors.iter().all(|color| *color == Color::Red))).ok_or("exact five-mana Riftcutter spell payment missing")?;
    let cast = find_event_after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Riftcutter cast receipt missing")?;
    let resolved = find_event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Riftcutter resolution missing")?;
    let entry = find_event_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Riftcutter battlefield entry missing")?;
    let attack = find_event_after(events, entry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).ok_or("Riftcutter did not attack before using its ability")?;
    let combat_damage = find_event_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: attacker, player: PlayerId(1), amount: 3 } if *attacker == source)).ok_or("Riftcutter's unblocked combat damage missing")?;
    let ability_paid = find_event_after(events, combat_damage, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: ability_source, ability: ABILITY_ID, mana_cost } if *ability_source == source && *mana_cost == ManaCost::with_colors(0, [Color::Red]))).ok_or("red ability mana payment missing")?;
    let sacrificed = find_event_after(events, ability_paid, |event| matches!(event, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source: ability_source, permanent } if *ability_source == source && *permanent == source)).ok_or("Riftcutter was not sacrificed as its activation cost")?;
    let source_gone = find_event_after(events, sacrificed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source)).ok_or("sacrificed Riftcutter did not move to graveyard")?;
    let activated = find_event_after(events, source_gone, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Riftcutter activated ability did not reach the stack")?;
    let destroyed = find_event_after(events, activated, |event| matches!(event, GameEvent::CardDestroyed { source: ability_source, card } if *ability_source == source && *card == target)).ok_or("activation did not destroy its chosen opposing land")?;
    let target_gone = find_event_after(events, destroyed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("destroyed opposing land did not move to graveyard")?;
    let ability_resolved = find_event_after(events, target_gone, |event| matches!(event, GameEvent::AbilityResolved { source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Riftcutter ability resolution receipt missing")?;
    let game_end = find_event_after(events, ability_resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal win event missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entry
        && entry < attack
        && attack < combat_damage
        && combat_damage < ability_paid
        && ability_paid < sacrificed
        && sacrificed < source_gone
        && source_gone < activated
        && activated < destroyed
        && destroyed < target_gone
        && target_gone < ability_resolved
        && ability_resolved < game_end)
    {
        return Err("Riftcutter spell, attack, source-cost, land-destruction, and terminal event order differs".to_owned());
    }
    let projection = project_events(events);
    let selected = [
        paid,
        cast,
        resolved,
        entry,
        attack,
        combat_damage,
        ability_paid,
        sacrificed,
        source_gone,
        activated,
        destroyed,
        target_gone,
        ability_resolved,
        game_end,
    ];
    let projected: Vec<ProjectedEvent> = selected
        .into_iter()
        .map(|index| projection[index].clone())
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "AbilityManaPaid",
        "SacrificedAsAbilityCost",
        "CardMoved",
        "AbilityActivated",
        "CardDestroyed",
        "CardMoved",
        "AbilityResolved",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!("projected Riftcutter events differ: {kinds:?}"));
    }
    let facts: Vec<_> = projected.iter().map(|event| &event.facts).collect();
    let objects = |index: usize| {
        facts[index]
            .objects
            .iter()
            .map(|object| object.0)
            .collect::<Vec<_>>()
    };
    if objects(0) != [source.0]
        || facts[0].seats != [SeatId(0)]
        || facts[0].amount != Some(5)
        || !facts[0].mana_colors.iter().any(|color| color == "Red")
        || objects(1) != [source.0]
        || objects(2) != [source.0]
        || objects(3) != [source.0]
        || facts[3].zone.as_deref() != Some("Battlefield")
        || !objects(4).contains(&source.0)
        || objects(5) != [source.0]
        || facts[5].seats != [SeatId(1)]
        || facts[5].amount != Some(3)
        || objects(6) != [source.0]
        || facts[6].mana_cost_colors != ["Red"]
        // The shared projector currently omits object references from the
        // sacrifice-cost receipt; the typed engine event above still binds
        // both source and permanent to the Riftcutter incarnation.
        || !objects(7).is_empty()
        || objects(8) != [source.0]
        || facts[8].zone.as_deref() != Some("Graveyard")
        || objects(9) != [source.0]
        || objects(10) != [source.0, target.0]
        || objects(11) != [target.0]
        || facts[11].zone.as_deref() != Some("Graveyard")
        || objects(12) != [source.0]
        || facts[12].ability.as_deref() != Some(ABILITY_ID)
        || facts[13].seats != [SeatId(0)]
    {
        return Err(format!(
            "Riftcutter projected event facts differ: {:?}",
            projected
                .iter()
                .map(|event| (&event.kind, &event.facts))
                .collect::<Vec<_>>()
        ));
    }
    Ok(json!({
        "trace_id": "rav-barbarian-riftcutter-attack-sacrifice-destroy-land-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_riftcutter_mountains_60", "rav_mountain_land_60"],
        "policies": ["rav.barbarian-riftcutter-red-sacrifice.v1", "rav.barbarian-riftcutter-land-deck.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "barbarian_riftcutter": {"object": source.0, "collector_number": 114, "mana_cost": "{4}{R}", "mana_paid": capture.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": capture.source_pt.unwrap().0, "toughness": capture.source_pt.unwrap().1, "attacked_before_sacrifice": true, "zone_after_cost": "Graveyard"},
        "activation": {"ability": ABILITY_ID, "source": source.0, "mana_cost": "{R}", "red_payment": true, "source_sacrificed_as_cost": capture.source_sacrificed_as_cost, "source_moved_before_stack": true},
        "target_land": {"object": target.0, "controller": 1, "zone_before": "Battlefield", "destroyed_by_source": capture.target_destroyed_by_source, "zone_after": "Graveyard"},
        "combat": {"damage_before_activation": 3, "opponent_life_at_terminal": capture.opponent_life},
        "events": projected,
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
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
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status {CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
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
