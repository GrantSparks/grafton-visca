//! Typed links from built-in protocol facts to checked positive source rows.

macro_rules! source_ids {
    ($($id:ident),+ $(,)?) => {
        /// Reference identities in the checked-in source catalog.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum SourceId { $($id),+ }

        impl SourceId {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$id),+];

            pub(crate) const fn as_str(self) -> &'static str {
                match self { $(Self::$id => stringify!($id)),+ }
            }
        }
    };
}
source_ids! {
    R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13, R14,
    R15, R16, R17, R18, R19, R20, R21, B1,
}

/// One exact positive row, including the packet/fact that the row establishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reference {
    pub(crate) id: SourceId,
    pub(crate) row: &'static str,
    pub(crate) packet: &'static str,
    pub(crate) models: &'static [crate::camera::profiles::ProfileId],
}

/// Evidence is explicit even for a raw-only or decode-only fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Unsourced,
    References(&'static [Reference]),
}

impl Source {
    /// Whether this row has complete typed references. Profile grant macros
    /// assert this in a const context; runtime admission refuses `Unsourced`.
    pub(crate) const fn is_sourced(self) -> bool {
        let Self::References(references) = self else {
            return false;
        };
        if references.is_empty() {
            return false;
        }
        let mut index = 0;
        while index < references.len() {
            let reference = references[index];
            if reference.row.is_empty()
                || reference.packet.is_empty()
                || reference.models.is_empty()
                || reference.id.as_str().is_empty()
                || reference.id as usize >= SourceId::ALL.len()
            {
                return false;
            }
            index += 1;
        }
        true
    }
    pub(crate) fn validate_for_profile(
        self,
        profile: &crate::ProfileSpec,
        feature: &'static str,
    ) -> crate::Result<()> {
        if self.covers_model(profile.capabilities().profile_id) {
            Ok(())
        } else {
            Err(crate::Error::FeatureNotSupported { feature })
        }
    }

    /// Check the actual encoded opcode/selector against this row's positive
    /// reference. Values and framing widths have their own profile validators.
    pub(crate) fn validate_frame(
        self,
        profile: &crate::ProfileSpec,
        frame: &[u8],
        feature: &'static str,
    ) -> crate::Result<()> {
        let Some(model) = profile.capabilities().profile_id else {
            return Ok(());
        };
        let Self::References(references) = self else {
            return Err(crate::Error::FeatureNotSupported { feature });
        };
        let body = frame.get(1..).unwrap_or_default();
        let matches = references
            .iter()
            .filter(|reference| reference.models.contains(&model))
            .any(|reference| {
                let mut bytes = body.iter();
                reference
                    .packet
                    .split_whitespace()
                    .all(|token| bytes.next().is_some_and(|byte| token_matches(token, *byte)))
            });
        if matches {
            Ok(())
        } else {
            Err(crate::Error::InvalidRequest(
                format!("{feature} encoded a packet outside its sourced register/selector").into(),
            ))
        }
    }

    pub(crate) const fn covers_model(
        self,
        model: Option<crate::camera::profiles::ProfileId>,
    ) -> bool {
        // A custom profile is an explicit caller-owned protocol authority.
        // It does not acquire a vendor provenance claim from this catalog.
        let Some(model) = model else { return true };
        if !self.is_sourced() {
            return false;
        }
        let Self::References(references) = self else {
            return false;
        };
        let mut any = false;
        let mut evi = false;
        let mut brc = false;
        let mut nearus = false;
        let mut index = 0;
        while index < references.len() {
            let reference = references[index];
            let mut declared = 0;
            while declared < reference.models.len() {
                if reference.models[declared] as usize == model as usize {
                    any = true;
                    evi |= reference.id as usize == SourceId::R8 as usize;
                    brc |= reference.id as usize == SourceId::R12 as usize;
                    nearus |= reference.id as usize == SourceId::R21 as usize;
                }
                declared += 1;
            }
            index += 1;
        }
        if model as usize == crate::camera::profiles::ProfileId::GenericVisca as usize {
            evi && brc && nearus
        } else {
            any
        }
    }
}

fn token_matches(token: &str, byte: u8) -> bool {
    fn nibble(code: u8, actual: u8) -> bool {
        match code {
            b'0'..=b'9' => actual == code - b'0',
            b'A'..=b'F' => actual == code - b'A' + 10,
            b'a'..=b'f' => actual == code - b'a' + 10,
            b'p' => true,
            _ => false,
        }
    }
    match token.as_bytes() {
        [high, low] => nibble(*high, byte >> 4) && nibble(*low, byte & 0x0f),
        _ => false,
    }
}

/// Keep references next to the fact they justify, without repeating path names.
macro_rules! source {
    ($id:ident, [$($model:ident),+ $(,)?], $row:literal, $packet:literal) => {
        $crate::source_evidence::Source::References(&[
            $crate::source_evidence::Reference {
                id: $crate::source_evidence::SourceId::$id,
                row: $row,
                packet: $packet,
                models: &[$($crate::camera::profiles::ProfileId::$model),+],
            },
        ])
    };
    ([$(($id:ident, [$($model:ident),+], $row:literal, $packet:literal)),+ $(,)?]) => {
        $crate::source_evidence::Source::References(&[
            $($crate::source_evidence::Reference {
                id: $crate::source_evidence::SourceId::$id,
                row: $row,
                packet: $packet,
                models: &[$($crate::camera::profiles::ProfileId::$model),+],
            }),+
        ])
    };
}
pub(crate) use source;

#[cfg(test)]
pub(crate) mod checked {
    use std::collections::{HashMap, HashSet};

    use super::{Source, SourceId};

    const BEGIN: &str = "<!-- BEGIN CHECKED SOURCE EVIDENCE -->";
    const END: &str = "<!-- END CHECKED SOURCE EVIDENCE -->";

    /// Structured positive evidence. Narrative mentions and catalog prose do
    /// not enter this index, including sentences saying a packet is absent.
    #[derive(Debug)]
    pub(crate) struct Index<'a> {
        catalog: HashSet<&'a str>,
        rows: HashMap<&'a str, Row<'a>>,
    }

    #[derive(Debug)]
    struct Row<'a> {
        source: &'a str,
        models: &'a str,
        subject: &'a str,
        packet: &'a str,
        polarity: &'a str,
        location: &'a str,
    }

    impl<'a> Index<'a> {
        pub(crate) fn parse(reference: &'a str) -> Result<Self, String> {
            let mut catalog = HashSet::new();
            for line in reference.lines() {
                let columns = columns(line);
                if let Some(id) = columns.first() {
                    if id.len() > 1
                        && matches!(id.as_bytes()[0], b'R' | b'B')
                        && id.as_bytes()[1..].iter().all(u8::is_ascii_digit)
                    {
                        if !SourceId::ALL.iter().any(|known| known.as_str() == *id) {
                            return Err(format!(
                                "source catalog identity {id} is absent from SourceId"
                            ));
                        }
                        catalog.insert(*id);
                    }
                }
            }
            let evidence = reference
                .split_once(BEGIN)
                .ok_or("missing checked evidence table")?
                .1
                .split_once(END)
                .ok_or("unterminated checked evidence table")?
                .0;
            let mut rows = HashMap::new();
            for line in evidence.lines() {
                let columns = columns(line);
                if columns.len() != 7 || columns[0] == "Evidence key" || columns[0].starts_with('-')
                {
                    continue;
                }
                let row = Row {
                    source: columns[1],
                    models: columns[2],
                    subject: columns[3],
                    packet: columns[4],
                    polarity: columns[5],
                    location: columns[6],
                };
                if rows.insert(columns[0], row).is_some() {
                    return Err(format!("duplicate checked evidence key {}", columns[0]));
                }
            }
            Ok(Self { catalog, rows })
        }

        pub(crate) fn validate(
            &self,
            source: Source,
            subject: &str,
            model: Option<&str>,
            packet: Option<&str>,
            polarity: Option<&str>,
        ) -> Result<(), String> {
            let Source::References(references) = source else {
                return Err(format!("{subject}: Unsourced"));
            };
            if references.is_empty() {
                return Err(format!("{subject}: empty source list"));
            }
            let mut model_covered = model.is_none();
            for reference in references {
                let id = reference.id.as_str();
                if !self.catalog.contains(id) {
                    return Err(format!("{subject}: source {id} is absent from the catalog"));
                }
                let row = self.rows.get(reference.row).ok_or_else(|| {
                    format!(
                        "{subject}: {} is not a positive evidence row",
                        reference.row
                    )
                })?;
                if !row.source.split(',').any(|source| source.trim() == id)
                    || !row.subject.split(',').any(|item| item.trim() == subject)
                    || row.packet != reference.packet
                    || row.location.is_empty()
                {
                    return Err(format!(
                        "{subject}: {} has a different source, subject, packet or location",
                        reference.row
                    ));
                }
                for declared in reference.models {
                    if !row
                        .models
                        .split(',')
                        .any(|candidate| candidate.trim() == format!("{declared:?}"))
                    {
                        return Err(format!(
                            "{subject}: {} does not cover declared model {declared:?}",
                            reference.row
                        ));
                    }
                }
                let applies = model.is_none_or(|model| {
                    reference
                        .models
                        .iter()
                        .any(|declared| format!("{declared:?}") == model)
                });
                if applies && packet.is_some_and(|packet| row.packet != packet) {
                    return Err(format!(
                        "{subject}: {} does not establish packet {}",
                        reference.row,
                        packet.unwrap_or_default()
                    ));
                }
                if applies && polarity.is_some_and(|polarity| row.polarity != polarity) {
                    return Err(format!(
                        "{subject}: {} does not establish reply polarity {}",
                        reference.row,
                        polarity.unwrap_or_default()
                    ));
                }
                if model.is_some_and(|model| {
                    row.models
                        .split(',')
                        .any(|candidate| candidate.trim() == model)
                }) {
                    model_covered = true;
                }
            }
            if model_covered {
                Ok(())
            } else {
                Err(format!(
                    "{subject}: no source applies to model {}",
                    model.unwrap_or_default()
                ))
            }
        }
    }

    fn columns(line: &str) -> Vec<&str> {
        if !line.starts_with('|') {
            return Vec::new();
        }
        line.trim_matches('|').split('|').map(str::trim).collect()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{checked::Index, Source};

    const FIXTURE: &str = "| R1 | primary |\n| R11 | primary |\nNo source documents 09 04 2B.\n<!-- BEGIN CHECKED SOURCE EVIDENCE -->\n| Evidence key | Source | Models | Subject | Packet | Boolean reply | Location |\n| positive | R1 | GenericVisca | inquiry.Power | 09 04 00 | on=02;off=03 | p. 1 |\n<!-- END CHECKED SOURCE EVIDENCE -->";

    #[test]
    fn evidence_resolves_exact_ids_models_packets_and_polarity() {
        let index = Index::parse(FIXTURE).expect("fixture index");
        let source = source!(R1, [GenericVisca], "positive", "09 04 00");
        assert!(index
            .validate(
                source,
                "inquiry.Power",
                Some("GenericVisca"),
                Some("09 04 00"),
                Some("on=02;off=03")
            )
            .is_ok());
        assert!(index
            .validate(source, "inquiry.Power", Some("ModelB"), None, None)
            .is_err());
        assert!(index
            .validate(source, "inquiry.Power", None, None, Some("on=03;off=02"))
            .is_err());
        assert!(index
            .validate(
                source!(R11, [GenericVisca], "positive", "09 04 00"),
                "inquiry.Power",
                None,
                None,
                None
            )
            .is_err());
    }

    #[test]
    fn negative_mentions_and_unsourced_rows_cannot_be_evidence() {
        let index = Index::parse(FIXTURE).expect("fixture index");
        for source in [
            Source::Unsourced,
            source!(R1, [GenericVisca], "09 04 2B", "09 04 2B"),
            source!(R1, [GenericVisca], "positive", "09 04 2B"),
        ] {
            let error = index
                .validate(
                    source,
                    "inquiry.IrisControl",
                    Some("GenericVisca"),
                    Some("09 04 2B"),
                    None,
                )
                .expect_err("negative evidence must fail");
            assert!(error.contains("inquiry.IrisControl"));
        }
    }
    #[test]
    fn custom_authority_and_generic_intersection_are_distinct() {
        use crate::camera::profiles::ProfileId;
        assert!(Source::Unsourced.covers_model(None));
        assert!(!Source::Unsourced.covers_model(Some(ProfileId::SonyFr7)));
        assert!(!source!(R8, [GenericVisca], "one-manufacturer", "09 04 00")
            .covers_model(Some(ProfileId::GenericVisca)));
        assert!(source!([
            (R8, [GenericVisca], "evi", "09 04 00"),
            (R12, [GenericVisca], "brc300", "09 04 00"),
            (R21, [GenericVisca], "nearus", "09 04 00")
        ])
        .covers_model(Some(ProfileId::GenericVisca)));
    }
    #[test]
    fn source_packet_tokens_match_exact_bytes_and_documented_socket_nibbles() {
        assert!(super::token_matches("02", 0x02));
        assert!(!super::token_matches("02", 0x03));
        assert!(super::token_matches("2p", 0x21));
        assert!(!super::token_matches("2p", 0x31));
        assert!(!super::token_matches("xx", 0x21));
        assert!(!super::token_matches("", 0x21));
    }
    #[test]
    fn encoded_command_selector_must_match_the_source_row() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("FR7");
        let source = crate::command::semantics::BuiltinCommand::PowerOn.source();
        assert!(source
            .validate_frame(&profile, &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff], "power on")
            .is_ok());
        assert!(source
            .validate_frame(&profile, &[0x81, 0x01, 0x04, 0x00, 0x03, 0xff], "power on")
            .is_err());
        assert!(source
            .validate_frame(&profile, &[0x81, 0x01, 0x04, 0x66, 0x02, 0xff], "power on")
            .is_err());
        assert!(source.validate_frame(&profile, &[], "power on").is_err());
    }
}
