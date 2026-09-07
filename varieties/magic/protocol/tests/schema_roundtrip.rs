//! Milestone 1: every transported type survives a serialisation round trip.
//!
//! A round trip is not a formality here. The protocol's whole purpose is to be
//! the boundary an out-of-process client eventually speaks across; a type that
//! silently loses a field in transit would turn a rules-correct engine into an
//! incorrect match.

mod support;

use cardbench_magic_protocol::{
    AbilityActivationDto, AbilityName, ActionRequestId, ActionRequestKind, BasicLandTypeDto,
    BlockAssignmentDto, CardName, CastRequestDto, ColorDto, CommandEnvelope, CommandKind,
    CommandReceipt, CommandResult, ConvokeContributionDto, ConvokePaymentDto,
    DamageReplacementChoiceDto, DecisionKindLabel, DecisionPrompt, DecisionRef,
    DecisionSelectionDto, DecisionVisibilityDto, EventRecord, EventStreamPage, EventVisibility,
    GameCommand, HiddenZoneProjection, LegalActionOption, LegalActionSurface,
    ManaAbilityActivationDto, ManaPaymentSelectionDto, MatchObservation, ObjectRef,
    PROTOCOL_SCHEMA_VERSION, PaymentManaAbilityDto, ProtocolCapabilities, ProtocolError,
    ReplacementChoiceDto, ReplacementEffectDto, SeatId, StateRevision, SupportLevel, TargetDto,
    TerminalOutcome, TerminalResult, TriggerOrderEntryDto, ViewerScope,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use support::{SEAT_A, SEAT_B, match_id, priority_request, revision, three_seat_observation};

/// Asserts JSON round-trip fidelity and that re-encoding is byte-stable.
///
/// The second encoding matters: a type that decodes to something equal but
/// re-encodes differently will produce a different digest on a replay path.
fn round_trip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let encoded = serde_json::to_string(value).expect("serialise");
    let decoded: T = serde_json::from_str(&encoded).expect("deserialise");
    assert_eq!(value, &decoded, "round trip changed the value");
    let re_encoded = serde_json::to_string(&decoded).expect("re-serialise");
    assert_eq!(encoded, re_encoded, "re-encoding was not byte-stable");
}

#[test]
fn schema_version_is_reported_and_compatible_with_itself() {
    assert!(PROTOCOL_SCHEMA_VERSION.is_compatible_with(PROTOCOL_SCHEMA_VERSION));
    round_trip(&PROTOCOL_SCHEMA_VERSION);
}

#[test]
fn capability_manifest_round_trips() {
    round_trip(&ProtocolCapabilities::current());
}

/// The manifest must not claim more than the engine can do. This is the test
/// that stops a future edit from quietly promoting an unimplemented format.
#[test]
fn capability_manifest_claims_only_what_is_implemented() {
    use cardbench_magic_protocol::{FeatureCapability, FormatCapability};

    let capabilities = ProtocolCapabilities::current();
    assert_eq!(
        capabilities.verified_seat_counts,
        vec![2, 4],
        "two seats are the duel fixtures; four are the M5 format fixtures in \
         engine/tests/formats_m5.rs"
    );
    assert_eq!(
        capabilities.format(FormatCapability::Duel),
        SupportLevel::Supported
    );
    // M5b/M5c landed the rules and the fixtures together. Each claim below is
    // defended by a named deterministic test; if one is deleted, this must
    // move back to `Absent` rather than the test being weakened.
    for (implemented, fixture) in [
        (
            FormatCapability::Commander,
            "formats_m5.rs::commander_unblocked_swing_reaches_21_and_eliminates",
        ),
        (
            FormatCapability::TwoHeadedGiant,
            "formats_m5.rs::two_headed_giant_unblocked_damage_hits_shared_team_life",
        ),
    ] {
        assert_eq!(
            capabilities.format(implemented),
            SupportLevel::Supported,
            "{implemented:?} is implemented and defended by engine/tests/{fixture}"
        );
    }
    assert_eq!(
        capabilities.format(FormatCapability::BoosterDraft),
        SupportLevel::Absent,
        "BoosterDraft has no rules implementation and must not be claimed"
    );
    assert_eq!(
        capabilities.format(FormatCapability::FreeForAll),
        SupportLevel::Modelled,
        "M5a's pod work -- seat-partitioned views and arity-general \
         orchestration -- is still outstanding"
    );
    assert_eq!(
        capabilities.feature(FeatureCapability::Teams),
        SupportLevel::Supported,
        "shared team life and the shared team turn are defended by \
         engine/tests/formats_m5.rs"
    );
    assert_eq!(
        capabilities.feature(FeatureCapability::ChosenAttackDefender),
        SupportLevel::Modelled,
        "the engine records a defender per attacker but rejects a combat that \
         splits across defenders, which is less than this capability promises"
    );
    assert_eq!(
        capabilities.feature(FeatureCapability::ExhaustiveLegalActions),
        SupportLevel::Absent,
        "no exhaustive enumeration exists yet"
    );
    assert_eq!(
        capabilities.feature(FeatureCapability::MultipleBlockers),
        SupportLevel::Absent
    );
}

#[test]
fn identities_round_trip() {
    round_trip(&match_id());
    round_trip(&SEAT_A);
    round_trip(&revision(42));
    round_trip(&ActionRequestId(9));
    round_trip(&DecisionRef(11));
    round_trip(&ObjectRef(1234));
    round_trip(&CardName::new("Birds of Paradise"));
    round_trip(&AbilityName::new("tap_for_any_color"));
}

#[test]
fn revision_display_is_stable_and_ordered() {
    assert_eq!(
        StateRevision::new(7, 0xabc).to_string(),
        "r7#0000000000000abc"
    );
    assert!(StateRevision::new(1, 9).precedes(StateRevision::new(2, 0)));
    assert!(!StateRevision::new(2, 0).precedes(StateRevision::new(1, 9)));
}

#[test]
fn observations_round_trip() {
    let observation = three_seat_observation();
    round_trip(&observation);
    round_trip(&observation.redacted_for(
        ViewerScope::Spectator,
        &cardbench_magic_protocol::ScopeMembership::none(),
    ));
}

#[test]
fn hidden_zone_projection_round_trips_in_both_shapes() {
    round_trip(&HiddenZoneProjection::Count(7));
    round_trip(&HiddenZoneProjection::Revealed(vec![support::card(
        1,
        "Watchwolf",
        SEAT_A,
    )]));
}

#[test]
fn terminal_results_round_trip_including_non_rules_outcomes() {
    for outcome in [
        TerminalOutcome::Win,
        TerminalOutcome::Draw,
        TerminalOutcome::Truncated,
    ] {
        round_trip(&TerminalResult {
            revision: revision(99),
            outcome,
            winning_seats: vec![SEAT_A],
            winning_teams: vec![cardbench_magic_protocol::TeamId(0)],
            eliminated_seats: vec![SEAT_B],
        });
    }
}

/// A truncated match is not a rules outcome and must never be reported as one.
#[test]
fn truncation_is_not_a_rules_outcome() {
    let truncated = TerminalResult {
        revision: revision(99),
        outcome: TerminalOutcome::Truncated,
        winning_seats: Vec::new(),
        winning_teams: Vec::new(),
        eliminated_seats: Vec::new(),
    };
    assert!(!truncated.is_rules_outcome());
    assert!(TerminalResult::draw(revision(99), vec![SEAT_A, SEAT_B]).is_rules_outcome());
}

#[test]
fn events_and_pages_round_trip() {
    let record = EventRecord::public(
        3,
        revision(3),
        "PolicyMoveSubmitted",
        vec![SEAT_A],
        "PolicyMoveSubmitted { player: PlayerId(0), kind: PassPriority }",
    );
    round_trip(&record);

    let private = EventRecord {
        sequence: 4,
        revision: revision(4),
        visibility: EventVisibility::Seats(BTreeSet::from([SEAT_A])),
        kind: "LibrarySearchResolved".to_owned(),
        seats: vec![SEAT_A],
        canonical: Some("LibrarySearchResolved { card: \"Chord of Calling\" }".to_owned()),
    };
    round_trip(&private);
    round_trip(&EventVisibility::JudgeOnly);
    round_trip(&EventVisibility::Team(cardbench_magic_protocol::TeamId(1)));

    round_trip(&EventStreamPage {
        from_sequence: 0,
        records: vec![record, private],
        revision: revision(4),
    });
}

#[test]
fn requests_round_trip() {
    let request = priority_request(SEAT_A, revision(5), 12);
    assert!(request.is_well_formed());
    round_trip(&request);

    let mut decision_request = priority_request(SEAT_B, revision(6), 13);
    decision_request.kind = ActionRequestKind::PendingDecision;
    decision_request.decision = Some(sample_prompt());
    assert!(decision_request.is_well_formed());
    round_trip(&decision_request);
}

/// A request whose kind and prompt disagree is a projection bug, not a
/// tolerable variation.
#[test]
fn request_shape_consistency_is_checked() {
    let mut malformed = priority_request(SEAT_A, revision(5), 12);
    malformed.kind = ActionRequestKind::PendingDecision;
    assert!(
        !malformed.is_well_formed(),
        "a decision request with no prompt must not pass"
    );

    let mut extraneous = priority_request(SEAT_A, revision(5), 12);
    extraneous.decision = Some(sample_prompt());
    assert!(
        !extraneous.is_well_formed(),
        "a priority request carrying a prompt must not pass"
    );
}

/// Every legal option must be self-identifying, so a cached option cannot be
/// replayed against a later state and look plausible.
#[test]
fn legal_options_carry_their_own_staleness_identity() {
    let mut request = priority_request(SEAT_A, revision(5), 12);
    request.legal = LegalActionSurface {
        exhaustive: false,
        options: vec![LegalActionOption {
            request: request.id,
            revision: request.revision,
            command: GameCommand::PassPriority,
            summary: "pass priority".to_owned(),
        }],
        unenumerated_kinds: vec![CommandKind::Cast, CommandKind::ActivateAbility],
    };
    assert!(request.is_well_formed());
    assert!(request.legal.contains(&GameCommand::PassPriority));
    round_trip(&request);

    let mut drifted = request.clone();
    drifted.legal.options[0].revision = revision(6);
    assert!(
        !drifted.is_well_formed(),
        "an option naming a different revision than its request must not pass"
    );
}

#[test]
fn command_results_round_trip() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let receipt = CommandReceipt {
        match_id: match_id(),
        seat: SEAT_A,
        request: request.id,
        client_command_id: cardbench_magic_protocol::ClientCommandId::new("cmd-1"),
        kind: CommandKind::PassPriority,
        previous_revision: revision(5),
        revision: revision(6),
        controller: "local:aggro-v2".to_owned(),
    };
    round_trip(&receipt);
    round_trip(&CommandResult::Accepted {
        receipt: receipt.clone(),
        events: vec![EventRecord::public(
            5,
            revision(6),
            "PriorityPassed",
            vec![SEAT_A],
            "PriorityPassed { player: PlayerId(0) }",
        )],
    });
    round_trip(&CommandResult::Duplicate { receipt });
    round_trip(&CommandResult::Rejected {
        error: ProtocolError::StaleRevision {
            observed: revision(4),
            current: revision(5),
        },
        revision: revision(5),
    });
}

#[test]
fn every_protocol_error_round_trips_and_renders() {
    let errors = [
        ProtocolError::SchemaMismatch {
            expected: PROTOCOL_SCHEMA_VERSION,
            actual: cardbench_magic_protocol::SchemaVersion::new(2, 0, 0),
        },
        ProtocolError::UnknownMatch {
            match_id: match_id(),
        },
        ProtocolError::MatchMismatch {
            expected: match_id(),
            actual: cardbench_magic_protocol::MatchId::new("other"),
        },
        ProtocolError::UnknownSeat { seat: SeatId(9) },
        ProtocolError::WrongSeat {
            expected: SEAT_A,
            actual: SEAT_B,
        },
        ProtocolError::StaleRevision {
            observed: revision(1),
            current: revision(2),
        },
        ProtocolError::StaleRequest {
            observed: ActionRequestId(1),
            current: ActionRequestId(2),
        },
        ProtocolError::StaleDecision {
            observed: Some(DecisionRef(1)),
            expected: Some(DecisionRef(2)),
        },
        ProtocolError::UnexpectedCommand {
            detail: "declared attackers outside combat".to_owned(),
        },
        ProtocolError::RulesRejected {
            detail: "target is no longer legal".to_owned(),
        },
        ProtocolError::Unsupported {
            capability: "Commander".to_owned(),
        },
    ];
    for error in &errors {
        round_trip(error);
        assert!(
            !error.to_string().is_empty(),
            "every rejection must be explainable to a client"
        );
    }
}

/// The command vocabulary is the widest surface, so it gets an explicit
/// per-variant round trip rather than a sample.
#[test]
fn every_command_variant_round_trips() {
    for command in sample_commands() {
        round_trip(&command);
    }
}

/// A command's declared kind must be total: no variant may fall through to a
/// wrong label, because receipts and coverage metrics are keyed on it.
#[test]
fn command_kinds_are_assigned_consistently() {
    let mut seen = BTreeSet::new();
    for command in sample_commands() {
        seen.insert(command.kind());
    }
    for expected in [
        CommandKind::PassPriority,
        CommandKind::Cast,
        CommandKind::CastWithMode,
        CommandKind::PlayLand,
        CommandKind::ActivateManaAbility,
        CommandKind::ActivateBoundManaAbility,
        CommandKind::ActivateAbility,
        CommandKind::Transmute,
        CommandKind::DeclareAttackers,
        CommandKind::DeclareBlockers,
        CommandKind::Draw,
        CommandKind::ChoosePrivateLibraryCards,
        CommandKind::ChoosePrivateOpponentLibraryCardToExile,
        CommandKind::ChooseLibrarySearchCard,
        CommandKind::ChooseTriggeredAbilityTargets,
        CommandKind::ChooseTriggeredAbilityEffectObject,
        CommandKind::ChooseDamageReplacement,
        CommandKind::ResolveOptionalTriggeredAbility,
        CommandKind::SubmitDecision,
        CommandKind::ReportEngineWeakness,
    ] {
        assert!(
            seen.contains(&expected),
            "{expected:?} has no sample command"
        );
    }
}

/// Exactly the commands that answer a rules decision report one. A command
/// that forgets to echo its decision would be accepted against a later
/// decision that happens to be of the same kind.
#[test]
fn decision_echo_is_present_exactly_where_required() {
    for command in sample_commands() {
        let expected = matches!(
            command.kind(),
            CommandKind::Draw
                | CommandKind::ChoosePrivateLibraryCards
                | CommandKind::ChoosePrivateOpponentLibraryCardToExile
                | CommandKind::ChooseLibrarySearchCard
                | CommandKind::ChooseTriggeredAbilityTargets
                | CommandKind::ChooseTriggeredAbilityEffectObject
                | CommandKind::ChooseDamageReplacement
                | CommandKind::ResolveOptionalTriggeredAbility
                | CommandKind::SubmitDecision
        );
        assert_eq!(
            command.decision().is_some(),
            expected,
            "{:?} reports the wrong decision-echo requirement",
            command.kind()
        );
    }
}

#[test]
fn every_decision_selection_variant_round_trips() {
    for selection in sample_selections() {
        round_trip(&GameCommand::SubmitDecision {
            decision: DecisionRef(1),
            selection,
        });
    }
}

#[test]
fn envelopes_round_trip_and_carry_full_attribution() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let envelope: CommandEnvelope = support::pass(&request, "cmd-1");
    assert_eq!(envelope.match_id, request.match_id);
    assert_eq!(envelope.seat, request.seat);
    assert_eq!(envelope.observed_revision, request.revision);
    assert_eq!(envelope.request, request.id);
    round_trip(&envelope);
}

/// A `MatchObservation` decoded from the wire must expose the same disclosure
/// set as the original. This is what lets the redaction tests trust
/// `disclosed_objects` after a transport hop.
#[test]
fn disclosure_survives_the_wire_unchanged() {
    let observation = three_seat_observation();
    let encoded = serde_json::to_string(&observation).expect("serialise");
    let decoded: MatchObservation = serde_json::from_str(&encoded).expect("deserialise");
    assert_eq!(observation.disclosed_objects(), decoded.disclosed_objects());
}

fn sample_prompt() -> DecisionPrompt {
    DecisionPrompt {
        decision: DecisionRef(21),
        kind: DecisionKindLabel::new("ChooseTargets"),
        visibility: DecisionVisibilityDto::Public,
        min_selections: 1,
        max_selections: 1,
        target_candidates: vec![TargetDto::Seat(SEAT_B), TargetDto::Permanent(ObjectRef(3))],
        candidates: vec![ObjectRef(3)],
        trigger_candidates: vec![TriggerOrderEntryDto {
            source: ObjectRef(4),
            source_incarnation: 2,
            ability: AbilityName::new("on_enter"),
            occurrence: 1,
        }],
        replacement_candidates: vec![ReplacementChoiceDto::Quantity {
            source: ObjectRef(5),
            source_incarnation: 1,
            effect: ReplacementEffectDto::MultiplyTokenCreation { multiplier: 2 },
        }],
        color_candidates: ColorDto::CARD_COLORS.to_vec(),
        card_name_candidates: vec![CardName::new("Lightning Helix")],
    }
}

fn cast_request() -> CastRequestDto {
    CastRequestDto {
        card: ObjectRef(10),
        targets: vec![TargetDto::Seat(SEAT_B)],
        convoke: vec![ConvokePaymentDto {
            creature: ObjectRef(11),
            contribution: ConvokeContributionDto::Color(ColorDto::Green),
        }],
        payment_mana_abilities: vec![
            PaymentManaAbilityDto::BasicLand {
                land: ObjectRef(12),
                color: ColorDto::Green,
            },
            PaymentManaAbilityDto::Bound(ManaAbilityActivationDto {
                source: ObjectRef(13),
                ability: AbilityName::new("tap_for_any"),
                chosen_color: Some(ColorDto::White),
            }),
        ],
    }
}

/// One command of every variant, split across two builders only to keep each
/// under the line budget.
fn sample_commands() -> Vec<GameCommand> {
    let mut commands = sample_cast_commands();
    commands.extend(sample_action_commands());
    commands.extend(sample_decision_commands());
    commands
}

fn sample_cast_commands() -> Vec<GameCommand> {
    vec![
        GameCommand::Cast(cast_request()),
        GameCommand::CastWithMode {
            request: cast_request(),
            mode: 1,
        },
        GameCommand::CastWithModeAndColorChoice {
            request: cast_request(),
            mode: 0,
            color: ColorDto::Blue,
        },
        GameCommand::CastWithPayment {
            request: cast_request(),
            chosen_x: Some(3),
            mana_selection: ManaPaymentSelectionDto {
                generic: vec![ColorDto::Green, ColorDto::Colorless],
                hybrid: vec![ColorDto::White],
            },
        },
        GameCommand::CastWithCreatureSpellAdditionalMana {
            request: cast_request(),
            chosen_x: None,
            mana_selection: ManaPaymentSelectionDto::default(),
            extra_payments: vec![cardbench_magic_protocol::ExtraManaPaymentDto {
                source: ObjectRef(30),
                colors: vec![ColorDto::Red],
            }],
        },
        GameCommand::CastWithColorChoice {
            request: cast_request(),
            color: ColorDto::Black,
        },
    ]
}

fn sample_action_commands() -> Vec<GameCommand> {
    let activation = AbilityActivationDto {
        source: ObjectRef(20),
        ability: AbilityName::new("pump"),
        sacrifice_sources: vec![ObjectRef(21)],
        additional_tap_creatures: vec![ObjectRef(22)],
        discard_cards: vec![ObjectRef(23)],
        targets: vec![TargetDto::Permanent(ObjectRef(24))],
    };
    vec![
        GameCommand::PassPriority,
        GameCommand::PlayLand {
            card: ObjectRef(40),
        },
        GameCommand::PlayLandWithEntryLifePayment {
            card: ObjectRef(41),
            pay_life: true,
        },
        GameCommand::ActivateManaAbility {
            land: ObjectRef(42),
            color: ColorDto::Green,
        },
        GameCommand::ActivateBoundManaAbility {
            activation: ManaAbilityActivationDto {
                source: ObjectRef(43),
                ability: AbilityName::new("filter"),
                chosen_color: None,
            },
        },
        GameCommand::ActivateBoundManaAbilityWithBundleChoice {
            activation: ManaAbilityActivationDto {
                source: ObjectRef(44),
                ability: AbilityName::new("bundle"),
                chosen_color: None,
            },
            chosen_bundle: cardbench_magic_protocol::ManaBundleDto {
                amounts: vec![cardbench_magic_protocol::ManaAmountDto {
                    color: ColorDto::Red,
                    amount: 1,
                }],
            },
        },
        GameCommand::ActivateAbility {
            activation: activation.clone(),
        },
        GameCommand::ActivateAbilityWithGeneralizedCosts {
            activation,
            cost_payment: cardbench_magic_protocol::AbilityCostPaymentDto {
                counter_sources: vec![ObjectRef(25)],
                return_permanents: vec![ObjectRef(26)],
                hand_cards_to_library_top: vec![ObjectRef(27)],
                graveyard_cards_to_exile: vec![ObjectRef(28)],
                chosen_x: Some(2),
            },
            mana_payment_selection: Some(ManaPaymentSelectionDto::default()),
        },
        GameCommand::Transmute {
            card: ObjectRef(50),
        },
        GameCommand::DeclareAttackers {
            attackers: vec![ObjectRef(60), ObjectRef(61)],
        },
        GameCommand::DeclareBlockers {
            assignments: vec![BlockAssignmentDto {
                attacker: ObjectRef(60),
                blocker: ObjectRef(70),
            }],
        },
        GameCommand::ReportEngineWeakness {
            code: "unrepresentable_replacement".to_owned(),
            detail: "two shields applied to one damage event".to_owned(),
        },
    ]
}

fn sample_decision_commands() -> Vec<GameCommand> {
    vec![
        GameCommand::Draw {
            decision: DecisionRef(80),
            dredge: Some(ObjectRef(81)),
        },
        GameCommand::ChoosePrivateLibraryCards {
            decision: DecisionRef(82),
            spell: ObjectRef(83),
            selected: vec![ObjectRef(84)],
        },
        GameCommand::ChoosePrivateOpponentLibraryCardToExile {
            decision: DecisionRef(85),
            source: ObjectRef(86),
            ability: AbilityName::new("exile_top"),
            selected: None,
        },
        GameCommand::ChooseLibrarySearchCard {
            decision: DecisionRef(87),
            source: ObjectRef(88),
            selected: Some(ObjectRef(89)),
        },
        GameCommand::ChooseTriggeredAbilityTargets {
            decision: DecisionRef(90),
            source: ObjectRef(91),
            ability: AbilityName::new("on_attack"),
            targets: vec![TargetDto::Seat(SEAT_A)],
        },
        GameCommand::ChooseTriggeredAbilityEffectObject {
            decision: DecisionRef(92),
            source: ObjectRef(93),
            ability: AbilityName::new("on_death"),
            selected: Some(ObjectRef(94)),
        },
        GameCommand::ChooseDamageReplacement {
            decision: DecisionRef(95),
            source: ObjectRef(96),
            source_incarnation: 3,
            target: TargetDto::Seat(SEAT_B),
            replacement: DamageReplacementChoiceDto::Redirect {
                id: 1,
                source: ObjectRef(96),
                protected: ObjectRef(97),
                destination: TargetDto::Permanent(ObjectRef(98)),
            },
        },
        GameCommand::ResolveOptionalTriggeredAbility {
            decision: DecisionRef(99),
            source: ObjectRef(100),
            ability: AbilityName::new("may_draw"),
            pay: true,
            target: Some(TargetDto::Seat(SEAT_A)),
        },
        GameCommand::SubmitDecision {
            decision: DecisionRef(101),
            selection: DecisionSelectionDto::Objects(vec![ObjectRef(102)]),
        },
    ]
}

fn sample_selections() -> Vec<DecisionSelectionDto> {
    vec![
        DecisionSelectionDto::Objects(vec![ObjectRef(1)]),
        DecisionSelectionDto::LibrarySearchAndCast {
            selected: Some(ObjectRef(2)),
            targets: vec![TargetDto::Seat(SEAT_A)],
        },
        DecisionSelectionDto::ExiledSpellCopyCast {
            card: Some(ObjectRef(3)),
            targets: vec![TargetDto::Permanent(ObjectRef(4))],
            mode: Some(1),
            color: Some(ColorDto::Red),
        },
        DecisionSelectionDto::LibraryTopPartition {
            hand: ObjectRef(5),
            top: Some(ObjectRef(6)),
            bottom: vec![ObjectRef(7), ObjectRef(8)],
        },
        DecisionSelectionDto::TargetPlayerLibraryTopReorder {
            top: vec![ObjectRef(9)],
            bottom: vec![ObjectRef(10)],
        },
        DecisionSelectionDto::Targets(vec![TargetDto::BasicLandType(BasicLandTypeDto::Forest)]),
        DecisionSelectionDto::TriggerOrder(vec![TriggerOrderEntryDto {
            source: ObjectRef(11),
            source_incarnation: 1,
            ability: AbilityName::new("trigger"),
            occurrence: 1,
        }]),
        DecisionSelectionDto::Replacements(vec![ReplacementChoiceDto::Damage(
            DamageReplacementChoiceDto::PermanentShield {
                permanent: ObjectRef(12),
            },
        )]),
        DecisionSelectionDto::Color(ColorDto::White),
        DecisionSelectionDto::CardName(CardName::new("Sensei's Divining Top")),
        DecisionSelectionDto::CounterUnlessPaysMana {
            pay: true,
            mana_abilities: vec![PaymentManaAbilityDto::BasicLand {
                land: ObjectRef(13),
                color: ColorDto::Blue,
            }],
            mana_selection: ManaPaymentSelectionDto::default(),
        },
        DecisionSelectionDto::CounterUnlessDiscardsHand { discard: false },
    ]
}
