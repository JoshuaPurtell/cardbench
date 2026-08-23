//! Complete public RAV printing inventory with a fail-closed executable boundary.
//!
//! This module is deliberately a catalog, not an assertion that every printed RAV
//! card is playable in the current engine. A record is either mapped to a named
//! executable compatibility definition or carries the capability gap that prevents
//! it from being resolved. The engine only receives `CardDefinition` values from
//! `crate::card_definitions`, so a catalog-only record cannot become a blank spell.

use std::collections::{BTreeMap, BTreeSet};

/// Published main-set collector-number count for Ravnica: City of Guilds.
pub const RAV_MAIN_SET_EXPECTED_PRINTING_COUNT: usize = 306;

/// Number of distinct names after the twenty basic-land printings are collapsed.
pub const RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT: usize = 291;

/// Number of basic-land printings in the published main-set collector-number range.
pub const RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT: usize = 20;

const DEFAULT_CATALOG_ONLY_STATUS: &str = "catalog-only:card-specific-rules-not-implemented";

/// The checked-in public source snapshot. It intentionally contains only collector
/// numbers, names, and `CardBench` semantic-status declarations.
pub const RAV_MAIN_SET_CATALOG_MANIFEST: &str = include_str!("../catalog/rav_main_set.tsv");

/// A printed RAV card's relationship to the currently executable rules slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardSemanticStatus {
    /// The printing has a `CardBench` executable compatibility definition.
    ///
    /// This does not claim complete Oracle fidelity; that definition's
    /// `supported_rules` remains the authoritative slice boundary.
    ExecutableCompatibilitySlice { definition_id: &'static str },
    /// The printing is deliberately cataloged but cannot be submitted to the game.
    /// The named gap is intentionally public and fail-closed.
    CatalogOnly { capability_gap: &'static str },
}

impl CardSemanticStatus {
    #[must_use]
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::ExecutableCompatibilitySlice { .. })
    }
}

/// A public, minimal set-printing record. No card art, rules text, flavor text, or
/// hidden benchmark fixture is included.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogCard {
    pub collector_number: u16,
    pub name: &'static str,
    pub semantic_status: CardSemanticStatus,
}

/// A malformed checked-in inventory or a violation of its catalog contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogValidationError(pub String);

impl std::fmt::Display for CatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CatalogValidationError {}

/// A caller tried to turn a catalog-only printing into an executable definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogResolutionError {
    UnknownCollectorNumber(u16),
    CapabilityGap {
        collector_number: u16,
        name: &'static str,
        capability_gap: &'static str,
    },
}

impl std::fmt::Display for CatalogResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCollectorNumber(number) => {
                write!(formatter, "RAV collector number {number} is not cataloged")
            }
            Self::CapabilityGap {
                collector_number,
                name,
                capability_gap,
            } => write!(
                formatter,
                "RAV #{collector_number} `{name}` is catalog-only: missing capability `{capability_gap}`"
            ),
        }
    }
}

impl std::error::Error for CatalogResolutionError {}

/// Parses the checked-in public manifest without a runtime data dependency.
pub fn parse_rav_main_set_catalog() -> Result<Vec<CatalogCard>, CatalogValidationError> {
    let mut cards = Vec::with_capacity(RAV_MAIN_SET_EXPECTED_PRINTING_COUNT);
    for (line_number, line) in RAV_MAIN_SET_CATALOG_MANIFEST.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.splitn(3, '\t');
        let number = fields.next().unwrap_or_default();
        let name = fields.next().unwrap_or_default();
        // The checked-in manifest declares this default in its header. Leaving the
        // third column out is intentionally a *negative* declaration: the card is
        // catalog-only with this explicit capability gap, never a blank executable.
        let status = fields.next().unwrap_or(DEFAULT_CATALOG_ONLY_STATUS);
        let collector_number = number.parse().map_err(|error| {
            CatalogValidationError(format!(
                "catalog line {} has invalid collector number `{number}`: {error}",
                line_number + 1
            ))
        })?;
        if name.is_empty() {
            return Err(CatalogValidationError(format!(
                "catalog line {} has no card name",
                line_number + 1
            )));
        }
        let semantic_status = parse_semantic_status(status, line_number + 1)?;
        cards.push(CatalogCard {
            collector_number,
            name,
            semantic_status,
        });
    }
    Ok(cards)
}

/// Returns the complete checked-in RAV printing catalog.
///
/// The manifest is source-controlled and validated by tests, so parse failure is a
/// programmer error rather than a runtime recovery path.
#[must_use]
pub fn rav_main_set_catalog() -> Vec<CatalogCard> {
    parse_rav_main_set_catalog().expect("checked-in RAV catalog must parse")
}

/// Resolves a collector number only when the manifest explicitly maps it to an
/// executable definition. Catalog-only cards fail with their declared gap.
pub fn executable_definition_id_for_collector(
    collector_number: u16,
) -> Result<&'static str, CatalogResolutionError> {
    let card = rav_main_set_catalog()
        .into_iter()
        .find(|card| card.collector_number == collector_number)
        .ok_or(CatalogResolutionError::UnknownCollectorNumber(
            collector_number,
        ))?;
    match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Ok(definition_id),
        CardSemanticStatus::CatalogOnly { capability_gap } => {
            Err(CatalogResolutionError::CapabilityGap {
                collector_number: card.collector_number,
                name: card.name,
                capability_gap,
            })
        }
    }
}

/// Validates the public catalog snapshot: no collector number or non-basic name
/// omission/duplication is allowed, while four printings of each basic land are
/// explicitly expected.
pub fn validate_rav_main_set_catalog() -> Result<(), CatalogValidationError> {
    let cards = parse_rav_main_set_catalog()?;
    if cards.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT {
        return Err(CatalogValidationError(format!(
            "RAV catalog has {} printings; expected {}",
            cards.len(),
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        )));
    }

    let mut collector_numbers = BTreeSet::new();
    let mut names = BTreeMap::<&str, usize>::new();
    for card in &cards {
        if !collector_numbers.insert(card.collector_number) {
            return Err(CatalogValidationError(format!(
                "RAV collector number {} appears more than once",
                card.collector_number
            )));
        }
        *names.entry(card.name).or_default() += 1;
        match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
                if definition_id.is_empty() {
                    return Err(CatalogValidationError(format!(
                        "RAV #{} has an empty executable definition id",
                        card.collector_number
                    )));
                }
            }
            CardSemanticStatus::CatalogOnly { capability_gap: "" } => {
                return Err(CatalogValidationError(format!(
                    "RAV #{} `{}` lacks a declared capability gap",
                    card.collector_number, card.name
                )));
            }
            CardSemanticStatus::CatalogOnly { .. } => {}
        }
    }

    let expected_numbers = (1_u16
        ..=u16::try_from(RAV_MAIN_SET_EXPECTED_PRINTING_COUNT)
            .expect("RAV expected count fits u16"))
        .collect::<BTreeSet<_>>();
    if collector_numbers != expected_numbers {
        return Err(CatalogValidationError(
            "RAV collector numbers must be the complete published 1..=306 range".to_owned(),
        ));
    }

    let basic_lands = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
    for basic_land in basic_lands {
        if names.get(basic_land) != Some(&4) {
            return Err(CatalogValidationError(format!(
                "RAV basic land `{basic_land}` must have four printings"
            )));
        }
    }
    for (name, count) in &names {
        if *count > 1 && !basic_lands.contains(name) {
            return Err(CatalogValidationError(format!(
                "RAV non-basic card `{name}` appears {count} times"
            )));
        }
    }
    if names.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT {
        return Err(CatalogValidationError(format!(
            "RAV catalog has {} distinct names; expected {}",
            names.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        )));
    }
    let basic_land_printings = cards
        .iter()
        .filter(|card| basic_lands.contains(&card.name))
        .count();
    if basic_land_printings != RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT {
        return Err(CatalogValidationError(format!(
            "RAV catalog has {basic_land_printings} basic-land printings; expected {RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT}"
        )));
    }
    Ok(())
}

fn parse_semantic_status(
    status: &'static str,
    line_number: usize,
) -> Result<CardSemanticStatus, CatalogValidationError> {
    if let Some(definition_id) = status.strip_prefix("executable:") {
        if definition_id.is_empty() {
            return Err(CatalogValidationError(format!(
                "catalog line {line_number} has an empty executable definition id"
            )));
        }
        return Ok(CardSemanticStatus::ExecutableCompatibilitySlice { definition_id });
    }
    if let Some(capability_gap) = status.strip_prefix("catalog-only:") {
        if capability_gap.is_empty() {
            return Err(CatalogValidationError(format!(
                "catalog line {line_number} has an empty capability gap"
            )));
        }
        return Ok(CardSemanticStatus::CatalogOnly { capability_gap });
    }
    Err(CatalogValidationError(format!(
        "catalog line {line_number} has unknown semantic status `{status}`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_catalog_has_complete_unique_printing_coverage() {
        validate_rav_main_set_catalog().expect("RAV public catalog must be complete and unique");
    }

    #[test]
    fn executable_cards_resolve_to_their_explicit_definition() {
        assert_eq!(
            executable_definition_id_for_collector(42),
            Ok("RAV-COPY-ENCHANTMENT")
        );
    }
}
