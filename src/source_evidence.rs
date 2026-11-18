//! Typed links from built-in protocol facts to checked positive source rows.

/// An address-free catalogue prefix, including any fixed selector or wildcard
/// nibble. Its text form exists only in the checked evidence tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PacketPattern {
    bytes: [u8; 8],
    mask: [u8; 8],
    len: usize,
    selectors: Option<(usize, &'static [&'static [u8]])>,
}

impl PacketPattern {
    pub(crate) const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) const fn exact(bytes: &[u8]) -> Self {
        assert!(
            bytes.len() <= 8,
            "source packet prefix exceeds pattern capacity"
        );
        let mut pattern = Self {
            bytes: [0; 8],
            mask: [0; 8],
            len: bytes.len(),
            selectors: None,
        };
        let mut index = 0;
        while index < bytes.len() {
            pattern.bytes[index] = bytes[index];
            pattern.mask[index] = 0xff;
            index += 1;
        }
        pattern
    }

    pub(crate) const fn prefix_len(mut self, len: usize) -> Self {
        assert!(
            len <= self.len,
            "source prefix extends past its catalogue bytes"
        );
        self.len = len;
        self
    }

    pub(crate) const fn then(mut self, bytes: &[u8], mask: &[u8]) -> Self {
        assert!(
            bytes.len() == mask.len(),
            "source pattern bytes and mask differ in length"
        );
        assert!(
            self.len + bytes.len() <= 8,
            "source packet prefix exceeds pattern capacity"
        );
        let mut index = 0;
        while index < bytes.len() {
            assert!(
                matches!(mask[index] & 0x0f, 0 | 0x0f) && matches!(mask[index] >> 4, 0 | 0x0f),
                "a packet wildcard masks whole nibbles"
            );
            self.bytes[self.len + index] = bytes[index] & mask[index];
            self.mask[self.len + index] = mask[index];
            index += 1;
        }
        self.len += bytes.len();
        self
    }

    /// Appends one finite, exact selector family. Unlike a nibble wildcard,
    /// this cannot authorize a sibling action's selector.
    pub(crate) const fn select(mut self, choices: &'static [&'static [u8]]) -> Self {
        assert!(!choices.is_empty(), "a selector family is nonempty");
        assert!(self.selectors.is_none(), "one selector family per packet");
        let width = choices[0].len();
        assert!(width > 0 && self.len + width <= 8, "invalid selector width");
        let mut index = 0;
        while index < choices.len() {
            assert!(choices[index].len() == width, "selector widths differ");
            index += 1;
        }
        self.selectors = Some((self.len, choices));
        self.len += width;
        self
    }

    #[cfg(test)]
    fn variants(self) -> impl Iterator<Item = Self> {
        let count = self.selectors.map_or(1, |(_, choices)| choices.len());
        (0..count).map(move |index| {
            let mut pattern = self;
            if let Some((offset, choices)) = self.selectors {
                for (byte, value) in choices[index].iter().enumerate() {
                    pattern.bytes[offset + byte] = *value;
                    pattern.mask[offset + byte] = 0xff;
                }
                pattern.selectors = None;
            }
            pattern
        })
    }

    pub(crate) fn matches_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() >= self.len
            && (0..self.len).all(|index| {
                bytes[index] & self.mask[index] == self.bytes[index] & self.mask[index]
            })
            && self.selectors.is_none_or(|(offset, choices)| {
                choices
                    .iter()
                    .any(|selector| bytes.get(offset..offset + selector.len()) == Some(*selector))
            })
    }

    pub(crate) fn validate_frame(self, frame: &[u8], feature: &'static str) -> crate::Result<()> {
        if frame.get(1..).is_some_and(|body| self.matches_bytes(body)) {
            Ok(())
        } else {
            Err(crate::Error::InvalidRequest {
                reason: format!("{feature} encoded a packet outside its sourced register/selector")
                    .into(),
            })
        }
    }

    /// The checked table sometimes summarizes selector rows by their common
    /// register. Expand that summary over the finite noun-row packet set.
    #[cfg(test)]
    fn covers_pattern(self, pattern: Self) -> bool {
        pattern.variants().all(|actual| {
            self.variants().any(|declared| {
                declared.len <= actual.len
                    && (0..declared.len).all(|index| {
                        declared.mask[index] & actual.mask[index] == declared.mask[index]
                            && declared.bytes[index] & declared.mask[index]
                                == actual.bytes[index] & actual.mask[index]
                    })
            })
        })
    }
}

impl PartialEq for PacketPattern {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len
            && self.bytes[..self.len] == other.bytes[..other.len]
            && self.mask[..self.len] == other.mask[..other.len]
            && self.selectors == other.selectors
    }
}
impl Eq for PacketPattern {}

#[cfg(test)]
impl std::fmt::Display for PacketPattern {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (variant, pattern) in self.variants().enumerate() {
            if variant > 0 {
                formatter.write_str(";")?;
            }
            for index in 0..pattern.len {
                if index > 0 {
                    formatter.write_str(" ")?;
                }
                for shift in [4, 0] {
                    if pattern.mask[index] & (0xf << shift) == 0 {
                        formatter.write_str("p")?;
                    } else {
                        write!(formatter, "{:X}", (pattern.bytes[index] >> shift) & 0xf)?;
                    }
                }
            }
        }
        Ok(())
    }
}

use crate::profiles::ProfileId;

macro_rules! source_ids {
    ($($id:ident: [$($model:ident),*]),+ $(,)?) => {
        /// Reference identities and model scope in the checked-in source catalog.
        // Supplemental catalog entries are recorded without checked rows.
        #[allow(dead_code)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum SourceId { $($id),+ }

        impl SourceId {
            #[cfg(test)]
            pub(crate) const ALL: &'static [Self] = &[$(Self::$id),+];

            #[cfg(test)]
            pub(crate) const fn as_str(self) -> &'static str {
                match self { $(Self::$id => stringify!($id)),+ }
            }

            pub(crate) const fn models(self) -> &'static [ProfileId] {
                match self { $(Self::$id => &[$(ProfileId::$model),*]),+ }
            }
        }
    };
}
source_ids! {
    R1: [PtzOpticsG2, PtzOptics30x],
    R2: [],
    R3: [],
    R4: [],
    R5: [],
    R6: [],
    R7: [SonyFr7],
    R8: [SonyEviH100],
    R9: [],
    R10: [PtzOpticsG3],
    R11: [SonyBrcH900],
    R12: [SonyBrc300],
    R13: [],
    R14: [PtzOpticsG3],
    R15: [],
    R16: [],
    R17: [],
    R18: [],
    R19: [],
    R20: [PtzOpticsG2, PtzOpticsG3, PtzOptics30x],
    R21: [NearusBrc300],
    R22: [PtzOpticsG2, PtzOpticsG3, PtzOptics30x],
    B1: [PtzOpticsG2, PtzOptics30x],
}

impl SourceId {
    /// A reference contributing to the Generic VISCA intersection does not
    /// establish coverage on its own.
    const fn applies_to(self, model: ProfileId) -> bool {
        if matches!(model, ProfileId::GenericVisca) {
            return matches!(self, Self::R8 | Self::R12 | Self::R21);
        }
        let models = self.models();
        let mut index = 0;
        while index < models.len() {
            if models[index] as usize == model as usize {
                return true;
            }
            index += 1;
        }
        false
    }
}

/// One exact positive provenance row. The owning ledger or inquiry supplies
/// its packet pattern once, independently of how many references establish it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reference {
    pub(crate) id: SourceId,
    pub(crate) row: &'static str,
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
            if reference.row.is_empty() || reference.id.models().is_empty() {
                return false;
            }
            index += 1;
        }
        true
    }
    pub(crate) fn validate_for_profile(
        self,
        profile: &crate::profiles::ProfileSpec,
        feature: &'static str,
    ) -> crate::Result<()> {
        if self.covers_model(profile.capabilities().profile_id) {
            Ok(())
        } else {
            Err(crate::Error::FeatureNotSupported { feature })
        }
    }

    pub(crate) const fn covers_model(self, model: Option<ProfileId>) -> bool {
        // A custom profile is an explicit caller-owned protocol authority.
        // It does not acquire a vendor provenance claim from this catalog.
        let Some(model) = model else { return true };
        if !self.is_sourced() {
            return false;
        }
        let Self::References(references) = self else {
            return false;
        };
        if matches!(model, ProfileId::GenericVisca) {
            let mut evi = false;
            let mut brc = false;
            let mut nearus = false;
            let mut index = 0;
            while index < references.len() {
                match references[index].id {
                    SourceId::R8 => evi = true,
                    SourceId::R12 => brc = true,
                    SourceId::R21 => nearus = true,
                    _ => {}
                }
                index += 1;
            }
            return evi && brc && nearus;
        }
        let mut index = 0;
        while index < references.len() {
            if references[index].id.applies_to(model) {
                return true;
            }
            index += 1;
        }
        false
    }

    /// Every citation of a grant must apply to its enclosing profile.
    pub(crate) const fn supports_grant(self, model: ProfileId) -> bool {
        if !self.covers_model(Some(model)) {
            return false;
        }
        let Self::References(references) = self else {
            return false;
        };
        let mut index = 0;
        while index < references.len() {
            if !references[index].id.applies_to(model) {
                return false;
            }
            index += 1;
        }
        true
    }
}

/// Keep references next to the fact they justify. The source id owns its models.
macro_rules! source {
    ($id:ident, $row:literal $(,)?) => {
        $crate::source_evidence::source!([($id, $row)])
    };
    ([$(($id:ident, $row:literal)),+ $(,)?]) => {
        $crate::source_evidence::Source::References(&[
            $($crate::source_evidence::Reference {
                id: $crate::source_evidence::SourceId::$id,
                row: $row,
            }),+
        ])
    };
}
pub(crate) use source;

#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) mod checked {
    use std::collections::{HashMap, HashSet};

    use super::{PacketPattern, Source, SourceId};
    use crate::{capabilities::TypedSupportSurface, profiles::ProfileId};

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
                if columns.len() != 6 || columns[0] == "Evidence key" || columns[0].starts_with('-')
                {
                    continue;
                }
                let row = Row {
                    source: columns[1],
                    subject: columns[2],
                    packet: columns[3],
                    polarity: columns[4],
                    location: columns[5],
                };
                if rows.insert(columns[0], row).is_some() {
                    return Err(format!("duplicate checked evidence key {}", columns[0]));
                }
            }
            Ok(Self { catalog, rows })
        }

        /// Every checked evidence row must have a live code reference.
        pub(crate) fn require_complete(&self, used: &HashSet<&str>) -> Result<(), String> {
            let mut orphaned = self
                .rows
                .keys()
                .filter(|key| !used.contains(**key))
                .copied()
                .collect::<Vec<_>>();
            orphaned.sort_unstable();
            if orphaned.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "unreferenced checked source evidence: {}",
                    orphaned.join(", ")
                ))
            }
        }

        /// A grant's packet set is the union of the noun rows it permits on
        /// this model. The docs may abbreviate a selector family by register;
        /// expand that prefix over those rows and require equality both ways.
        pub(crate) fn validate_grant(
            &self,
            source: Source,
            surface: TypedSupportSurface,
            model: ProfileId,
        ) -> Result<(), String> {
            let subject = format!("grant.{surface:?}");
            if !source.supports_grant(model) {
                return Err(format!(
                    "{subject}: no source applies to model {model:?} for every grant reference"
                ));
            }
            self.validate(source, &subject, Some(model), None, None)?;
            // Compare the finite packet set one selector at a time. A source
            // may list those choices separately even when one noun row stores
            // them together in a compound selector pattern.
            let patterns: Vec<_> = grant_patterns(surface, model)
                .into_iter()
                .flat_map(PacketPattern::variants)
                .collect();
            if patterns.is_empty() {
                return Err(format!("{subject}: no gated packet rows for {model:?}"));
            }
            let Source::References(references) = source else {
                unreachable!()
            };
            let mut documented = Vec::new();
            for reference in references {
                let packet = self.rows[reference.row].packet;
                let source_packets = packet
                    .split(';')
                    .map(|packet| grant_pattern(surface, packet))
                    .collect::<Result<Vec<_>, _>>()?;
                let source_packets =
                    project_grant_summary(reference.row, surface, model, source_packets)?;
                for pattern in &source_packets {
                    if !patterns
                        .iter()
                        .any(|actual| pattern.covers_pattern(*actual))
                    {
                        return Err(format!(
                            "{subject}: {pattern} is not the packet of any gated row"
                        ));
                    }
                }
                documented.extend(source_packets);
            }
            for pattern in &patterns {
                if !documented
                    .iter()
                    .any(|declared| declared.covers_pattern(*pattern))
                {
                    return Err(format!(
                        "{subject}: gated packet {pattern} is missing from its source references"
                    ));
                }
            }
            Ok(())
        }

        pub(crate) fn validate(
            &self,
            source: Source,
            subject: &str,
            model: Option<ProfileId>,
            packet: Option<&str>,
            polarity: Option<&str>,
        ) -> Result<(), String> {
            let Source::References(references) = source else {
                return Err(format!("{subject}: Unsourced"));
            };
            if references.is_empty() {
                return Err(format!("{subject}: empty source list"));
            }
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
                    || row.location.is_empty()
                {
                    return Err(format!(
                        "{subject}: {} has a different source, subject, packet or location",
                        reference.row
                    ));
                }
                let applies = model.is_none_or(|model| reference.id.applies_to(model));
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
            }
            if !source.covers_model(model) {
                return Err(format!("{subject}: no source applies to model {model:?}"));
            }
            Ok(())
        }
    }

    fn grant_patterns(surface: TypedSupportSurface, model: ProfileId) -> Vec<PacketPattern> {
        use crate::command::{
            inquiry_structs::{BuiltinInquiryMetadata, BUILTIN_INQUIRIES},
            semantics::BuiltinCommand,
            surface::{row_gate, RowGate},
        };
        let mut patterns = Vec::new();
        let mut push = |pattern| {
            if !patterns.contains(&pattern) {
                patterns.push(pattern);
            }
        };
        for row in BuiltinCommand::ALL {
            if matches!(row_gate(*row), RowGate::Typed(gates) | RowGate::DomainAndTyped(_, gates) if gates.contains(surface))
                && row.source().covers_model(Some(model))
            {
                push(
                    row.packet()
                        .expect("a sourced command has its packet pattern"),
                );
            }
        }
        for BuiltinInquiryMetadata {
            gate,
            source,
            bytes,
            kind,
            ..
        } in BUILTIN_INQUIRIES
        {
            if matches!(gate, RowGate::Typed(gates) | RowGate::DomainAndTyped(_, gates) if gates.contains(surface))
                && source.covers_model(Some(model))
            {
                let profile = model.spec().expect("built-in source facts");
                let bytes =
                    crate::command::inquiry_structs::body_for_profile(*kind, bytes, &profile);
                push(PacketPattern::exact(bytes));
            }
        }
        // This is the one runtime-only, argument-dependent permission: it
        // refines ZoomPosition's optical-plus-digital domain rather than a
        // separate static noun row (#912).
        if surface == TypedSupportSurface::DigitalZoomRange {
            push(
                BuiltinCommand::ZoomPosition
                    .packet()
                    .expect("direct zoom pattern"),
            );
        }
        patterns
    }

    /// Exact exceptions for the historical checked table's aggregate summaries.
    /// These do not affect permission: the authoritative set is still derived
    /// from the noun gates above. Each exception names its original reference
    /// key and checks the entire expected summary before projecting it.
    fn project_grant_summary(
        key: &str,
        surface: TypedSupportSurface,
        model: ProfileId,
        documented: Vec<PacketPattern>,
    ) -> Result<Vec<PacketPattern>, String> {
        use crate::command::{inquiry_structs::BUILTIN_INQUIRIES, semantics::BuiltinCommand as C};
        use TypedSupportSurface as S;
        let command = |row: C| -> Result<PacketPattern, String> {
            if !row.source().covers_model(Some(model)) {
                return Err(format!("{key}: supplemental {row:?} has no model source"));
            }
            row.packet()
                .ok_or_else(|| format!("{key}: supplemental {row:?} has no packet"))
        };
        let inquiry = |name: &str| -> Result<PacketPattern, String> {
            let row = BUILTIN_INQUIRIES
                .iter()
                .find(|row| row.name == name)
                .ok_or_else(|| format!("{key}: unknown supplemental inquiry {name}"))?;
            if !row.source.covers_model(Some(model)) {
                return Err(format!("{key}: supplemental {name} has no model source"));
            }
            Ok(PacketPattern::exact(row.bytes))
        };
        let (required, expected, projected) = match key {
            "r1-ptzopticsg2-ptzopticsantiflicker-grant"
            | "r10-ptzopticsg3-ptzopticsantiflicker-grant"
            | "r1-ptzoptics30x-ptzopticsantiflicker-grant" => {
                // The summary omitted the separately sourced FlickerModeInquiry.
                let setter = command(C::AntiFlicker)?;
                (
                    S::AntiFlicker,
                    vec![setter],
                    vec![setter, inquiry("FlickerModeInquiry")?],
                )
            }
            "r8-sonyevih100-focusnearlimitinquiry-grant" => {
                // The old inquiry-only spelling omitted its gated setter (#912).
                let getter = inquiry("FocusNearLimitInquiry")?;
                (
                    S::FocusNearLimit,
                    vec![getter],
                    vec![getter, command(C::FocusNearLimit)?],
                )
            }
            "r1-ptzopticsg2-directzoom-grant"
            | "r14-ptzopticsg3-directzoom-grant"
            | "r1-ptzoptics30x-directzoom-grant"
            | "r7-sonyfr7-directzoom-grant"
            | "r8-sonyevih100-directzoom-grant"
            | "r12-sonybrc300-directzoom-grant"
            | "r21-nearusbrc300-directzoom-grant" => {
                // Readback is sourced baseline context, not a DirectZoom grant.
                let setter = command(C::ZoomPosition)?;
                (
                    S::DirectZoom,
                    vec![setter, inquiry("ZoomPositionInquiry")?],
                    vec![setter],
                )
            }
            "r8-sonyevih100-digitalzoomtoggle-grant"
            | "r12-sonybrc300-digitalzoomtoggle-grant"
            | "r21-nearusbrc300-digitalzoomtoggle-grant" => {
                // CAM_DZoomModeInq is raw-only in these source summaries. It
                // grants no typed method and derives its register from DZoom.
                let setter = command(C::DigitalZoom)?;
                let mut raw_inquiry = setter;
                raw_inquiry.bytes[0] = crate::command::bytes::constants::INQUIRY;
                (
                    S::DigitalZoomToggle,
                    vec![setter, raw_inquiry],
                    vec![setter],
                )
            }
            "r1-ptzopticsg2-usbaudio-grant" | "r1-ptzoptics30x-usbaudio-grant" => {
                // The obsolete 0A-category prefix in this aggregate summary
                // is not positive permission. Both actual sourced rows use
                // the catalogue's 2A-category packet and remain checked.
                let getter = inquiry("UsbAudioInquiry")?;
                let mut obsolete = getter.prefix_len(3);
                obsolete.bytes[0] &= !0x20;
                (
                    S::UsbAudio,
                    vec![obsolete, getter],
                    vec![getter, command(C::UsbAudioOn)?, command(C::UsbAudioOff)?],
                )
            }
            "r1-ptzopticsg2-ptzopticspresetrecallspeed-grant"
            | "r10-ptzopticsg3-ptzopticspresetrecallspeed-grant"
            | "r1-ptzoptics30x-ptzopticspresetrecallspeed-grant" => {
                // The summary omits the terminator that distinguishes this
                // short packet from the shared pan/tilt drive register.
                let setter = command(C::PresetRecallSpeed)?;
                (
                    S::PresetRecallSpeed,
                    vec![setter.prefix_len(3).then(&[0], &[0])],
                    vec![setter],
                )
            }
            _ => return Ok(documented),
        };
        if surface != required
            || documented.len() != expected.len()
            || !documented.iter().all(|pattern| expected.contains(pattern))
            || !expected.iter().all(|pattern| documented.contains(pattern))
        {
            return Err(format!(
                "{key}: named grant-summary exception packet set changed"
            ));
        }
        Ok(projected)
    }

    fn grant_pattern(surface: TypedSupportSurface, packet: &str) -> Result<PacketPattern, String> {
        // The checked table's two historical semantic qualifiers are explicit
        // exceptions to its byte grammar. Both point to the same sourced
        // register as the relevant noun row; they are not new packet bytes.
        let packet = match surface {
            TypedSupportSurface::RgbTuning => {
                packet.trim().strip_suffix(" tuning").unwrap_or(packet)
            }
            TypedSupportSurface::DigitalZoomRange => packet
                .trim()
                .strip_suffix(" digital positions")
                .unwrap_or(packet),
            _ => packet,
        };
        let mut bytes = Vec::new();
        let mut masks = Vec::new();
        for token in packet.split_whitespace() {
            if token.len() != 2 {
                return Err(format!("invalid checked grant packet token {token}"));
            }
            let mut byte = 0;
            let mut mask = 0;
            for nibble in token.chars() {
                byte <<= 4;
                mask <<= 4;
                if nibble != 'p' {
                    byte |= u8::try_from(
                        nibble
                            .to_digit(16)
                            .ok_or_else(|| format!("invalid checked packet nibble {nibble}"))?,
                    )
                    .expect("hex nibble");
                    mask |= 0xf;
                }
            }
            bytes.push(byte);
            masks.push(mask);
        }
        Ok(PacketPattern::exact(&[]).then(&bytes, &masks))
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
    use super::{checked::Index, Source, SourceId};
    use crate::{capabilities::TypedSupportSurface, profiles::ProfileId};
    use std::collections::HashSet;

    const FIXTURE: &str = "| R1 | primary |\n| R11 | primary |\nNo source documents 09 04 2B.\n<!-- BEGIN CHECKED SOURCE EVIDENCE -->\n| Evidence key | Source | Subject | Packet | Boolean reply | Location |\n| positive | R1 | inquiry.Power | 09 04 00 | on=02;off=03 | p. 1 |\n<!-- END CHECKED SOURCE EVIDENCE -->";

    #[test]
    fn every_checked_evidence_row_has_a_live_code_reference() {
        let index =
            Index::parse(include_str!("../docs/visca_reference.md")).expect("checked source index");
        let mut used = HashSet::new();
        let mut record = |source: Source| {
            if let Source::References(references) = source {
                used.extend(references.iter().map(|reference| reference.row));
            }
        };
        for row in crate::command::semantics::BuiltinCommand::ALL {
            record(row.source());
        }
        for inquiry in crate::command::inquiry_structs::BUILTIN_INQUIRIES {
            record(inquiry.source);
        }
        for profile in crate::camera::profiles::BUILTIN_PROFILE_FACTS {
            for (_, source) in profile.typed_sources {
                record(*source);
            }
        }
        index
            .require_complete(&used)
            .expect("every checked row must be named by code");
    }

    #[test]
    fn an_unreferenced_docs_row_is_rejected() {
        let index = Index::parse(FIXTURE).expect("fixture index");
        assert!(index
            .require_complete(&HashSet::new())
            .expect_err("an unreferenced row must fail")
            .contains("positive"));
        index
            .require_complete(&HashSet::from(["positive"]))
            .expect("the fixture's only row is referenced");
    }

    #[test]
    fn exact_selector_grants_require_every_choice_and_refuse_extra_packets() {
        let document = include_str!("../docs/visca_reference.md");
        for (key, source, model) in [
            (
                "r1-ptzopticsg2-pictureeffect-grant",
                source!(R1, "r1-ptzopticsg2-pictureeffect-grant"),
                ProfileId::PtzOpticsG2,
            ),
            (
                "r14-ptzopticsg3-pictureeffect-grant",
                source!(R14, "r14-ptzopticsg3-pictureeffect-grant"),
                ProfileId::PtzOpticsG3,
            ),
            (
                "r1-ptzoptics30x-pictureeffect-grant",
                source!(R1, "r1-ptzoptics30x-pictureeffect-grant"),
                ProfileId::PtzOptics30x,
            ),
        ] {
            let surface = TypedSupportSurface::PictureEffect;
            Index::parse(document)
                .expect("checked source index")
                .validate_grant(source, surface, model)
                .expect("separate exact selectors cover the compound noun row");
            let line = document
                .lines()
                .find(|line| line.starts_with(&format!("| {key} |")))
                .expect("authored picture-effect grant");
            let packet = "01 04 63 00;01 04 63 04;09 04 63";
            assert!(line.contains(packet), "{key} changed its exact packet set");
            for (changed_packet, expected_error) in [
                (
                    "01 04 63 00;09 04 63",
                    "gated packet 01 04 63 04 is missing",
                ),
                (
                    "01 04 63 04;09 04 63",
                    "gated packet 01 04 63 00 is missing",
                ),
                (
                    "01 04 63 00;01 04 63 04",
                    "gated packet 09 04 63 is missing",
                ),
                (
                    "01 04 63 00;01 04 63 04;01 04 63 05;09 04 63",
                    "01 04 63 05 is not the packet of any gated row",
                ),
            ] {
                let changed_line = line.replace(packet, changed_packet);
                let changed = document.replace(line, &changed_line);
                let error = Index::parse(&changed)
                    .expect("changed selector evidence parses")
                    .validate_grant(source, surface, model)
                    .expect_err("missing or extra grant selectors must be refused");
                assert!(error.contains(expected_error), "{key}: {error}");
            }
        }
    }

    #[test]
    fn a_grant_cannot_cite_a_packet_outside_its_noun_rows() {
        let document = FIXTURE
            .replace("inquiry.Power", "grant.Iris")
            .replace("09 04 00", "01 04 FF");
        let index = Index::parse(&document).expect("fixture index");
        let error = index
            .validate_grant(
                source!(R1, "positive"),
                TypedSupportSurface::Iris,
                ProfileId::PtzOpticsG2,
            )
            .expect_err("an alien grant packet must fail");
        assert!(error.contains("not the packet of any gated row"), "{error}");
    }

    #[test]
    fn split_fr7_grants_reject_other_families_and_models() {
        let document = include_str!("../docs/visca_reference.md");
        for (surface, source, key, foreign_packet) in [
            (
                TypedSupportSurface::IrisSteps,
                source!(R7, "r7-grant-irissteps"),
                "r7-grant-irissteps",
                "01 05 34",
            ),
            (
                TypedSupportSurface::AutoIris,
                source!(R7, "r7-grant-autoiris"),
                "r7-grant-autoiris",
                "01 7E 04 4B 02",
            ),
            (
                TypedSupportSurface::OnePushWhiteBalance,
                source!(R7, "r7-grant-onepushwhitebalance"),
                "r7-grant-onepushwhitebalance",
                "01 04 35 0A",
            ),
            (
                TypedSupportSurface::SeparatePresetSpeed,
                source!(R7, "r7-grant-separatepresetspeed"),
                "r7-grant-separatepresetspeed",
                "01 7E 04 1C",
            ),
            (
                TypedSupportSurface::SeparatePresetSpeed,
                source!(R7, "r7-grant-separatepresetspeed"),
                "r7-grant-separatepresetspeed",
                "09 7E 04 1C",
            ),
            (
                TypedSupportSurface::PresetRecallSpeed,
                source!(R7, "r7-grant-presetrecallspeed"),
                "r7-grant-presetrecallspeed",
                "01 7E 01 0B",
            ),
            (
                TypedSupportSurface::PresetRecallSpeedInquiry,
                source!(R7, "r7-grant-presetrecallspeedinquiry"),
                "r7-grant-presetrecallspeedinquiry",
                "09 7E 01 0B",
            ),
        ] {
            let index = Index::parse(document).expect("checked source index");
            index
                .validate_grant(source, surface, ProfileId::SonyFr7)
                .expect("the narrowed source grant covers exactly its model's noun rows");
            let error = index
                .validate_grant(source, surface, ProfileId::PtzOpticsG2)
                .expect_err("FR7 evidence cannot establish a PTZOptics grant");
            assert!(
                error.contains("no source applies to model"),
                "{key}: {error}"
            );

            let line = document
                .lines()
                .find(|line| line.starts_with(&format!("| {key} |")))
                .expect("exact split grant row");
            let packet = line
                .trim_matches('|')
                .split('|')
                .nth(3)
                .expect("packet column")
                .trim();
            let changed_line = line.replace(packet, &format!("{packet};{foreign_packet}"));
            let changed = document.replace(line, &changed_line);
            let index = Index::parse(&changed).expect("mutated source index");
            let error = index
                .validate_grant(source, surface, ProfileId::SonyFr7)
                .expect_err("a sibling family's packet cannot widen the grant");
            assert!(
                error.contains("not the packet of any gated row"),
                "{key}: {error}"
            );
        }
    }

    #[test]
    fn merged_noise_reduction_grants_require_both_source_halves() {
        let index =
            Index::parse(include_str!("../docs/visca_reference.md")).expect("checked source index");
        macro_rules! check {
            ($surface:ident, $model:ident, $query_source:ident, $query:literal,
             $control_source:ident, $control:literal) => {{
                let combined = source!([($query_source, $query), ($control_source, $control)]);
                index
                    .validate_grant(combined, TypedSupportSurface::$surface, ProfileId::$model)
                    .expect("a merged level family retains both checked source anchors");
                for incomplete in [
                    source!($query_source, $query),
                    source!($control_source, $control),
                ] {
                    let error = index
                        .validate_grant(
                            incomplete,
                            TypedSupportSurface::$surface,
                            ProfileId::$model,
                        )
                        .expect_err("neither inquiry nor control evidence alone covers the family");
                    assert!(
                        error.contains("missing from its source references"),
                        "{error}"
                    );
                }
            }};
        }
        check!(
            NoiseReduction2D,
            PtzOpticsG2,
            R1,
            "r1-ptzopticsg2-noisereduction2d-grant",
            R22,
            "r14-ptzopticsg2-noisereduction2dcontrol-grant"
        );
        check!(
            NoiseReduction3D,
            PtzOpticsG2,
            R1,
            "r1-ptzopticsg2-noisereduction3d-grant",
            R22,
            "r14-ptzopticsg2-noisereduction3dcontrol-grant"
        );
        check!(
            NoiseReduction2D,
            PtzOpticsG3,
            R14,
            "r14-ptzopticsg3-noisereduction2d-grant",
            R22,
            "r14-ptzopticsg3-noisereduction2dcontrol-grant"
        );
        check!(
            NoiseReduction3D,
            PtzOpticsG3,
            R14,
            "r14-ptzopticsg3-noisereduction3d-grant",
            R22,
            "r14-ptzopticsg3-noisereduction3dcontrol-grant"
        );
        check!(
            NoiseReduction2D,
            PtzOptics30x,
            R1,
            "r1-ptzoptics30x-noisereduction2d-grant",
            R22,
            "r14-ptzoptics30x-noisereduction2dcontrol-grant"
        );
        check!(
            NoiseReduction3D,
            PtzOptics30x,
            R1,
            "r1-ptzoptics30x-noisereduction3d-grant",
            R22,
            "r14-ptzoptics30x-noisereduction3dcontrol-grant"
        );
        check!(
            NoiseReduction2D,
            SonyEviH100,
            R8,
            "r8-sonyevih100-noisereduction2d-grant",
            R8,
            "r8-sonyevih100-noisereduction2dcontrol-grant"
        );
    }

    #[test]
    fn mode_only_and_aggregate_grant_rows_are_retired() {
        let document = include_str!("../docs/visca_reference.md");
        for key in [
            "r7-sonyfr7-autotrackingwhitebalance-grant",
            "r1-ptzoptics30x-standardwhitebalancemodes-grant",
            "r1-ptzopticsg2-standardwhitebalancemodes-grant",
            "r11-sonybrch900-standardwhitebalancemodes-grant",
            "r12-genericvisca-standardwhitebalancemodes-grant",
            "r12-sonybrc300-standardwhitebalancemodes-grant",
            "r14-ptzopticsg3-standardwhitebalancemodes-grant",
            "r21-genericvisca-standardwhitebalancemodes-grant",
            "r21-nearusbrc300-standardwhitebalancemodes-grant",
            "r8-genericvisca-standardwhitebalancemodes-grant",
            "r8-sonyevih100-standardwhitebalancemodes-grant",
            "r7-grant-sonyfr7iris",
            "r7-grant-sonyfr7whitebalance",
            "r7-grant-sonyfr7presetspeed",
        ] {
            assert!(
                !document
                    .lines()
                    .any(|line| line.starts_with(&format!("| {key} |"))),
                "retired grant row remains positive evidence: {key}"
            );
        }
        let index = Index::parse(document).expect("checked source index");
        let error = index
            .validate_grant(
                source!(R7, "r7-grant-sonyfr7whitebalance"),
                TypedSupportSurface::OnePushWhiteBalance,
                ProfileId::SonyFr7,
            )
            .expect_err("the old aggregate key must not alias the new one-shot grant");
        assert!(error.contains("not a positive evidence row"), "{error}");
    }

    #[test]
    fn white_balance_action_grants_exclude_separately_sourced_mode_selection() {
        use crate::command::semantics::BuiltinCommand;

        let document = include_str!("../docs/visca_reference.md");
        let index = Index::parse(document).expect("checked source index");
        for (key, source, surface, action, mode) in [
            (
                "r1-ptzopticsg2-colortemperature-grant",
                source!(R1, "r1-ptzopticsg2-colortemperature-grant"),
                TypedSupportSurface::ColorTemperature,
                "01 04 20",
                "01 04 35 20",
            ),
            (
                "r1-ptzopticsg2-onepushwhitebalance-grant",
                source!(R1, "r1-ptzopticsg2-onepushwhitebalance-grant"),
                TypedSupportSurface::OnePushWhiteBalance,
                "01 04 10 05",
                "01 04 35 03",
            ),
        ] {
            index
                .validate_grant(source, surface, ProfileId::PtzOpticsG2)
                .expect("the action alone exactly describes this optional grant");
            let line = document
                .lines()
                .find(|line| line.starts_with(&format!("| {key} |")))
                .expect("the authored action grant remains present");
            let changed_line =
                line.replace(&format!("| {action} |"), &format!("| {action};{mode} |"));
            assert_ne!(line, changed_line);
            let changed = document.replace(line, &changed_line);
            let error = Index::parse(&changed)
                .expect("adding a mode packet still parses")
                .validate_grant(source, surface, ProfileId::PtzOpticsG2)
                .expect_err("mode selection must not become optional action permission");
            assert!(error.contains("not the packet of any gated row"), "{error}");
        }
        // Narrowing the optional grant never retires the positive, model-scoped
        // mode evidence used by the ordinary white-balance mode inventory.
        for (row, packet) in [
            (BuiltinCommand::WhiteBalanceOnePush, "01 04 35 03"),
            (BuiltinCommand::WhiteBalanceColorTemperature, "01 04 35 20"),
        ] {
            let source = row.source();
            let references = match source {
                Source::References(references) => Some(references),
                Source::Unsourced => None,
            }
            .expect("white-balance modes retain explicit model evidence");
            for reference in references {
                for model in reference.id.models() {
                    index
                        .validate(
                            source,
                            &format!("command.{row:?}"),
                            Some(*model),
                            Some(packet),
                            None,
                        )
                        .expect("every individual mode packet keeps its positive source");
                }
            }
        }
    }

    #[test]
    fn named_grant_summary_exceptions_are_exact_and_do_not_follow_a_family() {
        const KEY: &str = "r1-ptzopticsg2-directzoom-grant";
        let document = include_str!("../docs/visca_reference.md");
        let line = document
            .lines()
            .find(|line| line.starts_with(&format!("| {KEY} |")))
            .expect("exact aggregate grant row");
        let columns = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        let source = source!(R1, "r1-ptzopticsg2-directzoom-grant");
        let setter = crate::command::semantics::BuiltinCommand::ZoomPosition
            .packet()
            .expect("zoom packet")
            .to_string();
        for packet in [setter.clone(), format!("{setter};09 04 FF")] {
            let changed_line = line.replace(columns[3], &packet);
            let changed = document.replace(line, &changed_line);
            let index = Index::parse(&changed).expect("changed document still parses");
            let error = index
                .validate_grant(
                    source,
                    TypedSupportSurface::DirectZoom,
                    ProfileId::PtzOpticsG2,
                )
                .expect_err("changed exception set must fail");
            assert!(error.contains("exception packet set changed"), "{error}");
        }
        let renamed = document.replace(KEY, "unrecognized-direct-zoom-summary");
        let index = Index::parse(&renamed).expect("renamed row still parses");
        let error = index
            .validate_grant(
                source!(R1, "unrecognized-direct-zoom-summary"),
                TypedSupportSurface::DirectZoom,
                ProfileId::PtzOpticsG2,
            )
            .expect_err("an exception must not follow the surface family to a new row");
        assert!(error.contains("not the packet of any gated row"), "{error}");
    }
    #[test]
    fn evidence_resolves_exact_ids_models_packets_and_polarity() {
        let index = Index::parse(FIXTURE).expect("fixture index");
        let source = source!(R1, "positive");
        assert_eq!(
            SourceId::R1.models(),
            &[ProfileId::PtzOpticsG2, ProfileId::PtzOptics30x]
        );
        assert!(source.supports_grant(ProfileId::PtzOptics30x));
        assert!(!source.supports_grant(ProfileId::SonyFr7));
        assert!(
            !source!([(R1, "positive"), (R11, "foreign")]).supports_grant(ProfileId::PtzOpticsG2)
        );
        let common = source!([(R8, "evi"), (R12, "brc"), (R21, "nearus")]);
        assert!(common.supports_grant(ProfileId::GenericVisca));
        assert!(!source!([(R8, "evi"), (R12, "brc")]).supports_grant(ProfileId::GenericVisca));
        assert!(
            !source!([(R8, "evi"), (R12, "brc"), (R21, "nearus"), (R1, "foreign")])
                .supports_grant(ProfileId::GenericVisca)
        );
        assert!(index
            .validate(
                source,
                "inquiry.Power",
                Some(ProfileId::PtzOpticsG2),
                Some("09 04 00"),
                Some("on=02;off=03")
            )
            .is_ok());
        assert!(index
            .validate(
                source,
                "inquiry.Power",
                Some(ProfileId::SonyFr7),
                None,
                None
            )
            .is_err());
        assert!(index
            .validate(source, "inquiry.Power", None, None, Some("on=03;off=02"))
            .is_err());
        assert!(index
            .validate(source!(R11, "positive"), "inquiry.Power", None, None, None)
            .is_err());
    }

    #[test]
    fn negative_mentions_and_unsourced_rows_cannot_be_evidence() {
        let index = Index::parse(FIXTURE).expect("fixture index");
        for source in [
            Source::Unsourced,
            source!(R1, "09 04 2B"),
            source!(R1, "positive"),
        ] {
            let error = index
                .validate(
                    source,
                    "inquiry.IrisControl",
                    Some(ProfileId::PtzOpticsG2),
                    Some("09 04 2B"),
                    None,
                )
                .expect_err("negative evidence must fail");
            assert!(error.contains("inquiry.IrisControl"));
        }
    }
    #[test]
    fn custom_authority_and_generic_intersection_are_distinct() {
        use crate::profiles::ProfileId;
        assert!(Source::Unsourced.covers_model(None));
        assert!(!Source::Unsourced.covers_model(Some(ProfileId::SonyFr7)));
        assert!(!source!(R8, "one-manufacturer").covers_model(Some(ProfileId::GenericVisca)));
        assert!(source!([(R8, "evi"), (R12, "brc300"), (R21, "nearus")])
            .covers_model(Some(ProfileId::GenericVisca)));
    }
    #[test]
    fn source_packet_patterns_match_exact_bytes_and_documented_socket_nibbles() {
        use super::PacketPattern;
        let exact = PacketPattern::exact(&[0x02]);
        assert!(exact.matches_bytes(&[0x02]));
        assert!(!exact.matches_bytes(&[0x03]));
        let variable = PacketPattern::exact(&[]).then(&[0x20], &[0xf0]);
        assert!(variable.matches_bytes(&[0x21]));
        assert!(variable.matches_bytes(&[0x27]));
        assert!(!variable.matches_bytes(&[0x31]));
        assert!(!variable.matches_bytes(&[]));
        assert_eq!(variable.to_string(), "2p");
        assert_eq!(
            PacketPattern::exact(&[1, 2, 3, 4]).prefix_len(3),
            PacketPattern::exact(&[1, 2, 3])
        );
    }
    #[test]
    fn stop_and_action_selectors_reject_sibling_frames() {
        use crate::command::semantics::BuiltinCommand as B;
        let stop = B::PanTiltStop.packet().expect("STOP source");
        let drive = B::PanTiltDrive.packet().expect("drive source");
        for pan in 1..=3 {
            for tilt in 1..=3 {
                let frame = [0x81, 1, 6, 1, 0x18, 0x14, pan, tilt, 0xff];
                assert_eq!(
                    stop.validate_frame(&frame, "STOP").is_ok(),
                    pan == 3 && tilt == 3
                );
                assert_eq!(
                    drive.validate_frame(&frame, "drive").is_ok(),
                    pan != 3 || tilt != 3
                );
            }
        }
        for (row, body, accepted) in [
            (B::PushAfPress, &[1, 0x7e, 4, 0x58][..], &[1][..]),
            (B::PushAfRelease, &[1, 0x7e, 4, 0x58][..], &[0][..]),
            (B::MenuDisplay, &[1, 6, 6][..], &[2, 3][..]),
            (B::MenuSelect, &[1, 6, 6][..], &[5][..]),
            (B::MenuCancel, &[1, 6, 6][..], &[4][..]),
            (B::PictureEffect, &[1, 4, 0x63][..], &[0, 4][..]),
        ] {
            for selector in 0..=u8::MAX {
                let frame = [vec![0x81], body.to_vec(), vec![selector, 0xff]].concat();
                assert_eq!(
                    row.packet()
                        .expect("selector pattern")
                        .validate_frame(&frame, "action")
                        .is_ok(),
                    accepted.contains(&selector),
                    "{row:?} selector {selector:02X}"
                );
            }
        }
    }

    #[test]
    fn encoded_command_selector_must_match_the_source_row() {
        let source = crate::command::semantics::BuiltinCommand::PowerOn
            .packet()
            .expect("power pattern");
        assert!(source
            .validate_frame(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xff], "power on")
            .is_ok());
        assert!(source
            .validate_frame(&[0x81, 0x01, 0x04, 0x00, 0x03, 0xff], "power on")
            .is_err());
        assert!(source
            .validate_frame(&[0x81, 0x01, 0x04, 0x66, 0x02, 0xff], "power on")
            .is_err());
        assert!(source.validate_frame(&[], "power on").is_err());
    }
}
