//! Attribute-parsing policy shared by every macro in this crate.
//!
//! Each key may appear once across all of an item's helper attributes, and
//! every numeric VISCA byte is an unsuffixed integer literal in any Rust radix.

use proc_macro2::Span;
use quote::ToTokens;
use syn::{meta::ParseNestedMeta, spanned::Spanned, Error, LitInt, Result};

/// One attribute key that rejects a second occurrence, naming both spans.
pub(crate) struct AttrSlot<T> {
    entry: Option<(T, Span)>,
}

impl<T> Default for AttrSlot<T> {
    fn default() -> Self {
        Self { entry: None }
    }
}

impl<T> AttrSlot<T> {
    /// Stores the value for the key named by `meta`, rejecting a duplicate.
    pub(crate) fn set(
        &mut self,
        attribute: &str,
        meta: &ParseNestedMeta<'_>,
        value: T,
    ) -> Result<()> {
        let key_span = meta.path.span();
        if let Some((_, first)) = &self.entry {
            let key = key_name(meta);
            let mut error = Error::new(
                key_span,
                format!("duplicate `{attribute}` attribute `{key}`"),
            );
            error.combine(Error::new(*first, format!("first `{key}` specified here")));
            return Err(error);
        }
        self.entry = Some((value, key_span));
        Ok(())
    }

    /// The stored value, if the key was given.
    pub(crate) fn get(&self) -> Option<&T> {
        self.entry.as_ref().map(|(value, _)| value)
    }

    /// Consumes the slot, returning the stored value.
    pub(crate) fn into_value(self) -> Option<T> {
        self.entry.map(|(value, _)| value)
    }

    /// Consumes the slot, returning the stored value with its key's span.
    pub(crate) fn into_entry(self) -> Option<(T, Span)> {
        self.entry
    }
}

/// The diagnostic for a key the attribute does not define.
pub(crate) fn unknown_key(meta: &ParseNestedMeta<'_>, attribute: &str, expected: &str) -> Error {
    Error::new_spanned(
        &meta.path,
        format!(
            "unknown `{attribute}` attribute `{}`; expected {expected}",
            key_name(meta)
        ),
    )
}

/// The one diagnostic for a VISCA byte that is not an unsuffixed integer
/// literal in `0..=255`. `subject` names the value, for example "`opcode`".
pub(crate) fn invalid_u8_literal(tokens: impl ToTokens, subject: &str) -> Error {
    Error::new_spanned(
        tokens,
        format!("{subject} must be an unsuffixed integer literal in 0..=255"),
    )
}

/// Parses an unsuffixed integer literal in `0..=255`, in any radix.
pub(crate) fn parse_u8_literal(literal: &LitInt, subject: &str) -> Result<u8> {
    if !literal.suffix().is_empty() {
        return Err(invalid_u8_literal(literal, subject));
    }
    literal
        .base10_parse::<u8>()
        .map_err(|_| invalid_u8_literal(literal, subject))
}

fn key_name(meta: &ParseNestedMeta<'_>) -> String {
    meta.path.to_token_stream().to_string().replace(' ', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn literal(source: &str) -> LitInt {
        syn::parse_str(source).expect("integer literal")
    }

    #[test]
    fn every_radix_parses_to_the_same_byte() {
        for source in ["71", "0x47", "0o107", "0b0100_0111", "7_1"] {
            assert_eq!(
                parse_u8_literal(&literal(source), "`opcode`").ok(),
                Some(71)
            );
        }
    }

    #[test]
    fn suffixed_and_wide_literals_are_rejected_with_one_message() {
        for source in ["1u8", "0x100", "0b1_0000_0000", "256"] {
            let error = parse_u8_literal(&literal(source), "`opcode`")
                .expect_err("literal outside the policy");
            assert_eq!(
                error.to_string(),
                "`opcode` must be an unsuffixed integer literal in 0..=255"
            );
        }
    }
}
