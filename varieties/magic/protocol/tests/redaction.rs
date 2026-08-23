//! Milestone 1: hidden information does not cross a viewer boundary.
//!
//! These tests assert absence, which is the hard direction. Where possible
//! they scan the whole observation rather than named fields, so a future field
//! that forgets to redact fails an existing test instead of needing a new one.

mod support;

use cardbench_magic_protocol::{
    CardDto, CardName, EventRecord, EventVisibility, HiddenZoneProjection, ObjectRef,
    ScopeMembership, SeatId, StackItemKind, StackItemObservation, StackObjectRef, TeamId,
    ViewerScope,
};
use std::collections::BTreeSet;
use support::{
    SEAT_A, SEAT_B, SEAT_C, authoritative_observation, card, revision, three_seat_observation,
};

fn seat_scope(seat: SeatId) -> (ViewerScope, ScopeMembership) {
    (ViewerScope::Seat(seat), ScopeMembership::for_seat(seat))
}

/// Object ids of every card the fixture places in `seat`'s hidden zones.
fn hidden_objects_of(
    observation: &cardbench_magic_protocol::MatchObservation,
    seat: SeatId,
) -> Vec<ObjectRef> {
    let entry = observation.seat(seat).expect("seat exists");
    let mut hidden = Vec::new();
    for zone in [&entry.hand, &entry.library] {
        if let Some(cards) = zone.revealed() {
            hidden.extend(cards.iter().map(|card| card.object));
        }
    }
    hidden
}

#[test]
fn a_seat_sees_its_own_hidden_zones() {
    let (scope, membership) = seat_scope(SEAT_A);
    let projected = authoritative_observation().redacted_for(scope, &membership);
    let own = projected.seat(SEAT_A).expect("own seat");
    assert!(
        own.hand.revealed().is_some(),
        "a seat must see its own hand"
    );
    assert!(own.library.revealed().is_some());
    assert!(own.mana_pool.is_some());
}

/// The core leak test. Every object the opponent holds in a hidden zone must
/// be absent from the whole projected observation, not merely from the field a
/// test author remembered to check.
#[test]
fn a_seat_never_sees_an_opponents_hidden_zones() {
    let authoritative = authoritative_observation();
    let secrets = hidden_objects_of(&authoritative, SEAT_B);
    assert!(!secrets.is_empty(), "fixture must actually hide something");

    let (scope, membership) = seat_scope(SEAT_A);
    let projected = authoritative.redacted_for(scope, &membership);
    let disclosed = projected.disclosed_objects();
    for secret in secrets {
        assert!(
            !disclosed.contains(&secret),
            "{secret} leaked into seat A's observation"
        );
    }

    let opponent = projected.seat(SEAT_B).expect("opponent seat");
    assert!(opponent.hand.revealed().is_none());
    assert!(opponent.library.revealed().is_none());
    assert!(
        opponent.mana_pool.is_none(),
        "an opponent's floating mana is private"
    );
}

/// Counts survive redaction. Hand size is public information and a client that
/// cannot render it is wrong in the other direction.
#[test]
fn redaction_preserves_public_counts() {
    let authoritative = authoritative_observation();
    let expected_hand = authoritative.seat(SEAT_B).expect("seat").hand.count();
    let expected_library = authoritative.seat(SEAT_B).expect("seat").library.count();

    let (scope, membership) = seat_scope(SEAT_A);
    let projected = authoritative.redacted_for(scope, &membership);
    let opponent = projected.seat(SEAT_B).expect("seat");
    assert_eq!(opponent.hand.count(), expected_hand);
    assert_eq!(opponent.library.count(), expected_library);
    assert_eq!(opponent.life, 20);
    assert_eq!(opponent.battlefield.len(), 1, "the battlefield is public");
    assert!(opponent.battlefield[0].is_identified());
    assert_eq!(opponent.graveyard.len(), 1, "the graveyard is public");
    assert!(opponent.graveyard[0].is_identified());
}

/// A spectator sees no seat's private information, including its own — a
/// spectator has no seat.
#[test]
fn a_spectator_sees_no_private_information_at_all() {
    let authoritative = three_seat_observation();
    let projected = authoritative.redacted_for(ViewerScope::Spectator, &ScopeMembership::none());
    for seat in &projected.seats {
        assert!(
            seat.hand.revealed().is_none(),
            "{} hand leaked to a spectator",
            seat.seat
        );
        assert!(seat.library.revealed().is_none());
        assert!(seat.mana_pool.is_none());
    }
}

/// The redaction step is idempotent, so a projection applied twice — for
/// example by an orchestrator and again by a transport — cannot re-reveal.
#[test]
fn redaction_is_idempotent() {
    let (scope, membership) = seat_scope(SEAT_A);
    let once = authoritative_observation().redacted_for(scope, &membership);
    let twice = once.redacted_for(scope, &membership);
    assert_eq!(once, twice);
}

/// Narrowing must never widen. Re-projecting a seat's view for a *different*
/// seat cannot disclose anything the first projection had already removed.
#[test]
fn re_projecting_a_narrow_view_cannot_widen_it() {
    let (scope_a, membership_a) = seat_scope(SEAT_A);
    let (scope_b, membership_b) = seat_scope(SEAT_B);
    let for_a = authoritative_observation().redacted_for(scope_a, &membership_a);
    let then_for_b = for_a.redacted_for(scope_b, &membership_b);
    let b_secrets: BTreeSet<ObjectRef> = then_for_b.disclosed_objects().into_iter().collect();
    let a_disclosed: BTreeSet<ObjectRef> = for_a.disclosed_objects().into_iter().collect();
    assert!(
        b_secrets.is_subset(&a_disclosed),
        "re-projection disclosed objects the source view did not contain"
    );
}

/// A judge scope is the only way to obtain everything, and it must be
/// unreachable from an ordinary seat scope.
#[test]
fn only_authority_scopes_bypass_redaction() {
    assert!(ViewerScope::Judge.is_authoritative());
    assert!(ViewerScope::PostGameReplay.is_authoritative());
    assert!(!ViewerScope::Seat(SEAT_A).is_authoritative());
    assert!(!ViewerScope::Spectator.is_authoritative());
    assert!(!ViewerScope::Team(TeamId(0)).is_authoritative());

    let membership = ScopeMembership::none();
    for seat in [SEAT_A, SEAT_B, SEAT_C] {
        assert!(ViewerScope::Judge.may_see_private(seat, &membership));
        assert!(ViewerScope::PostGameReplay.may_see_private(seat, &membership));
        assert!(!ViewerScope::Spectator.may_see_private(seat, &membership));
    }
}

/// Team entitlement follows membership, not seat identity. This is the shape
/// Two-Headed Giant will need; it is exercised now so the predicate is not
/// written for the first time under format pressure.
#[test]
fn team_entitlement_follows_membership() {
    let team = ViewerScope::Team(TeamId(0));
    let membership = ScopeMembership::for_seats([SEAT_A, SEAT_B]);
    assert!(team.may_see_private(SEAT_A, &membership));
    assert!(team.may_see_private(SEAT_B, &membership));
    assert!(!team.may_see_private(SEAT_C, &membership));

    let projected = three_seat_observation().redacted_for(team, &membership);
    assert!(
        projected
            .seat(SEAT_A)
            .expect("seat")
            .hand
            .revealed()
            .is_some()
    );
    assert!(
        projected
            .seat(SEAT_B)
            .expect("seat")
            .hand
            .revealed()
            .is_some()
    );
    assert!(
        projected
            .seat(SEAT_C)
            .expect("seat")
            .hand
            .revealed()
            .is_none()
    );
}

/// A team scope has no single acting seat, so a command cannot be attributed
/// to it. Without this a teammate could act as the other.
#[test]
fn only_a_seat_scope_can_act() {
    assert_eq!(ViewerScope::Seat(SEAT_A).acting_seat(), Some(SEAT_A));
    assert_eq!(ViewerScope::Team(TeamId(0)).acting_seat(), None);
    assert_eq!(ViewerScope::Spectator.acting_seat(), None);
    assert_eq!(ViewerScope::Judge.acting_seat(), None);
    assert_eq!(ViewerScope::PostGameReplay.acting_seat(), None);
}

/// Redacting a card removes its characteristics, not just its name. A hidden
/// card whose colour and type are known is a partial reveal.
#[test]
fn a_redacted_card_discloses_no_characteristics() {
    let identified: CardDto = card(1, "Watchwolf", SEAT_A);
    assert!(identified.is_identified());
    assert!(!identified.colors.is_empty());
    assert!(!identified.card_types.is_empty());

    let hidden = identified.redacted();
    assert!(!hidden.is_identified());
    assert!(hidden.colors.is_empty());
    assert!(hidden.mana_colors.is_empty());
    assert!(hidden.card_types.is_empty());
    assert!(hidden.basic_land_type.is_none());
    assert!(!hidden.can_attack);
    assert_eq!(
        hidden.object, identified.object,
        "the positional identity must survive so the zone stays addressable"
    );
    assert_eq!(hidden.redacted(), hidden, "redaction is idempotent");
}

/// A revealed library top is public: it must survive redaction, or a client
/// cannot render an effect every player can see.
#[test]
fn a_publicly_revealed_card_survives_redaction() {
    let mut authoritative = authoritative_observation();
    let revealed = card(999, "Sensei's Divining Top", SEAT_B);
    authoritative
        .seats
        .iter_mut()
        .find(|seat| seat.seat == SEAT_B)
        .expect("seat")
        .revealed_library_top = Some(revealed);

    let (scope, membership) = seat_scope(SEAT_A);
    let projected = authoritative.redacted_for(scope, &membership);
    let top = projected
        .seat(SEAT_B)
        .expect("seat")
        .revealed_library_top
        .as_ref()
        .expect("public reveal survives");
    assert_eq!(top.definition, Some(CardName::new("Sensei's Divining Top")));
    assert!(projected.disclosed_objects().contains(&ObjectRef(999)));
}

/// The stack is public. A spell on the stack is visible to every viewer,
/// including one whose owner's hand is hidden.
#[test]
fn the_stack_is_public() {
    let mut authoritative = authoritative_observation();
    authoritative.stack.push(StackItemObservation {
        stack_object: StackObjectRef(1),
        controller: SEAT_B,
        kind: StackItemKind::Spell,
        card: Some(card(500, "Lightning Helix", SEAT_B)),
        ability: None,
        targets: Vec::new(),
    });

    let (scope, membership) = seat_scope(SEAT_A);
    let projected = authoritative.redacted_for(scope, &membership);
    assert!(projected.disclosed_objects().contains(&ObjectRef(500)));
}

/// An event a seat is not entitled to keeps its position and kind but loses
/// its payload. Dropping it outright would desynchronise the receiver's
/// replay cursor.
#[test]
fn a_private_event_is_blanked_rather_than_dropped() {
    let private = EventRecord {
        sequence: 12,
        revision: revision(12),
        visibility: EventVisibility::Seats(BTreeSet::from([SEAT_B])),
        kind: "LibrarySearchResolved".to_owned(),
        seats: vec![SEAT_B],
        canonical: Some("LibrarySearchResolved { card: \"Chord of Calling\" }".to_owned()),
    };

    let (scope, membership) = seat_scope(SEAT_A);
    assert!(!private.is_visible_to(scope, &membership));
    let projected = private.redacted_for(scope, &membership);
    assert_eq!(projected.sequence, 12, "the cursor position must survive");
    assert_eq!(projected.kind, "LibrarySearchResolved");
    assert!(
        projected.canonical.is_none(),
        "the payload named a private card"
    );
    assert!(
        projected.seats.is_empty(),
        "routing seats can themselves be a reveal"
    );

    let (owner_scope, owner_membership) = seat_scope(SEAT_B);
    assert!(private.is_visible_to(owner_scope, &owner_membership));
    assert_eq!(
        private.redacted_for(owner_scope, &owner_membership),
        private
    );
}

#[test]
fn a_public_event_reaches_every_scope() {
    let public = EventRecord::public(
        1,
        revision(1),
        "PolicyMoveSubmitted",
        vec![SEAT_A],
        "PolicyMoveSubmitted { player: PlayerId(0), kind: PassPriority }",
    );
    for (scope, membership) in [
        seat_scope(SEAT_A),
        seat_scope(SEAT_B),
        (ViewerScope::Spectator, ScopeMembership::none()),
        (ViewerScope::Judge, ScopeMembership::none()),
    ] {
        assert!(public.is_visible_to(scope, &membership));
        assert_eq!(public.redacted_for(scope, &membership), public);
    }
}

/// A judge-only event never reaches a playing seat, whatever its membership.
#[test]
fn a_judge_only_event_never_reaches_a_seat() {
    let judge_only = EventRecord {
        sequence: 3,
        revision: revision(3),
        visibility: EventVisibility::JudgeOnly,
        kind: "InvariantChecked".to_owned(),
        seats: Vec::new(),
        canonical: Some("InvariantChecked { digest: 0x1234 }".to_owned()),
    };
    for seat in [SEAT_A, SEAT_B, SEAT_C] {
        let (scope, membership) = seat_scope(seat);
        assert!(!judge_only.is_visible_to(scope, &membership));
        assert!(
            judge_only
                .redacted_for(scope, &membership)
                .canonical
                .is_none()
        );
    }
    assert!(!judge_only.is_visible_to(ViewerScope::Spectator, &ScopeMembership::none()));
    assert!(judge_only.is_visible_to(ViewerScope::Judge, &ScopeMembership::none()));
}

/// An eliminated seat stays addressable: its public board and provenance
/// remain projectable, and its hidden zones remain hidden.
#[test]
fn an_eliminated_seat_is_still_projected() {
    let (scope, membership) = seat_scope(SEAT_A);
    let projected = three_seat_observation().redacted_for(scope, &membership);
    let eliminated = projected
        .seat(SEAT_C)
        .expect("eliminated seat is still present");
    assert!(eliminated.eliminated);
    assert_eq!(eliminated.life, 0);
    assert!(eliminated.hand.revealed().is_none());
    assert_eq!(
        projected.living_opponents_of(SEAT_A).len(),
        1,
        "only the living opponent counts"
    );
    assert_eq!(projected.living_opponents_of(SEAT_A)[0].seat, SEAT_B);
}

/// `HiddenZoneProjection` cannot carry a count that disagrees with its
/// contents, which is the reason it is a sum type rather than a struct with
/// both fields.
#[test]
fn a_hidden_zone_count_always_matches_its_contents() {
    let revealed = HiddenZoneProjection::Revealed(vec![
        card(1, "A", SEAT_A),
        card(2, "B", SEAT_A),
        card(3, "C", SEAT_A),
    ]);
    assert_eq!(revealed.count(), 3);
    assert_eq!(revealed.redacted(), HiddenZoneProjection::Count(3));
    assert_eq!(revealed.redacted().count(), 3);
    assert!(revealed.redacted().revealed().is_none());
}
