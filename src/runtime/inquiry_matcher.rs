//! Content-based inquiry matching for raw VISCA protocol.
//!
//! This module provides type-safe correlation of raw VISCA inquiry replies
//! by parsing payload content against expected response types.

use std::collections::HashMap;

use crate::command::response::{parse_inquiry_payload, ViscaResponseType};

/// Result of attempting to resolve a raw inquiry ID from payload content.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveResult {
    /// Exactly one inquiry matches the payload.
    Unique(u32),
    /// Multiple inquiries of the same type match (ambiguous).
    Ambiguous(Vec<u32>),
    /// No inquiries match the payload.
    None,
}

/// Resolve a raw VISCA inquiry ID by matching payload content.
///
/// This function attempts to parse the payload against all in-flight inquiry
/// response types to find which inquiry the reply belongs to. It leverages
/// the crate's existing type-safe parsers for zero-allocation matching.
///
/// # Arguments
///
/// * `payload` - The raw inquiry reply payload bytes
/// * `in_flight` - Map of in-flight inquiry IDs to their expected response types
///
/// # Returns
///
/// * `ResolveResult::Unique(id)` - Exactly one inquiry matches
/// * `ResolveResult::Ambiguous(ids)` - Multiple inquiries match (same type)
/// * `ResolveResult::None` - No inquiries match the payload
pub fn resolve_raw_inquiry_id(
    payload: &[u8],
    in_flight: &HashMap<u32, ViscaResponseType>,
) -> ResolveResult {
    // Try parsing the payload against each expected response type
    let mut matches = in_flight
        .iter()
        .filter_map(|(id, response_type)| {
            // Use the existing zero-allocation parser
            // If parsing succeeds, this inquiry type matches the payload
            if parse_inquiry_payload(payload, response_type).is_ok() {
                Some(*id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Remove duplicates (defensive, shouldn't happen with unique IDs)
    matches.dedup();

    match matches.len() {
        0 => ResolveResult::None,
        1 => ResolveResult::Unique(matches[0]),
        _ => ResolveResult::Ambiguous(matches),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unique_match() {
        let mut in_flight = HashMap::new();
        in_flight.insert(1, ViscaResponseType::Power);
        in_flight.insert(2, ViscaResponseType::ZoomPosition);

        // Power inquiry response: 0x90 0x50 0x02 0xFF (power on)
        let power_payload = &[0x02];

        let result = resolve_raw_inquiry_id(power_payload, &in_flight);
        assert_eq!(result, ResolveResult::Unique(1));
    }

    #[test]
    fn test_no_match() {
        let mut in_flight = HashMap::new();
        in_flight.insert(1, ViscaResponseType::Power);
        in_flight.insert(2, ViscaResponseType::ZoomPosition);

        // Invalid payload that doesn't match any expected type
        let invalid_payload = &[0xAA, 0xBB, 0xCC];

        let result = resolve_raw_inquiry_id(invalid_payload, &in_flight);
        assert_eq!(result, ResolveResult::None);
    }

    #[test]
    fn test_ambiguous_match() {
        let mut in_flight = HashMap::new();
        // Two inquiries expecting the same response type
        in_flight.insert(1, ViscaResponseType::Power);
        in_flight.insert(2, ViscaResponseType::Power);

        // Power inquiry response
        let power_payload = &[0x02];

        let result = resolve_raw_inquiry_id(power_payload, &in_flight);
        if let ResolveResult::Ambiguous(ids) = result {
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&1));
            assert!(ids.contains(&2));
        } else {
            assert!(
                matches!(result, ResolveResult::Ambiguous(_)),
                "Expected Ambiguous result, got {:?}",
                result
            );
        }
    }

    #[test]
    fn test_different_length_payloads() {
        let mut in_flight = HashMap::new();
        in_flight.insert(1, ViscaResponseType::Power);
        in_flight.insert(2, ViscaResponseType::ZoomPosition);
        in_flight.insert(3, ViscaResponseType::FocusPosition);

        // Power response has 1 byte (0x02 = on, 0x03 = off)
        let power_payload = &[0x02];

        let result = resolve_raw_inquiry_id(power_payload, &in_flight);
        assert_eq!(result, ResolveResult::Unique(1));

        // Test with a 4-byte payload that could match multiple types
        in_flight.clear();
        in_flight.insert(1, ViscaResponseType::ZoomPosition);
        in_flight.insert(2, ViscaResponseType::Shutter);

        let position_payload = &[0x00, 0x00, 0x00, 0x00];

        let result2 = resolve_raw_inquiry_id(position_payload, &in_flight);
        // This should match at least one inquiry
        assert!(
            matches!(
                result2,
                ResolveResult::Unique(_) | ResolveResult::Ambiguous(_)
            ),
            "Should match at least one inquiry, got {:?}",
            result2
        );
    }

    #[test]
    fn test_ambiguous_position_inquiries() {
        let mut in_flight = HashMap::new();
        in_flight.insert(1, ViscaResponseType::ZoomPosition);
        in_flight.insert(2, ViscaResponseType::FocusPosition);

        // Both zoom and focus use 4 nibbles, so this payload is ambiguous
        let position_payload = &[0x00, 0x00, 0x00, 0x00];

        let result = resolve_raw_inquiry_id(position_payload, &in_flight);
        if let ResolveResult::Ambiguous(ids) = result {
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&1));
            assert!(ids.contains(&2));
        } else {
            assert!(
                matches!(result, ResolveResult::Ambiguous(_)),
                "Expected Ambiguous result for zoom/focus positions, got {:?}",
                result
            );
        }
    }
}
