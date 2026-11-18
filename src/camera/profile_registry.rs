//! Crate-private built-in profile registry.
//!
//! This module is the source of truth for built-in profile facts. The macro at
//! the bottom expands those facts into the public zero-sized profile types,
//! profile ID/group methods, validated runtime profile facts, typed support markers, and
//! invariant tests.

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProfileEvidence {
    pub(crate) key: &'static str,
    pub(crate) rationale: &'static str,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinProfileFacts {
    pub(crate) id: super::ProfileId,
    pub(crate) group: super::ProfileGroup,
    pub(crate) type_name: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) vendor: &'static str,
    pub(crate) description: &'static str,
    pub(crate) envelope: crate::ProfileEnvelope,
    pub(crate) tcp: Option<u16>,
    pub(crate) udp: Option<u16>,
    pub(crate) serial: bool,
    pub(crate) default_camera_id: u8,
    pub(crate) inquiry_support: crate::capabilities::InquirySupport,
    pub(crate) position_inquiries: Option<crate::AffectedAxes>,
    pub(crate) image_base_support: bool,
    pub(crate) focus_zone_inquiry: bool,
    pub(crate) usb_audio: bool,
    pub(crate) typed_support: crate::capabilities::TypedSupportSet,
    pub(crate) typed_sources: &'static [(
        crate::capabilities::TypedSupportSurface,
        crate::source_evidence::Source,
    )],
    /// Surfaces whose dedicated metadata is advertised but whose typed marker
    /// is withheld; each needs an evidence entry saying why.
    pub(crate) withheld: crate::capabilities::TypedSupportSet,
    pub(crate) evidence: &'static [ProfileEvidence],
}

#[cfg(test)]
impl BuiltinProfileFacts {
    pub(crate) fn has_typed_support(
        self,
        surface: crate::capabilities::TypedSupportSurface,
    ) -> bool {
        self.typed_support.contains(surface)
    }
}

/// Acknowledgement deadline every built-in profile uses until per-profile
/// values are measured on hardware (#689). Operational tuning may lengthen it,
/// never shorten it.
pub(crate) const INTERIM_ACK_TIMEOUT_MS: u64 = 500;

/// Inquiry-response deadline every built-in profile uses until per-profile
/// values are measured on hardware (#689). Operational tuning may lengthen it,
/// never shorten it.
pub(crate) const INTERIM_INQUIRY_TIMEOUT_MS: u64 = 1000;

/// Projects only the transport block's facts into the public config value.
pub(crate) const fn transport_compatibility(
    tcp: Option<u16>,
    udp: Option<u16>,
    serial: bool,
) -> crate::profiles::TransportCompatibility {
    let mut transports = crate::profiles::TransportCompatibility::new();
    if let Some(port) = tcp {
        transports = transports.with_tcp(port);
    }
    if let Some(port) = udp {
        transports = transports.with_udp(port);
    }
    if serial {
        transports = transports.with_serial();
    }
    transports
}

/// A command-category deadline table written in milliseconds, for the rows
/// whose table differs from [`crate::profiles::CommandTimeouts::DEFAULT`].
pub(crate) const fn command_timeouts_ms(
    quick: u64,
    movement: u64,
    preset: u64,
    long_running: u64,
    network: u64,
) -> crate::CommandTimeouts {
    use std::time::Duration;

    crate::CommandTimeouts::DEFAULT
        .with_quick_timeout(Duration::from_millis(quick))
        .with_movement_timeout(Duration::from_millis(movement))
        .with_preset_timeout(Duration::from_millis(preset))
        .with_long_running_timeout(Duration::from_millis(long_running))
        .with_network_timeout(Duration::from_millis(network))
}

/// A per-profile fact a profile group reports for every member.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberFact {
    SonyEncapsulation,
    Tcp,
    Udp,
    Serial,
}

/// The weaker of two inquiry support levels.
pub(crate) const fn weaker_inquiry_support(
    left: crate::capabilities::InquirySupport,
    right: crate::capabilities::InquirySupport,
) -> crate::capabilities::InquirySupport {
    use crate::capabilities::InquirySupport::{Full, None, Partial};

    match (left, right) {
        (None, _) | (_, None) => None,
        (Partial, _) | (_, Partial) => Partial,
        (Full, Full) => Full,
    }
}

macro_rules! __individual_camera_id {
    ($value:expr) => {{
        #[allow(clippy::panic)]
        const ID: $crate::IndividualCameraId = {
            let checked = $crate::IndividualCameraId::new($value);
            match &checked {
                Ok(id) => {
                    let id = *id;
                    ::core::mem::forget(checked);
                    id
                }
                Err(_) => {
                    ::core::mem::forget(checked);
                    panic!("profile default must be an individual camera ID")
                }
            }
        };
        ID
    }};
}

macro_rules! range {
    ($ty:ty, $min:expr, $max:expr) => {
        const { $crate::capabilities::CapabilityRange::<$ty>::from_registry($min, $max) }
    };
}

/// A table-sourced value domain: its bounds and, when the table skips
/// positions, the unlisted ones.
macro_rules! domain {
    ($ty:ty, $min:expr, $max:expr) => {
        const { $crate::capabilities::CapabilityDomain::<$ty>::from_registry($min, $max, &[]) }
    };
    ($ty:ty, $min:expr, $max:expr, gaps: [$($gap:expr),+ $(,)?]) => {
        const { $crate::capabilities::CapabilityDomain::<$ty>::from_registry($min, $max, &[$($gap),+]) }
    };
}

macro_rules! __impl_supports_tcp {
    ($profile:ty, None) => {};
    ($profile:ty, Some($port:expr)) => {
        impl $crate::capabilities::SupportsTcp for $profile {}
    };
}
macro_rules! __impl_supports_udp {
    ($profile:ty, None) => {};
    ($profile:ty, Some($port:expr)) => {
        impl $crate::capabilities::SupportsUdp for $profile {}
    };
}
macro_rules! __impl_supports_serial {
    ($profile:ty, false) => {};
    ($profile:ty, true) => {
        impl $crate::capabilities::SupportsSerial for $profile {}
    };
}

/// Join the profile-specific const diagnostic with its missing row name.
pub(super) const fn missing_source_message<const N: usize>(prefix: &str, row: &str) -> [u8; N] {
    let mut bytes = [0; N];
    let mut index = 0;
    while index < prefix.len() {
        bytes[index] = prefix.as_bytes()[index];
        index += 1;
    }
    let mut offset = 0;
    while offset < row.len() {
        bytes[index + offset] = row.as_bytes()[offset];
        offset += 1;
    }
    bytes
}

/// Report a missing evidence row where its marker is granted.
macro_rules! __assert_fixed_source {
    ($profile:ident, $label:ident, $gate:expr) => {
        #[allow(clippy::panic)]
        const _: () = {
            const PREFIX: &str = concat!(
                stringify!($profile),
                " ",
                stringify!($label),
                " lacks checked source evidence for "
            );
            const MISSING: &str = match $crate::command::surface::uncovered_fixed_source(
                ProfileId::$profile,
                $gate,
            ) {
                Some(row) => row,
                None => "",
            };
            if !MISSING.is_empty() {
                const BYTES: [u8; PREFIX.len() + MISSING.len()] =
                    profile_registry::missing_source_message(PREFIX, MISSING);
                match std::str::from_utf8(&BYTES) {
                    Ok(message) => panic!("{}", message),
                    Err(_) => panic!("invalid const source evidence diagnostic"),
                }
            }
        };
    };
}

/// Optional record presence is the sole domain-marker authority.
macro_rules! __impl_domain_marker {
    (None for $profile:ident, $marker:ident) => {};
    ({$($facts:tt)*} for $profile:ident, $marker:ident) => {
        impl $crate::capabilities::$marker for $profile {}
        __assert_fixed_source!(
            $profile,
            $marker,
            $crate::command::surface::RowGate::Domain($crate::command::surface::domain_gate!(
                $marker
            ))
        );
    };
}
#[cfg(test)]
macro_rules! __assert_domain_marker {
    (None for $profile:ty, $assert_marker:ident) => {};
    ({$($facts:tt)*} for $profile:ty, $assert_marker:ident) => {
        $assert_marker::<$profile>();
    };
}
macro_rules! __domain_present {
    (None) => {
        false
    };
    ({$($facts:tt)*}) => {
        true
    };
}
#[cfg(test)]
macro_rules! __focus_zone_fact {
    (None) => { false };
    ({ zone_inquiry: $value:expr, $($rest:tt)* }) => { $value };
    ({ $key:ident: $value:expr, $($rest:tt)* }) => { __focus_zone_fact!({$($rest)*}) };
}

/// Leaves are typed Rust expressions; the record keys are direct serde keys.
/// Struct literal exhaustiveness catches a new missing or unknown record fact.
macro_rules! __optional_record {
    ($record:ident, None) => { None };
    ($record:ident, {$( $key:ident: $value:expr, )*}) => {
        Some($record { $( $key: $value, )* })
    };
}
macro_rules! __pan_tilt_facts {
    (None) => { None };
    ({
        geometry: $geometry:tt,
        $( $key:ident: $value:expr, )*
    }) => {
        Some($crate::capabilities::PanTiltCaps {
            geometry: __pan_tilt_geometry!($geometry),
            $( $key: $value, )*
        })
    };
}
macro_rules! __pan_tilt_geometry {
    (None) => {
        None
    };
    ({
        pan_range: $pan_range:expr,
        tilt_ranges: $tilt_ranges:expr,
        coordinates: $coordinates:tt,
        endpoint_quantization: $quantization:expr,
    }) => {
        Some($crate::capabilities::PanTiltGeometry {
            pan_range: $pan_range,
            tilt_ranges: $tilt_ranges,
            coordinates: __coordinate_conversion!($coordinates),
            endpoint_quantization: $quantization,
        })
    };
}
macro_rules! __coordinate_conversion {
    ({
        coordinate_system: $system:expr,
        wire_codec: $codec:expr,
        pan_degrees_to_units: $pan_scale:expr,
        tilt_degrees_to_units: $tilt_scale:expr,
    }) => {
        $crate::profiles::PanTiltCoordinateConversion::from_registry(
            $system,
            $codec,
            $pan_scale,
            $tilt_scale,
        )
    };
}
// Project the same registry coordinates without constructing heap-owning facts.
macro_rules! __pan_tilt_coordinates {
    (None) => { None };
    ({ geometry: $geometry:tt, $($rest:tt)* }) => {
        __pan_tilt_coordinates!(@geometry $geometry)
    };
    (@geometry None) => { None };
    (@geometry {
        pan_range: $pan_range:expr,
        tilt_ranges: $tilt_ranges:expr,
        coordinates: $coordinates:tt,
        endpoint_quantization: $quantization:expr,
    }) => {
        Some(__coordinate_conversion!($coordinates))
    };
}

macro_rules! __white_balance_facts {
    (None) => {
        None
    };
    ({
        modes: $modes:expr,
        one_push: $one_push:expr,
        color_temp_range: $range:expr,
        color_temperature_steps: $steps:expr,
        rgb_gain: $gain:tt,
        rgb_tuning: $tuning:tt,
    }) => {
        Some($crate::capabilities::WhiteBalanceCaps {
            modes: $modes,
            one_push: $one_push,
            color_temp_range: $range,
            color_temperature_steps: $steps,
            rgb_gain: __optional_record!(RgbGainRanges, $gain),
            rgb_tuning: __optional_record!(RgbTuningRanges, $tuning),
        })
    };
}

macro_rules! define_typed_support_marker_impls {
    (
        $dollar:tt
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    $(encodings: [$($encoding_variant:ident => $encoding_tag:literal),+ $(,)?],)?
                    surface_doc: $surface_doc:literal,
                    marker_doc: $marker_doc:literal,
                    diagnostic: $diagnostic:literal,
                },
            )*
        ]
    ) => {
        macro_rules! __impl_typed_support_marker {
            $(
                ($surface for $dollar profile:ty) => {
                    impl $crate::capabilities::$marker for $dollar profile {}
                };
            )*
        }
    };
}

crate::capabilities::typed_support_registry::typed_support_registry!(
    define_typed_support_marker_impls,
    $
);
/// A row's record keys, read from its authored fact tokens rather than a
/// separate inventory. Braced records are structural nodes; typed Rust value
/// expressions (ranges, durations, arrays, checked scalars) are opaque leaves.
#[cfg(all(test, feature = "serde"))]
macro_rules! __row_fact_keys {
    (@fields $keys:ident,) => {};
    (@fields $keys:ident, $key:ident: { $($nested:tt)* }, $($rest:tt)*) => {
        $keys.insert(stringify!($key).into(), __row_fact_keys!({$($nested)*}));
        __row_fact_keys!(@fields $keys, $($rest)*);
    };
    (@fields $keys:ident, $key:ident: $value:expr, $($rest:tt)*) => {
        $keys.insert(stringify!($key).into(), serde_json::Value::Null);
        __row_fact_keys!(@fields $keys, $($rest)*);
    };
    ({$($fields:tt)*}) => {{
        #[allow(unused_mut)]
        let mut keys = serde_json::Map::new();
        __row_fact_keys!(@fields keys, $($fields)*);
        serde_json::Value::Object(keys)
    }};
}

/// Compare only fact-record keys. A declared typed leaf retains its own serde
/// contract instead of requiring a second DSL for Duration, ranges or tables.
#[cfg(all(test, feature = "serde"))]
pub(crate) fn check_fact_keys(
    row: &serde_json::Value,
    exported: &serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let Some(expected) = row.as_object() else {
        return Ok(());
    };
    let actual = exported
        .as_object()
        .ok_or_else(|| format!("{path}: expected a fact record"))?;
    for key in actual.keys() {
        if !expected.contains_key(key) {
            return Err(format!(
                "{path}/{key}: serialized fact has no registry-row key"
            ));
        }
    }
    for (key, child) in expected {
        let actual_child = actual
            .get(key)
            .ok_or_else(|| format!("{path}/{key}: registry-row key has no serialized fact"))?;
        check_fact_keys(child, actual_child, &format!("{path}/{key}"))?;
    }
    Ok(())
}

macro_rules! __define_builtin_profiles {
    (
        groups $groups:tt
        profiles {$(profile $profile:ident {
            doc: $doc:literal,
            id: $id:literal,
            id_doc: $id_doc:literal,
            vendor: $vendor:literal,
            description: $description:literal,
            facts: $facts:tt,
            withheld: $withheld:tt,
            evidence: $evidence:tt,
        })*}
    ) => {
        __define_builtin_profiles!(@emit groups $groups profiles {$(profile $profile {
            doc: $doc, id: $id, id_doc: $id_doc, vendor: $vendor, description: $description,
            facts: $facts, withheld: $withheld, evidence: $evidence,
        })*});
        #[cfg(all(test, feature = "serde"))]
        impl ProfileId {
            fn row_fact_keys(self) -> serde_json::Value {
                let mut keys = match self {$(Self::$profile => __row_fact_keys!($facts),)*};
                keys["encodings"] = self.row_encoding_keys();
                keys
            }
        }
    };
    (
        @emit
        groups {
            $(group $group:ident {
                doc: $group_doc:literal,
                display: $group_display:literal,
                id: $group_id:literal,
                profiles: [$( $group_profile:ident ),* $(,)?],
            })*
        }
        profiles {
            $(profile $profile:ident {
                doc: $profile_doc:literal,
                id: $profile_id:literal,
                id_doc: $id_doc:literal,
                vendor: $vendor:literal,
                description: $description:literal,
                facts: {
                    capabilities: {
                        profile_id: from_id,
                        model_name: $model_name:literal,
                        default_camera_id: $default_camera_id:expr,
                        pan_tilt: $pan_tilt_facts:tt,
                        zoom: $zoom_facts:tt,
                        focus: $focus_facts:tt,
                        exposure: $exposure_facts:tt,
                        white_balance: $white_balance_facts:tt,
                        image: $image_facts:tt,
                        presets: $preset_facts:tt,
                        power: $power_facts:tt,
                        menu: $menu_facts:tt,
                        inquiry_support: $inquiry_support:expr,
                        typed_support: grants([$(($support:ident $(($support_encoding:ident))?, $support_source:expr)),* $(,)?]),
                    },
                    encodings: from_grants,
                    transports: {
                        tcp_port: $tcp:ident $(($tcp_port:expr))?,
                        udp_port: $udp:ident $(($udp_port:expr))?,
                        serial: $serial:tt,
                    },
                    envelope: $envelope:expr,
                    timing: {$( $timing_key:ident: $timing_value:expr, )*},
                    maximum_command_sockets: $maximum_command_sockets:expr,
                    supports_operation_complete: $supports_operation_complete:expr,
                    supports_command_cancel: $supports_command_cancel:expr,
                    position_inquiries: $position_inquiries:expr,
                },
                withheld: [$( $withheld:ident ),* $(,)?],
                evidence: [$(($evidence_key:literal, $evidence_text:literal)),* $(,)?],
            })*
        }
    ) => {
        $(
            #[doc = $profile_doc]
            #[derive(Debug, Default, Clone, Copy)]
            pub struct $profile;

            impl $crate::profiles::CompileTimeProfile for $profile {
                const TYPED_SUPPORT: $crate::capabilities::TypedSupportSet =
                    $crate::capabilities::TypedSupportSet::from_surfaces(&[
                        $($crate::capabilities::TypedSupportSurface::$support,)*
                    ]);
                const DOMAINS: $crate::capabilities::DomainSet = {
                    use $crate::capabilities::{Domain, DomainSet};
                    let mut domains = DomainSet::empty();
                    if __domain_present!($power_facts) { domains = domains.with(Domain::Power); }
                    if __domain_present!($zoom_facts) { domains = domains.with(Domain::Zoom); }
                    if __domain_present!($focus_facts) { domains = domains.with(Domain::Focus); }
                    if __domain_present!($pan_tilt_facts) { domains = domains.with(Domain::PanTilt); }
                    if __domain_present!($preset_facts) { domains = domains.with(Domain::Presets); }
                    if __domain_present!($exposure_facts) { domains = domains.with(Domain::Exposure); }
                    if __domain_present!($white_balance_facts) { domains = domains.with(Domain::WhiteBalance); }
                    if __domain_present!($menu_facts) { domains = domains.with(Domain::Menu); }
                    if __domain_present!($image_facts) { domains = domains.with(Domain::Image); }
                    domains
                };
                const TRANSPORTS: $crate::capabilities::TransportSet =
                    ProfileId::$profile.transport_compatibility().transport_set();
                const ID: Option<ProfileId> = Some(ProfileId::$profile);

                fn spec() -> $crate::Result<$crate::ProfileSpec> {
                    ProfileId::$profile.spec()
                }
            }

            __assert_fixed_source!($profile, always, $crate::command::surface::RowGate::Always);
            __impl_domain_marker!($power_facts for $profile, HasPower);
            __impl_domain_marker!($zoom_facts for $profile, HasZoom);
            __impl_domain_marker!($focus_facts for $profile, HasFocus);
            __impl_domain_marker!($pan_tilt_facts for $profile, HasPanTilt);
            __impl_domain_marker!($preset_facts for $profile, HasPresets);
            __impl_domain_marker!($exposure_facts for $profile, HasExposure);
            __impl_domain_marker!($white_balance_facts for $profile, HasWhiteBalance);
            __impl_domain_marker!($menu_facts for $profile, HasMenu);
            __impl_domain_marker!($image_facts for $profile, HasImage);
            __impl_supports_tcp!($profile, $tcp $(($tcp_port))?);
            __impl_supports_udp!($profile, $udp $(($udp_port))?);
            __impl_supports_serial!($profile, $serial);
            $(
                const _: () = assert!(
                    $support_source.supports_grant(ProfileId::$profile),
                    concat!(stringify!($profile), " grant ", stringify!($support),
                        " requires source evidence for its model")
                );
                __impl_typed_support_marker!($support for $profile);
            )*
        )*

        /// Profile group for runtime polymorphism and profile dispatch.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
        #[non_exhaustive]
        pub enum ProfileGroup {
            $(
                #[doc = $group_doc]
                #[cfg_attr(feature = "serde", serde(rename = $group_id))]
                $group,
            )*
        }

        /// Serializable camera profile identifier.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
        #[non_exhaustive]
        pub enum ProfileId {
            $(
                #[doc = $id_doc]
                #[cfg_attr(feature = "serde", serde(rename = $profile_id))]
                $profile,
            )*
        }

        #[cfg(test)]
        pub(crate) const BUILTIN_PROFILE_FACTS: &[profile_registry::BuiltinProfileFacts] = &[
            $(
                profile_registry::BuiltinProfileFacts {
                    id: ProfileId::$profile,
                    group: ProfileId::$profile.profile_group(),
                    type_name: stringify!($profile),
                    display_name: $model_name,
                    vendor: $vendor,
                    description: $description,
                    envelope: if ProfileId::$profile.uses_sony_encapsulation() {
                        $crate::ProfileEnvelope::SonyEncapsulated
                    } else {
                        $crate::ProfileEnvelope::RawVisca
                    },
                    tcp: $tcp $(($tcp_port))?,
                    udp: $udp $(($udp_port))?,
                    serial: $serial,
                    default_camera_id: $default_camera_id,
                    inquiry_support: $inquiry_support,
                    position_inquiries: $position_inquiries,
                    image_base_support: __domain_present!($image_facts),
                    focus_zone_inquiry: __focus_zone_fact!($focus_facts),
                    usb_audio: <$profile as $crate::profiles::CompileTimeProfile>::TYPED_SUPPORT.contains($crate::capabilities::TypedSupportSurface::UsbAudio),
                    typed_support: <$profile as $crate::profiles::CompileTimeProfile>::TYPED_SUPPORT,
                    typed_sources: &[$(($crate::capabilities::TypedSupportSurface::$support, $support_source),)*],
                    withheld: $crate::capabilities::TypedSupportSet::from_surfaces(&[
                        $($crate::capabilities::TypedSupportSurface::$withheld,)*
                    ]),
                    evidence: &[
                        $(profile_registry::ProfileEvidence {
                            key: $evidence_key,
                            rationale: $evidence_text,
                        },)*
                    ],
                },
            )*
        ];

        impl ProfileId {
            /// Resolves this built-in profile through strict profile validation.
            pub fn spec(self) -> $crate::Result<$crate::ProfileSpec> {
                $crate::ProfileSpec::from_facts(self.raw_facts())
            }

            /// Registry grants without constructing owned profile facts.
            pub(crate) const fn typed_support(self) -> $crate::capabilities::TypedSupportSet {
                match self { $(Self::$profile => <$profile as $crate::profiles::CompileTimeProfile>::TYPED_SUPPORT,)* }
            }

            /// Present domain records, without constructing owned profile facts.
            pub(crate) const fn domains(self) -> $crate::capabilities::DomainSet {
                match self { $(Self::$profile => <$profile as $crate::profiles::CompileTimeProfile>::DOMAINS,)* }
            }

            /// Registry framing without constructing owned profile facts.
            pub(crate) const fn pan_tilt_coordinates(self) -> Option<$crate::profiles::PanTiltCoordinateConversion> {
                match self { $(Self::$profile => __pan_tilt_coordinates!($pan_tilt_facts),)* }
            }

            /// Returns the pinned snake_case identifier used in configurations.
            pub const fn as_str(&self) -> &'static str {
                match self { $(Self::$profile => $profile_id,)* }
            }

            /// Direct unvalidated source facts for construction and identity
            /// comparison. This path never calls `spec`, so validation cannot
            /// recurse when checking a persisted built-in identity.
            pub(crate) fn raw_facts(self) -> $crate::profiles::ProfileFacts {
                use $crate::capabilities::{
                    Capabilities, ExposureCaps, FocusCaps, ImageCaps, MenuCaps, PowerCaps,
                    PresetCaps, RgbGainRanges, RgbTuningRanges, ZoomCaps,
                };
                match self {$(Self::$profile => $crate::profiles::ProfileFacts {
                    capabilities: Capabilities {
                        profile_id: Some(Self::$profile),
                        model_name: $model_name.into(),
                        default_camera_id: self.default_camera_id(),
                        pan_tilt: __pan_tilt_facts!($pan_tilt_facts),
                        zoom: __optional_record!(ZoomCaps, $zoom_facts),
                        focus: __optional_record!(FocusCaps, $focus_facts),
                        exposure: __optional_record!(ExposureCaps, $exposure_facts),
                        white_balance: __white_balance_facts!($white_balance_facts),
                        image: __optional_record!(ImageCaps, $image_facts),
                        presets: __optional_record!(PresetCaps, $preset_facts),
                        power: __optional_record!(PowerCaps, $power_facts),
                        menu: __optional_record!(MenuCaps, $menu_facts),
                        inquiry_support: $inquiry_support,
                        typed_support: <$profile as $crate::profiles::CompileTimeProfile>::TYPED_SUPPORT,
                    },
                    encodings: $crate::capabilities::ProfileEncodings::EMPTY
                        $($( .with_encoding($crate::capabilities::encoding_selection!($support($support_encoding))) )?)*,
                    transports: profile_registry::transport_compatibility(
                        $tcp $(($tcp_port))?, $udp $(($udp_port))?, $serial,
                    ),
                    envelope: $envelope,
                    timing: $crate::profiles::ProfileTiming {$( $timing_key: $timing_value, )*},
                    maximum_command_sockets: $maximum_command_sockets,
                    supports_operation_complete: $supports_operation_complete,
                    supports_command_cancel: $supports_command_cancel,
                    position_inquiries: $position_inquiries,
                },)*}
            }

            #[cfg(all(test, feature = "serde"))]
            #[allow(clippy::expect_used)]
            fn row_encoding_keys(self) -> serde_json::Value {
                match self {$(Self::$profile => {
                    #[allow(unused_mut)]
                    let mut keys = serde_json::Map::new();
                    $($( {
                        let selection = $crate::capabilities::encoding_selection!($support($support_encoding));
                        let value = serde_json::to_value(selection).expect("grant encoding serializes");
                        let object = value.as_object().expect("encoding is surface-keyed");
                        for key in object.keys() { keys.insert(key.clone(), serde_json::Value::Null); }
                    } )?)*
                    serde_json::Value::Object(keys)
                },)*}
            }

            /// Returns the public compile-time type name for this profile.
            pub(crate) const fn compile_time_type_name(&self) -> &'static str {
                match self {
                    $(ProfileId::$profile => stringify!($profile),)*
                }
            }

            /// Returns the human-readable display name for this profile.
            pub const fn display_name(&self) -> &'static str {
                match self {
                    $(ProfileId::$profile => $model_name,)*
                }
            }

            /// Projects transport facts without constructing or validating a full profile.
            pub(crate) const fn transport_compatibility(self) -> $crate::profiles::TransportCompatibility {
                match self {$(Self::$profile => profile_registry::transport_compatibility(
                    $tcp $(($tcp_port))?, $udp $(($udp_port))?, $serial,
                ),)*}
            }

            /// Returns the default TCP port for this profile when TCP is supported.
            pub const fn default_tcp_port(&self) -> Option<u16> {
                match self {
                    $(ProfileId::$profile => $tcp $(($tcp_port))?,)*
                }
            }

            /// Returns the default UDP port for this profile when UDP is supported.
            pub const fn default_udp_port(&self) -> Option<u16> {
                match self {
                    $(ProfileId::$profile => $udp $(($udp_port))?,)*
                }
            }

            /// Returns the default camera ID for this profile.
            pub const fn default_camera_id(&self) -> $crate::IndividualCameraId {
                match self {
                    $(ProfileId::$profile => __individual_camera_id!($default_camera_id),)*
                }
            }

            /// Returns whether this profile uses Sony encapsulation.
            ///
            /// This is the profile's envelope type: only the Sony envelope
            /// carries sequence correlation.
            pub const fn uses_sony_encapsulation(&self) -> bool {
                match self {
                    $(ProfileId::$profile => {
                        matches!($envelope, $crate::profiles::ProfileEnvelope::SonyEncapsulated)
                    })*
                }
            }

            /// Returns the profile group for this camera profile.
            pub const fn profile_group(&self) -> ProfileGroup {
                match self {
                    $($(ProfileId::$group_profile => ProfileGroup::$group,)*)*
                }
            }

            /// Returns all available profile IDs.
            pub const fn all() -> &'static [ProfileId] {
                &[$(ProfileId::$profile,)*]
            }

            /// Returns a vendor identifier for this profile.
            pub const fn vendor(&self) -> &'static str {
                match self {
                    $(ProfileId::$profile => $vendor,)*
                }
            }

            /// Returns the level of VISCA inquiry command support for this profile.
            pub const fn inquiry_support(&self) -> $crate::capabilities::InquirySupport {
                match self {
                    $(ProfileId::$profile => $inquiry_support,)*
                }
            }

            /// Returns a brief description of this profile's capabilities.
            pub const fn description(&self) -> &'static str {
                match self {
                    $(ProfileId::$profile => $description,)*
                }
            }

            #[cfg(test)]
            pub(crate) fn registry_facts(&self) -> &'static profile_registry::BuiltinProfileFacts {
                BUILTIN_PROFILE_FACTS
                    .iter()
                    .find(|facts| facts.id == *self)
                    .expect("all ProfileId variants are generated from BUILTIN_PROFILE_FACTS")
            }
        }

        impl ProfileGroup {
            /// Returns all profile IDs that belong to this group.
            pub const fn profiles(&self) -> &'static [ProfileId] {
                match self {
                    $(ProfileGroup::$group => &[$(ProfileId::$group_profile,)*],)*
                }
            }

            /// Returns a human-readable display name for this profile group.
            pub const fn display_name(&self) -> &'static str {
                match self {
                    $(ProfileGroup::$group => $group_display,)*
                }
            }

            /// Returns whether every profile in this group uses Sony
            /// encapsulation.
            pub fn uses_sony_encapsulation(&self) -> bool {
                self.every_member(profile_registry::MemberFact::SonyEncapsulation)
            }

            /// Returns whether every profile in this group supports TCP
            /// transport.
            pub fn supports_tcp(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Tcp)
            }

            /// Returns whether every profile in this group supports UDP
            /// transport.
            pub fn supports_udp(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Udp)
            }

            /// Returns whether every profile in this group supports serial
            /// transport.
            pub fn supports_serial(&self) -> bool {
                self.every_member(profile_registry::MemberFact::Serial)
            }

            /// Returns the VISCA inquiry support every profile in this group
            /// has: the weakest level among its members.
            pub const fn inquiry_support(&self) -> $crate::capabilities::InquirySupport {
                let members = self.profiles();
                let mut weakest = $crate::capabilities::InquirySupport::Full;
                let mut index = 0;
                while index < members.len() {
                    weakest = profile_registry::weaker_inquiry_support(
                        weakest,
                        members[index].inquiry_support(),
                    );
                    index += 1;
                }
                weakest
            }

            /// Group facts are derived from the member rows, so they cannot
            /// disagree with them.
            fn every_member(&self, fact: profile_registry::MemberFact) -> bool {
                let members = self.profiles();
                let mut index = 0;
                while index < members.len() {
                    let member = members[index];
                    let holds = match fact {
                        profile_registry::MemberFact::SonyEncapsulation => {
                            member.uses_sony_encapsulation()
                        }
                        profile_registry::MemberFact::Tcp => member.transport_compatibility().supports($crate::transport::StandardTransport::Tcp),
                        profile_registry::MemberFact::Udp => member.transport_compatibility().supports($crate::transport::StandardTransport::Udp),
                        profile_registry::MemberFact::Serial => member.transport_compatibility().supports($crate::transport::StandardTransport::Serial),
                    };
                    if !holds {
                        return false;
                    }
                    index += 1;
                }
                true
            }
        }

        impl std::str::FromStr for ProfileId {
            type Err = $crate::Error;

            fn from_str(value: &str) -> $crate::Result<Self> {
                match value {
                    $($profile_id => Ok(Self::$profile),)*
                    _ => Err($crate::Error::invalid_parameter(
                        "profile_id", value.to_owned(),
                        format!("expected one of: {}", Self::all().iter().map(Self::as_str).collect::<Vec<_>>().join(", ")),
                    )),
                }
            }
        }

        impl Default for ProfileId {
            fn default() -> Self { Self::GenericVisca }
        }

        impl std::fmt::Display for ProfileGroup {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }

        impl std::fmt::Display for ProfileId {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.display_name())
            }
        }

        #[cfg(test)]
        fn registry_profile_type_names_for(
            surface: $crate::capabilities::TypedSupportSurface,
        ) -> String {
            BUILTIN_PROFILE_FACTS
                .iter()
                .filter(|facts| facts.has_typed_support(surface))
                .map(|facts| format!("`{}`", facts.type_name))
                .collect::<Vec<_>>()
                .join(", ")
        }

        #[cfg(test)]
        #[allow(clippy::expect_used)]
        mod registry_tests {
            #[test]
            fn transport_predicate_and_ports_match_the_documented_matrix() {
                use $crate::transport::{NetworkTransport, StandardTransport};
                let rows = [
                    (ProfileId::PtzOpticsG2, Some(5678), Some(1259), true),
                    (ProfileId::PtzOpticsG3, Some(5678), Some(1259), true),
                    (ProfileId::PtzOptics30x, Some(5678), Some(1259), true),
                    (ProfileId::SonyFr7, None, Some(52381), false),
                    (ProfileId::SonyBrcH900, None, Some(52381), false),
                    (ProfileId::SonyEviH100, None, None, true),
                    (ProfileId::SonyBrc300, None, None, true),
                    (ProfileId::NearusBrc300, None, None, true),
                    (ProfileId::GenericVisca, Some(5678), Some(1259), true),
                ];
                assert_eq!(rows.len(), ProfileId::all().len());
                let docs = include_str!("../../docs/architecture_inventory.md");
                for (profile, tcp, udp, serial) in rows {
                    let facts = profile.transport_compatibility();
                    assert_eq!(facts, profile.spec().expect("built-in profile").transports());
                    for (kind, expected) in [
                        (StandardTransport::Tcp, tcp.is_some()),
                        (StandardTransport::Udp, udp.is_some()),
                        (StandardTransport::Serial, serial),
                    ] {
                        assert_eq!(facts.supports(kind), expected, "{profile:?} {kind:?}");
                    }
                    assert_eq!(facts.default_port(NetworkTransport::Tcp), tcp);
                    assert_eq!(facts.default_port(NetworkTransport::Udp), udp);
                    let port = |port: Option<u16>| port.map_or_else(|| "n/a".to_owned(), |value| value.to_string());
                    let row = format!("| `{profile:?}` | {} | {} | {} |", port(tcp), port(udp), if serial { "yes" } else { "no" });
                    assert!(docs.contains(&row), "missing documented transport row: {row}");
                }
            }

            use super::*;

            fn assert_profile_registry<P>(
                facts: &profile_registry::BuiltinProfileFacts,
                id: ProfileId,
            )
            where
                P: $crate::profiles::CompileTimeProfile,
            {
                let spec = P::spec().unwrap_or_else(|error| panic!("{id:?}: {error}"));
                let caps = spec.capabilities();
                assert_eq!(P::ID, Some(id));
                assert_eq!(spec, id.spec().expect("runtime built-in resolution"));
                assert_eq!(caps.profile_id, Some(id));
                assert_eq!(caps.typed_support, P::TYPED_SUPPORT);
                assert_eq!(facts.typed_support, P::TYPED_SUPPORT);
                assert_eq!(P::DOMAINS, $crate::capabilities::DomainSet::from_capabilities(caps));
                assert_eq!(P::TRANSPORTS, spec.transports().transport_set());
                assert_eq!(facts.id, id);
                assert_eq!(facts.group, id.profile_group());
                assert_eq!(facts.display_name, caps.model_name);
                assert_eq!(facts.display_name, id.display_name());
                assert_eq!(facts.vendor, id.vendor());
                assert_eq!(facts.description, id.description());
                assert_eq!(facts.tcp, spec.transports().tcp_port());
                assert_eq!(facts.udp, spec.transports().udp_port());
                assert_eq!(facts.serial, spec.transports().supports_serial());
                assert_eq!(facts.default_camera_id, caps.default_camera_id.value());
                assert_eq!(facts.default_camera_id, id.default_camera_id().value());
                assert_eq!(facts.inquiry_support, caps.inquiry_support);
                assert_eq!(facts.position_inquiries, spec.position_inquiries());
                assert_eq!(facts.envelope, spec.envelope());
                assert_eq!(caps.image.is_some(), facts.image_base_support);
                assert_eq!(caps.focus.as_ref().is_some_and(|focus| focus.zone_inquiry), facts.focus_zone_inquiry);
                assert_eq!(caps.supports($crate::capabilities::TypedSupportSurface::UsbAudio), facts.usb_audio);
                assert_eq!(
                    facts.has_typed_support($crate::capabilities::TypedSupportSurface::ExposureMode),
                    caps.exposure.as_ref().is_some_and(|exposure| !exposure.modes.is_empty()),
                );
                assert_eq!(
                    caps.pan_tilt.is_some_and(|domain| domain.geometry.is_some()),
                    caps.supports($crate::capabilities::TypedSupportSurface::PanTiltPositioning),
                );
                // A row may not advertise a typed image method without an image record.
                for surface in [
                    $crate::capabilities::TypedSupportSurface::ImageFlip,
                    $crate::capabilities::TypedSupportSurface::ImageMirror,
                    $crate::capabilities::TypedSupportSurface::CombinedImageFlip,
                    $crate::capabilities::TypedSupportSurface::Contrast,
                    $crate::capabilities::TypedSupportSurface::Sharpness,
                    $crate::capabilities::TypedSupportSurface::Saturation,
                    $crate::capabilities::TypedSupportSurface::Hue,
                    $crate::capabilities::TypedSupportSurface::Luminance,
                    $crate::capabilities::TypedSupportSurface::Gamma,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                    $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                    $crate::capabilities::TypedSupportSurface::PictureEffect,
                    $crate::capabilities::TypedSupportSurface::ImageFreeze,
                ] {
                    assert!(facts.image_base_support || !caps.supports(surface), "{id:?} {surface:?}");
                }
            }

            #[test]
            fn profile_ids_and_groups_match_registry() {
                assert_eq!(ProfileId::all().len(), BUILTIN_PROFILE_FACTS.len());
                for facts in BUILTIN_PROFILE_FACTS {
                    let id = facts.id;
                    assert_eq!(id.registry_facts().type_name, facts.type_name);
                    assert!(facts.group.profiles().contains(&id));
                    assert_eq!(id.profile_group(), facts.group);
                    assert_eq!(
                        id.uses_sony_encapsulation(),
                        facts.group.uses_sony_encapsulation()
                    );
                }
            }

            #[test]
            fn builtin_profile_capabilities_match_registry() {
                $(
                    assert_profile_registry::<$profile>(
                        ProfileId::$profile.registry_facts(),
                        ProfileId::$profile,
                    );
                )*
            }

            /// #818: the surface-to-metadata rule
            /// (`Capabilities::surface_metadata`) holds in both directions for
            /// every built-in row. A marker always needs its metadata (profile
            /// validation enforces that); a surface with its own metadata fact
            /// must also carry the marker whenever the metadata advertises it,
            /// unless the row lists the surface in `withheld` and its evidence
            /// says why.
            #[test]
            fn typed_support_markers_match_dedicated_metadata() {
                fn assert_row(id: ProfileId, caps: &$crate::capabilities::Capabilities) {
                    use $crate::capabilities::SurfaceMetadata::Dedicated;

                    let facts = id.registry_facts();
                    for surface in $crate::capabilities::TypedSupportSurface::ALL {
                        let marker = caps.supports(surface);
                        let metadata = caps.surface_metadata(surface);
                        assert!(
                            !marker || metadata.permits(),
                            "{id:?} grants {surface:?} without its metadata"
                        );
                        let Dedicated(advertised) = metadata else {
                            assert!(
                                !facts.withheld.contains(surface),
                                "{id:?} withholds {surface:?}, which has no metadata of its own"
                            );
                            continue;
                        };
                        if facts.withheld.contains(surface) {
                            // An exemption is exact: the metadata advertises
                            // the surface, the marker is absent, and the
                            // evidence says why. A stale entry fails here.
                            assert!(
                                advertised && !marker,
                                "{id:?}: stale withheld {surface:?} (metadata {advertised}, marker {marker})"
                            );
                            assert!(
                                facts.evidence.iter().any(|item| item.key == surface.wire_name().replace('-', "_")),
                                "{id:?} withholds {surface:?} without a `{}` evidence entry",
                                surface.wire_name().replace('-', "_")
                            );
                        } else {
                            assert_eq!(
                                marker, advertised,
                                "{id:?}: {surface:?} marker disagrees with its metadata; fix the row or list the surface in `withheld`"
                            );
                        }
                    }
                }

                $(
                    assert_row(
                        ProfileId::$profile,
                        $crate::ProfileSpec::from_compile_time::<$profile>()
                            .unwrap_or_else(|error| panic!("{} ProfileSpec failed: {error}", stringify!($profile)))
                            .capabilities(),
                    );
                )*
            }

            /// A STOP queued on a raw byte stream waits at most one ACK plus one
            /// ambiguity interval for the rules that keep its answers decisive
            /// (#795). For every built-in profile that bound is below half a
            /// halt's budget, which is at least the movement completion
            /// deadline, so a halt's STOPs are always written within it.
            #[test]
            fn stop_wait_bound_is_below_every_halt_budget() {
                $(
                    let timing = ProfileId::$profile.spec().expect("profile").timing();
                    let ack = timing.ack_timeout();
                    let ambiguity = timing.ambiguity_timeout();
                    let movement = timing.command_timeouts().movement_timeout();
                    assert!(
                        (ack + ambiguity) * 2 <= movement,
                        "{}: STOP wait bound {:?} against halt budget {movement:?}",
                        stringify!($profile),
                        ack + ambiguity,
                    );
                )*
            }

            /// Every built-in unit conversion and shutter fact agrees with the
            /// profile tables (#807, #808, #819, #828 M2): the public pan/tilt
            /// helper equals the request path, every raw unit round-trips
            /// through degrees, each advertised shutter code is constructible
            /// and reachable by its exposure time, a profile's nominal optical
            /// ratio reaches the end of its optical range, and each Kelvin step
            /// round-trips.
            #[test]
            fn builtin_unit_conversions_agree_with_profile_tables() {


                fn sweep(range: &std::ops::RangeInclusive<f32>) -> impl Iterator<Item = f32> + '_ {
                    let steps = ((range.end() - range.start()) / 0.05).floor() as i32;
                    (0..=steps).map(move |step| range.start() + step as f32 * 0.05)
                }

                fn assert_reachable(id: ProfileId, scale: $crate::capabilities::ZoomScale) {
                    let max = scale.optical_max();
                    let units = scale
                        .units(scale.optical_ratio())
                        .unwrap_or_else(|error| panic!("{id:?} nominal ratio: {error}"));
                    assert!(
                        units <= max && units >= max - 1,
                        "{id:?} {}x maps to {units:#06X}, not the optical end {max:#06X}",
                        scale.optical_ratio()
                    );
                }

                fn assert_profile<P>(id: ProfileId)
                where
                    P: $crate::profiles::CompileTimeProfile + Default,
                {
                    let spec = $crate::ProfileSpec::from_compile_time::<P>()
                        .unwrap_or_else(|error| panic!("{id:?} ProfileSpec failed: {error}"));
                    let caps = spec.capabilities();
                    if let Some(geometry) = caps.pan_tilt.and_then(|domain| domain.geometry) {
                        let conversion = geometry.coordinates;
                        let pan_range = geometry.pan_range.as_inclusive();
                        let tilt_range = geometry.tilt_ranges.range(None).as_inclusive();
                        let pan_degrees = geometry.pan_range_degrees();
                        let tilt_degrees = geometry.tilt_range_degrees(None);
                        for pan in sweep(&pan_degrees).filter(|value| value > pan_degrees.start() && value < pan_degrees.end()) {
                            let expected = conversion.pan_units(pan)
                                .zip(conversion.tilt_units(0.0))
                                .filter(|(pan, tilt)| {
                                    pan_range.contains(pan) && tilt_range.contains(tilt)
                                });
                            assert_eq!(
                                $crate::TargetRegistration::from_profile($crate::target::TargetSlot::CAMERA_1, spec.clone()).convert_pan_tilt_degrees(pan, 0.0).ok(),
                                expected,
                                "{id:?} pan {pan}°"
                            );
                        }
                        for tilt in sweep(&tilt_degrees).filter(|value| value > tilt_degrees.start() && value < tilt_degrees.end()) {
                            let expected = conversion.pan_units(0.0)
                                .zip(conversion.tilt_units(tilt))
                                .filter(|(pan, tilt)| {
                                    pan_range.contains(pan) && tilt_range.contains(tilt)
                                });
                            assert_eq!(
                                $crate::TargetRegistration::from_profile($crate::target::TargetSlot::CAMERA_1, spec.clone()).convert_pan_tilt_degrees(0.0, tilt).ok(),
                                expected,
                                "{id:?} tilt {tilt}°"
                            );
                        }
                        for units in pan_range.clone() {
                            assert_eq!(
                                conversion.pan_units(conversion.pan_degrees(units)),
                                Some(units),
                                "{id:?} pan unit {units}"
                            );
                        }
                        for units in tilt_range.clone() {
                            assert_eq!(
                                conversion.tilt_units(conversion.tilt_degrees(units)),
                                Some(units),
                                "{id:?} tilt unit {units}"
                            );
                        }
                    }

                    match (caps.zoom.as_ref().expect("zoom").optical_ratio, caps.zoom_scale()) {
                        (Some(ratio), Ok(scale)) => {
                            assert_eq!(scale.optical_ratio(), ratio, "{id:?}");
                            assert_reachable(id, scale);
                        }
                        (None, Err($crate::Error::InvalidRequest { reason: message })) => assert!(
                            message.contains("zoom_scale_for_lens"),
                            "{id:?}: {message}"
                        ),
                        (ratio, scale) => panic!("{id:?}: ratio {ratio:?} gave {scale:?}"),
                    }

                    for entry in &caps.exposure.as_ref().expect("exposure").shutter_speeds {
                        let code = $crate::types::ShutterSpeed::new(entry.value).expect("profile shutter code");
                        assert_eq!(
                            caps.shutter_speed_for(entry.exposure).ok(),
                            Some(code),
                            "{id:?} {}",
                            entry.exposure
                        );
                    }

                    if let Some(range) = &caps.white_balance.as_ref().expect("white balance").color_temp_range {
                        for kelvin in (range.min()..=range.max()).step_by(100) {
                            let color_temp = $crate::types::Kelvin::new(kelvin)
                                .unwrap_or_else(|error| panic!("{id:?} {kelvin} K: {error}"));
                            let white = caps.white_balance.as_ref().expect("white balance");
                            let code = white.color_temperature_wire(color_temp).expect("profile step");
                            assert_eq!(white.color_temperature_from_wire(code).expect("profile step").value(), kelvin, "{id:?}");
                        }
                    }
                }

                $(
                    assert_profile::<$profile>(ProfileId::$profile);
                )*

                // The PTZOptics G2 family's lens is a camera fact: each bench
                // lens (PT12X/PT20X/PT30X-NDI G2, §4.1) reaches its optical end.
                let g2 = $crate::ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile").capabilities().clone();
                assert_eq!(g2.zoom.as_ref().expect("zoom").optical_ratio, None);
                for ratio in [12.0, 20.0, 30.0] {
                    let scale = g2.zoom_scale_for_lens(ratio).expect("G2 lens");
                    assert_reachable(ProfileId::PtzOpticsG2, scale);
                }
            }

            /// Optional image-record presence emits both the runtime domain
            /// and the static `HasImage` marker.
            /// Negative marker assertions live in public compile-contract
            /// fixtures, because Rust has no stable negative-trait assertion.
            #[test]
            fn image_processing_markers_follow_base_support() {
                fn assert_image_processing_marker<P: $crate::capabilities::HasImage>() {}

                $(
                    __assert_domain_marker!(
                        $image_facts for $profile,
                        assert_image_processing_marker
                    );
                )*
            }

            /// No built-in profile has a hardware-measured acknowledgement or
            /// inquiry-response deadline yet (#689), so every row must name
            /// the shared interim values rather than a literal of its own. A
            /// row that needs another value must cite a measurement.
            #[test]
            fn builtin_deadlines_are_the_interim_values_until_measured() {
                let ack = std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS);
                let inquiry =
                    std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS);
                $(
                    let timing = $crate::ProfileSpec::from_compile_time::<$profile>()
                        .unwrap_or_else(|error| {
                            panic!("{} ProfileSpec failed: {error}", stringify!($profile))
                        })
                        .timing();
                    assert_eq!(timing.ack_timeout(), ack, "{}", stringify!($profile));
                    assert_eq!(timing.inquiry_timeout(), inquiry, "{}", stringify!($profile));
                )*
            }

            #[test]
            fn sony_fr7_has_no_shared_ae_mode_inventory() {
                let profile = $crate::ProfileSpec::from_compile_time::<$crate::profiles::SonyFr7>()
                    .expect("Sony FR7 profile");
                let capabilities = profile.capabilities();
                assert!(capabilities.exposure.is_some());
                assert!(capabilities.exposure.as_ref().expect("exposure").modes.is_empty());
                assert!(!capabilities.supports(
                    $crate::capabilities::TypedSupportSurface::ExposureMode
                ));
                for mode in [
                    $crate::command::exposure_wire::ExposureMode::Auto,
                    $crate::command::exposure_wire::ExposureMode::Manual,
                    $crate::command::exposure_wire::ExposureMode::Shutter,
                    $crate::command::exposure_wire::ExposureMode::Iris,
                    $crate::command::exposure_wire::ExposureMode::Bright,
                ] {
                    assert!(!capabilities.exposure.as_ref().expect("exposure").modes.contains(&mode));
                }
            }

            #[test]
            fn shared_exposure_mode_support_is_exact_and_inventory_backed() {
                // `assert_profile_registry` ties each row's marker to its
                // mode inventory; this pins which rows have one.
                for facts in BUILTIN_PROFILE_FACTS {
                    let expected = facts.id != ProfileId::SonyFr7;
                    assert_eq!(
                        facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::ExposureMode
                        ),
                        expected,
                        "{:?} shared exposure-mode support must exactly follow the source-backed mode inventory",
                        facts.id
                    );
                }
            }

            #[test]
            fn every_typed_grant_resolves_a_positive_model_source() {
                let index = $crate::source_evidence::checked::Index::parse(include_str!("../../docs/visca_reference.md")).expect("checked source index");
                let mut failures = Vec::new();
                for facts in BUILTIN_PROFILE_FACTS {
                    assert_eq!(facts.typed_sources.len(), facts.typed_support.iter().count());
                    for (surface, source) in facts.typed_sources {
                        let model = format!("{:?}", facts.id);
                        if let Err(error) = index.validate_grant(*source, *surface, facts.id) { failures.push(format!("{model}: {error}")); }
                    }
                }
                assert!(failures.is_empty(), "{}", failures.join("\n"));
            }

            #[test]
            fn typed_support_registry_has_evidence_for_surprising_gaps() {
                for facts in BUILTIN_PROFILE_FACTS {
                    if !facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::Iris,
                    ) {
                        assert!(
                            facts.evidence.iter().any(|item| item.key == "iris"),
                            "{} must document why iris typed support is absent",
                            facts.type_name
                        );
                    }
                }

                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30x,
                ] {
                    let facts = id.registry_facts();
                    assert!(
                        facts.evidence.iter().any(|item| item.key == "digital_zoom"),
                        "{id:?} must document why digital zoom typed support is absent"
                    );
                }
            }

            #[test]
            fn ptzoptics_profiles_follow_consolidated_reference_boundaries() {
                fn assert_ptzoptics_raw_profile<P>()
                where
                    P: $crate::profiles::CompileTimeProfile
                        + $crate::capabilities::SupportsTcp
                        + $crate::capabilities::SupportsUdp
                        + $crate::capabilities::SupportsSerial
                        + Default,
                {
                    let spec = P::spec().expect("PTZOptics profile");
                    let caps = spec.capabilities();
                    let pan = caps.pan_tilt.as_ref().expect("pan/tilt");
                    let zoom = caps.zoom.as_ref().expect("zoom");
                    let exposure = caps.exposure.as_ref().expect("exposure");
                    let focus = caps.focus.as_ref().expect("focus");
                    let presets = caps.presets.as_ref().expect("presets");
                    assert_eq!(spec.transports(), $crate::profiles::TransportCompatibility::new().with_tcp(5678).with_udp(1259).with_serial());
                    assert_eq!(pan.geometry.expect("geometry").pan_range, range!(i32, -2448, 2448));
                    assert_eq!(pan.geometry.expect("geometry").tilt_ranges.range(None), range!(i32, -432, 1296));
                    assert_eq!(pan.pan_speed.max(), 24);
                    assert_eq!(pan.tilt_speed.max(), 20);
                    assert_eq!(zoom.optical_max, 0x4000);
                    assert_eq!(zoom.digital_max, None);
                    assert!(zoom.direct);
                    assert!(exposure.iris_range.is_some());
                    assert!(exposure.modes.contains(&$crate::command::exposure_wire::ExposureMode::Iris));
                    assert!(!focus.zones.is_empty());
                    assert!(!focus.near_limit_inquiry);
                    assert_eq!(presets.highest, 127);
                    assert_eq!(caps.image.as_ref().expect("image").picture_effects, vec![$crate::types::PictureEffectMode::Off, $crate::types::PictureEffectMode::BlackAndWhite]);
                    assert!(!presets.tour);
                }

                assert_ptzoptics_raw_profile::<PtzOpticsG2>();
                assert_ptzoptics_raw_profile::<PtzOpticsG3>();
                assert_ptzoptics_raw_profile::<PtzOptics30x>();

                // #828 L7: no PTZOptics source documents socket cancel for any
                // of the three profiles, so each records that and keeps it
                // unavailable.
                assert!(
                    !ProfileId::PtzOpticsG2.spec().expect("profile").supports_command_cancel()
                );
                assert!(
                    !ProfileId::PtzOpticsG3.spec().expect("profile").supports_command_cancel()
                );
                assert!(
                    !ProfileId::PtzOptics30x.spec().expect("profile").supports_command_cancel()
                );
                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30x,
                ] {
                    assert!(
                        id.registry_facts()
                            .evidence
                            .iter()
                            .any(|item| item.key == "command_cancel"),
                        "{id:?} must record why socket cancel is unavailable"
                    );
                }

                for id in [
                    ProfileId::PtzOpticsG2,
                    ProfileId::PtzOpticsG3,
                    ProfileId::PtzOptics30x,
                ] {
                    let facts = id.registry_facts();
                    assert!(facts.has_typed_support($crate::capabilities::TypedSupportSurface::FocusZone));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::Iris));
                    assert!(facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::ExposureMode
                    ));
                    assert!(facts
                        .has_typed_support($crate::capabilities::TypedSupportSurface::PictureEffect));
                    assert!(!facts.has_typed_support(
                        $crate::capabilities::TypedSupportSurface::FocusNearLimit
                    ));
                    for surface in [
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction2D,
                        $crate::capabilities::TypedSupportSurface::NoiseReduction3D,
                    ] {
                        assert!(facts.has_typed_support(surface), "{id:?} {surface:?}");
                    }
                }

                const R14_NR_EVIDENCE: &str = "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain.";
                for id in [ProfileId::PtzOpticsG2, ProfileId::PtzOpticsG3] {
                    let evidence = id
                        .registry_facts()
                        .evidence
                        .iter()
                        .find(|item| item.key == "noise_reduction")
                        .expect("G2 and G3 must record the R14 NR evidence");
                    assert_eq!(evidence.rationale, R14_NR_EVIDENCE, "{id:?}");
                }
                let legacy_30x_evidence = ProfileId::PtzOptics30x
                    .registry_facts()
                    .evidence
                    .iter()
                    .find(|item| item.key == "noise_reduction")
                    .expect("legacy 30X must record separate inquiry and control sources");
                assert_eq!(
                    legacy_30x_evidence.rationale,
                    "R1 supplies legacy PT30X SDI/NDI G2 09 04 50/53/54 inquiries, including 3D 0..8, but no setters. R14 supplies 04 50/53/54 controls only because R15 explicitly narrows this profile to that legacy PT30X SDI/NDI G2 raw-VISCA family. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by query output domains; do not generalize this evidence to newer 30X products."
                );
                // R8 documents only the EVI-H100 `04 53` 2D level control and
                // inquiry; the `04 50` mode and the 3D level stay PTZOptics.
                let ptzoptics = "`PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`";
                let with_evi = "`PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30x`, `SonyEviH100`";
                for (surface, profiles) in [
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2D, with_evi),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2D, with_evi),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction2DMode, ptzoptics),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction3D, ptzoptics),
                    ($crate::capabilities::TypedSupportSurface::NoiseReduction3D, ptzoptics),
                ] {
                    assert_eq!(registry_profile_type_names_for(surface), profiles, "{surface:?}");
                }

                for (id, focus_zone_inquiry, usb_audio) in [
                    (ProfileId::PtzOpticsG2, true, true),
                    (ProfileId::PtzOpticsG3, false, false),
                    (ProfileId::PtzOptics30x, true, true),
                ] {
                    let facts = id.registry_facts();
                    assert_eq!(facts.focus_zone_inquiry, focus_zone_inquiry, "{id:?}");
                    assert_eq!(
                        facts.has_typed_support(
                            $crate::capabilities::TypedSupportSurface::FocusZoneInquiry
                        ),
                        focus_zone_inquiry,
                        "{id:?} focus-zone inquiry marker"
                    );
                    assert_eq!(facts.usb_audio, usb_audio, "{id:?}");
                    assert_eq!(
                        facts.has_typed_support($crate::capabilities::TypedSupportSurface::UsbAudio),
                        usb_audio,
                        "{id:?} USB-audio marker"
                    );
                }
            }

            #[test]
            fn typed_support_registry_covers_declared_surface_vocabulary() {
                for facts in BUILTIN_PROFILE_FACTS {
                    for surface in facts.typed_support.iter() {
                        assert!(
                            $crate::capabilities::TypedSupportSurface::ALL.contains(&surface),
                            "{surface:?} is not in ALL_TYPED_SUPPORT_SURFACES"
                        );
                    }
                }

                for surface in $crate::capabilities::TypedSupportSurface::ALL {
                    let profiles = registry_profile_type_names_for(surface);
                    if matches!(
                        surface,
                        $crate::capabilities::TypedSupportSurface::SnapFocus
                            | $crate::capabilities::TypedSupportSurface::MotionSync
                            | $crate::capabilities::TypedSupportSurface::AutoFocusSensitivity
                            | $crate::capabilities::TypedSupportSurface::ImageFreeze
                            | $crate::capabilities::TypedSupportSurface::TallyBrightness
                            | $crate::capabilities::TypedSupportSurface::TallyMode
                    ) {
                        assert!(
                            profiles.is_empty(),
                            "{surface:?} should remain absent for built-ins until evidenced"
                        );
                    } else {
                        assert!(
                            !profiles.is_empty(),
                            "{surface:?} has no built-in typed support and should be removed or classified"
                        );
                    }
                }
            }

            #[cfg(feature = "serde")]
            #[test]
            fn authored_row_fact_keys_match_every_exported_profile() {
                for id in ProfileId::all() {
                    let exported = serde_json::to_value(id.spec().expect("profile")).expect("serialize profile");
                    profile_registry::check_fact_keys(&id.row_fact_keys(), &exported, "")
                        .unwrap_or_else(|error| panic!("{id:?}: {error}"));
                }
            }

            #[cfg(feature = "serde")]
            #[test]
            fn row_key_alignment_detects_new_missing_and_renamed_record_facts() {
                let id = ProfileId::PtzOpticsG2;
                let exported = serde_json::to_value(id.spec().expect("profile")).expect("serialize profile");
                let keys = id.row_fact_keys();
                for path in ["", "/capabilities", "/timing", "/transports", "/capabilities/pan_tilt/geometry/coordinates", "/capabilities/white_balance/rgb_gain"] {
                    let mut changed = exported.clone();
                    changed.pointer_mut(path).expect("fact node").as_object_mut().expect("fact record")
                        .insert("future_fact".into(), serde_json::Value::Bool(false));
                    let error = profile_registry::check_fact_keys(&keys, &changed, "").expect_err("new fact must have a row counterpart");
                    assert!(error.contains("future_fact"), "{error}");
                }
                let mut missing = exported.clone();
                missing["timing"].as_object_mut().expect("timing record").remove("inquiry_spacing");
                assert!(profile_registry::check_fact_keys(&keys, &missing, "").expect_err("missing fact").contains("inquiry_spacing"));
                let mut renamed = keys.clone();
                let value = renamed["capabilities"].as_object_mut().expect("capabilities record").remove("model_name").expect("model key");
                renamed["capabilities"].as_object_mut().expect("capabilities record").insert("model_label".into(), value);
                assert!(profile_registry::check_fact_keys(&renamed, &exported, "").expect_err("renamed fact").contains("model_name"));
            }


            #[test]
            fn profile_ids_parse_their_pinned_strings_without_serde() {
                for id in ProfileId::all() {
                    assert_eq!(id.as_str().parse::<ProfileId>().expect("canonical id"), *id);
                }
                for invalid in ["", "PtzOpticsG2", "unknown_camera", " ptzoptics_g2"] {
                    let error = invalid.parse::<ProfileId>().expect_err("only pinned ids are accepted").to_string();
                    for id in ProfileId::all() { assert!(error.contains(id.as_str()), "{error}"); }
                }
            }


        }
    };
}

/// The built-in profile registry: one row per profile, passed to `$callback`.
///
/// `facts` uses ProfileSpec's serialized field keys and nesting. Leaves are
/// typed Rust expressions. `profile_id: from_id` uses the one pinned row id;
/// `typed_support: grants(...)` adds evidence to each permission; and
/// `encodings: from_grants` derives each selected family from that same grant.
/// These sentinels avoid copying identity, grants or encodings into parallel data.
///
/// `camera::profiles` expands it with `__define_builtin_profiles` into the
/// profile types and everything derived from them, and the crate's public
/// re-export lists expand it with [`reexport_builtin_profiles`], so a new row
/// needs no hand edit elsewhere.
macro_rules! builtin_profile_registry {
    ($callback:path) => {
        $callback! {
            groups {
                group GenericVisca {
                    doc: "Generic VISCA-compatible cameras with basic features.",
                    display: "Generic VISCA",
                    id: "generic_visca",
                    profiles: [GenericVisca, SonyBrc300, SonyEviH100, NearusBrc300],
                }
                group PtzOptics {
                    doc: "PtzOptics G2/G3/30X cameras using raw VISCA with PTZOptics timing limits.",
                    display: "PtzOptics Series",
                    id: "ptzoptics",
                    profiles: [PtzOpticsG2, PtzOpticsG3, PtzOptics30x],
                }
                group SonyProfessional {
                    doc: "Sony professional cameras using Sony encapsulated VISCA.",
                    display: "Sony Professional",
                    id: "sony_professional",
                    profiles: [SonyFr7, SonyBrcH900],
                }
            }
            profiles {
                profile PtzOpticsG2 {
                    doc: "PtzOptics G2-series camera profile.",
                    id: "ptzoptics_g2",
                    id_doc: "PtzOptics G2 series cameras",
                    vendor: "PtzOptics",
                    description: "G2-series PTZ camera with 12x, 20x, and 30x optical variants and 128 presets",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "PtzOptics G2",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -2448, 2448),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -432, 1296)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                                        pan_degrees_to_units: 14.4,
                                        tilt_degrees_to_units: 14.4,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 24),
                                tilt_speed: range!(u8, 1, 20),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x0000, 0xFFFF),
                                near_end: $crate::capabilities::FocusNearEnd::Unknown,
                                auto_focus: true,
                                one_push: false,
                                zone_inquiry: true,
                                zones: (profile_constants::PTZ_OPTICS_G2_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x0C)),
                                shutter_speeds: (profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0, 7)),
                                brightness_range: Some(domain!(u8, 0, 17)),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: true,
                            },
                            white_balance: {
                                modes: (profile_constants::COLOR_TEMP_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: Some(range!(u16, 2500, 8000)),
                                color_temperature_steps: std::sync::Arc::from(profile_constants::PTZ_COLOR_TEMPERATURE_STEPS),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: {
                                    red: range!(i8, -10, 10),
                                    blue: range!(i8, -10, 10),
                                },
                            },
                            image: {
                                contrast_range: Some(range!(u8, 0, 14)),
                                sharpness_range: Some(range!(u8, 0, 15)),
                                saturation_range: Some(range!(u8, 0, 14)),
                                flip: true,
                                mirror: true,
                                hue_range: Some(range!(u8, 0, 14)),
                                picture_effects: vec![$crate::types::PictureEffectMode::Off, $crate::types::PictureEffectMode::BlackAndWhite],
                                luminance_range: Some(range!(u8, 0, 14)),
                                requires_settings_save_for_flip: true,
                                gamma_range: Some(range!(u8, 0, 4)),
                            },
                            presets: {
                                highest: 127,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 24)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(10),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Full,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R1, "r1-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-powerstandby-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-shutterdirect-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-gaincontrol-grant")])),
                                (GainLimit, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-gainlimit-grant")])),
                                (MenuEnterReturn, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-menuenterreturn-grant")])),
                                (MenuDisplayInquiry, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-menustatusinquiry-grant")])),
                                (AntiFlicker, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-ptzopticsantiflicker-grant")])),
                                (SettingsSave, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-ptzopticssettingssave-grant")])),
                                (PresetRecallSpeed(PtzOptics), $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-ptzopticspresetrecallspeed-grant")])),
                                (MulticastStreaming, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-ptzopticsmulticaststreaming-grant")])),
                                (NdiQuality, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-ptzopticsndiquality-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-exposuremode-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-exposurecompensation-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-brightnesscontrol-grant")])),
                                (FocusLock, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-focuslock-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-directzoom-grant")])),
                                (Iris, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-iriscontrol-grant")])),
                                (FocusZone, $crate::source_evidence::source!([(B1, "b1-ptzopticsg2-focuszone-grant")])),
                                (FocusZoneInquiry, $crate::source_evidence::source!([(B1, "b1-ptzopticsg2-focuszoneinquiry-grant")])),
                                (UsbAudio, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-usbaudio-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-backlightcompensation-grant")])),
                                (WideDynamicRange, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-widedynamicrange-grant")])),
                                (ColorTemperature, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-colortemperature-grant")])),
                                (ColorTemperatureInquiry, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-colortemperatureinquiry-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-rgbgain-grant")])),
                                (RgbTuning, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-rgbtuning-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-onepushwhitebalance-grant")])),
                                (AutoWhiteBalanceSensitivity, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-autowhitebalancesensitivity-grant")])),
                                (ImageFlip, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-imageflip-grant")])),
                                (ImageMirror, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-imagemirror-grant")])),
                                (CombinedImageFlip, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-combinedimageflip-grant")])),
                                (Contrast, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-contrastcontrol-grant")])),
                                (Sharpness, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-sharpnesscontrol-grant")])),
                                (SharpnessMode, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-sharpnessmode-grant")])),
                                (Saturation, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-saturationcontrol-grant")])),
                                (Hue, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-huecontrol-grant")])),
                                (Luminance, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-luminancecontrol-grant")])),
                                (Gamma, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-gammacontrol-grant")])),
                                (NoiseReduction2D, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-noisereduction2d-grant"), (R22, "r14-ptzopticsg2-noisereduction2dcontrol-grant")])),
                                (NoiseReduction2DMode, $crate::source_evidence::source!([(R22, "r14-ptzopticsg2-noisereduction2dmode-grant")])),
                                (NoiseReduction3D, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-noisereduction3d-grant"), (R22, "r14-ptzopticsg2-noisereduction3dcontrol-grant")])),
                                (PictureEffect, $crate::source_evidence::source!([(R1, "r1-ptzopticsg2-pictureeffect-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: Some(5678),
                            udp_port: Some(1259),
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(150),
                            inquiry_spacing: std::time::Duration::from_millis(150),
                            command_spacing: std::time::Duration::from_millis(100),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("command_cancel", "On the PTZOptics G2 bench (PT30X-NDI G2 firmware ARM 6.3.51THI and PT12X-NDI G2 firmware ARM 6.4.18SHI; 2026-10-05), a socket cancel `81 2y FF` sent while a slow absolute pan/tilt move ran on socket y (ACK `90 4y`) was answered `90 60 02 FF` (syntax error) and the move continued to its target and completed (`90 5y`): the G2 family does not implement socket cancel."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795), `81 09 00 02 FF` returns the 2-byte payload `00 52`, not the 7-byte Sony layout, and neither R1 nor R14 documents the reply. Keep the typed version inquiry absent; send the raw inquiry instead."),
                        ("digital_zoom", "No model- and firmware-identified source-backed evidence establishes VISCA digital-zoom control for G2; keep it unavailable pending documented validation."),
                        ("iris", "R1 and the current PTZOptics G2/G3 Developer Portal (R14 in docs/visca_reference.md) document G2 `04 39`, iris reset/up/down, direct `04 4B`, and `09 04 4B` position inquiry. They omit the distinct `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the G2 family."),
                        ("focus_zones", "R14/R20 document `CAM_AFZone` Top `00`, Center `01` and Bottom `02`. On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795) the cameras also report `03`, accept `81 01 04 AA 03 FF` with ACK and completion, and read `03` back, so `FocusZone::Zone03` is admitted for this family."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the 81 2A 02 A0 04 USB-audio command and matching inquiry for G2 models."),
                        ("noise_reduction", "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain."),
                        ],
                }

                profile PtzOpticsG3 {
                    doc: "PtzOptics G3 camera profile.",
                    id: "ptzoptics_g3",
                    id_doc: "PtzOptics G3 series cameras",
                    vendor: "PtzOptics",
                    description: "Latest generation PTZ camera using the conservative raw VISCA preset range",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "PtzOptics G3",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -2448, 2448),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -432, 1296)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                                        pan_degrees_to_units: 14.4,
                                        tilt_degrees_to_units: 14.4,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 24),
                                tilt_speed: range!(u8, 1, 20),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x0000, 0xFFFF),
                                near_end: $crate::capabilities::FocusNearEnd::Unknown,
                                auto_focus: true,
                                one_push: false,
                                zone_inquiry: false,
                                zones: (profile_constants::DOCUMENTED_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x0C)),
                                shutter_speeds: (profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0, 7)),
                                brightness_range: Some(domain!(u8, 0, 17)),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: true,
                            },
                            white_balance: {
                                modes: (profile_constants::COLOR_TEMP_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: Some(range!(u16, 2500, 8000)),
                                color_temperature_steps: std::sync::Arc::from(profile_constants::PTZ_COLOR_TEMPERATURE_STEPS),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: {
                                    red: range!(i8, -10, 10),
                                    blue: range!(i8, -10, 10),
                                },
                            },
                            image: {
                                contrast_range: Some(range!(u8, 0, 14)),
                                sharpness_range: Some(range!(u8, 0, 15)),
                                saturation_range: Some(range!(u8, 0, 14)),
                                flip: true,
                                mirror: true,
                                hue_range: Some(range!(u8, 0, 14)),
                                picture_effects: vec![$crate::types::PictureEffectMode::Off, $crate::types::PictureEffectMode::BlackAndWhite],
                                luminance_range: Some(range!(u8, 0, 14)),
                                requires_settings_save_for_flip: true,
                                gamma_range: Some(range!(u8, 0, 4)),
                            },
                            presets: {
                                highest: 127,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 24)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(10),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Full,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R14, "r14-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-powerstandby-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-shutterdirect-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-gaincontrol-grant")])),
                                (GainLimit, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-gainlimit-grant")])),
                                (MenuEnterReturn, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-menuenterreturn-grant")])),
                                (MenuDisplayInquiry, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-menustatusinquiry-grant")])),
                                (AntiFlicker, $crate::source_evidence::source!([(R10, "r10-ptzopticsg3-ptzopticsantiflicker-grant")])),
                                (SettingsSave, $crate::source_evidence::source!([(R10, "r10-ptzopticsg3-ptzopticssettingssave-grant")])),
                                (PresetRecallSpeed(PtzOptics), $crate::source_evidence::source!([(R10, "r10-ptzopticsg3-ptzopticspresetrecallspeed-grant")])),
                                (MulticastStreaming, $crate::source_evidence::source!([(R10, "r10-ptzopticsg3-ptzopticsmulticaststreaming-grant")])),
                                (NdiQuality, $crate::source_evidence::source!([(R10, "r10-ptzopticsg3-ptzopticsndiquality-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-exposuremode-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-exposurecompensation-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-brightnesscontrol-grant")])),
                                (FocusLock, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-focuslock-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-directzoom-grant")])),
                                (Iris, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-iriscontrol-grant")])),
                                (FocusZone, $crate::source_evidence::source!([(R20, "r20-ptzopticsg3-focuszone-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-backlightcompensation-grant")])),
                                (WideDynamicRange, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-widedynamicrange-grant")])),
                                (ColorTemperature, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-colortemperature-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-rgbgain-grant")])),
                                (RgbTuning, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-rgbtuning-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-onepushwhitebalance-grant")])),
                                (AutoWhiteBalanceSensitivity, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-autowhitebalancesensitivity-grant")])),
                                (ImageFlip, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-imageflip-grant")])),
                                (ImageMirror, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-imagemirror-grant")])),
                                (CombinedImageFlip, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-combinedimageflip-grant")])),
                                (Contrast, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-contrastcontrol-grant")])),
                                (Sharpness, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-sharpnesscontrol-grant")])),
                                (SharpnessMode, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-sharpnessmode-grant")])),
                                (Saturation, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-saturationcontrol-grant")])),
                                (Hue, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-huecontrol-grant")])),
                                (Luminance, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-luminancecontrol-grant")])),
                                (Gamma, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-gammacontrol-grant")])),
                                (NoiseReduction2D, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-noisereduction2d-grant"), (R22, "r14-ptzopticsg3-noisereduction2dcontrol-grant")])),
                                (NoiseReduction2DMode, $crate::source_evidence::source!([(R22, "r14-ptzopticsg3-noisereduction2dmode-grant")])),
                                (NoiseReduction3D, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-noisereduction3d-grant"), (R22, "r14-ptzopticsg3-noisereduction3dcontrol-grant")])),
                                (PictureEffect, $crate::source_evidence::source!([(R14, "r14-ptzopticsg3-pictureeffect-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: Some(5678),
                            udp_port: Some(1259),
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(150),
                            inquiry_spacing: std::time::Duration::from_millis(150),
                            command_spacing: std::time::Duration::from_millis(100),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("command_cancel", "No PTZOptics source (R1, R10, R14) documents the standard `8x 2p FF` socket-cancel command and G3 was not on the bench, so it stays unavailable under the source rule. The G2 family, which shares this command set, rejects it on the bench."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "R10 and R14 do not document a G3 `09 00 02` reply layout, and the G2 family replies with an unsourced 2-byte payload. G3 was not bench-tested; the typed Sony-format version inquiry stays absent under the source-evidence rule until a source or G3 hardware evidence establishes the reply."),
                        ("focus_zones", "The R14 Focus page (R20), which PTZOptics describes as the full G2/G3 VISCA list, documents only the `CAM_AFZone` values Top `00`, Center `01` and Bottom `02`. `FocusZone::Zone03` was observed only on the PTZOptics G2 bench (#795); G3 was not bench-tested, so the typed setter refuses it before any I/O."),
                        ("digital_zoom", "PTZOptics built-ins keep VISCA digital zoom unavailable until model-specific evidence exists."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                        ("iris", "R10 and the current PTZOptics G2/G3 Developer Portal (R14 in docs/visca_reference.md) independently document G3 `04 39`, iris reset/up/down, direct `04 4B`, and `09 04 4B` position inquiry. They omit the distinct `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("vendor_controls", "The PTZOptics Move 4K G3 command manual (R10 in docs/visca_reference.md) documents the anti-flicker, settings-save, preset-recall-speed, multicast-streaming, and NDI-quality command families retained for G3."),
                        ("focus_zone_inquiry", "The G3 command list establishes focus-zone selection, but its query table does not establish the matching 81 09 04 AA response; keep the inquiry untyped."),
                        ("usb_audio", "The UAC command/query table is source-backed for the Gen-2 entries, not G3; keep USB audio conservative pending a G3-specific source."),
                        ("picture_effect", "PTZOptics' official Developer Portal identifies its current VISCA list for G2 and G3 and documents the 04 63 picture-effect command and inquiry there."),
                        ("noise_reduction", "R14 documents G2/G3 04 50 Auto/Manual control plus 04 53 0/off and 1..5 and 04 54 0/off and 1..8 controls; its separate query page lists 09 04 50 and 09 04 53/54 replies. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by the query output domain."),
                        ],
                }

                profile PtzOptics30x {
                    doc: "PtzOptics legacy PT30X SDI/NDI G2 raw-VISCA camera profile.",
                    id: "ptzoptics_30x",
                    id_doc: "PTZOptics legacy PT30X SDI/NDI G2 (Gen-2 raw-VISCA) camera",
                    vendor: "PtzOptics",
                    description: "Legacy PT30X SDI/NDI G2 raw-VISCA profile with 30x optical zoom",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "PtzOptics 30X",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -2448, 2448),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -432, 1296)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                                        pan_degrees_to_units: 14.4,
                                        tilt_degrees_to_units: 14.4,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 24),
                                tilt_speed: range!(u8, 1, 20),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: Some(30.0),
                            },
                            focus: {
                                range: range!(u16, 0x0000, 0xFFFF),
                                near_end: $crate::capabilities::FocusNearEnd::Unknown,
                                auto_focus: true,
                                one_push: false,
                                zone_inquiry: true,
                                zones: (profile_constants::PTZ_OPTICS_G2_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x0C)),
                                shutter_speeds: (profile_constants::PTZ_OPTICS_G2_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0, 7)),
                                brightness_range: Some(domain!(u8, 0, 17)),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: true,
                            },
                            white_balance: {
                                modes: (profile_constants::COLOR_TEMP_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: Some(range!(u16, 2500, 8000)),
                                color_temperature_steps: std::sync::Arc::from(profile_constants::PTZ_COLOR_TEMPERATURE_STEPS),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: {
                                    red: range!(i8, -10, 10),
                                    blue: range!(i8, -10, 10),
                                },
                            },
                            image: {
                                contrast_range: Some(range!(u8, 0, 14)),
                                sharpness_range: Some(range!(u8, 0, 15)),
                                saturation_range: Some(range!(u8, 0, 14)),
                                flip: true,
                                mirror: true,
                                hue_range: Some(range!(u8, 0, 14)),
                                picture_effects: vec![$crate::types::PictureEffectMode::Off, $crate::types::PictureEffectMode::BlackAndWhite],
                                luminance_range: Some(range!(u8, 0, 14)),
                                requires_settings_save_for_flip: true,
                                gamma_range: Some(range!(u8, 0, 4)),
                            },
                            presets: {
                                highest: 127,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 24)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(10),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Full,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R1, "r1-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-powerstandby-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-shutterdirect-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-gaincontrol-grant")])),
                                (GainLimit, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-gainlimit-grant")])),
                                (MenuEnterReturn, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-menuenterreturn-grant")])),
                                (MenuDisplayInquiry, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-menustatusinquiry-grant")])),
                                (AntiFlicker, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-ptzopticsantiflicker-grant")])),
                                (SettingsSave, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-ptzopticssettingssave-grant")])),
                                (PresetRecallSpeed(PtzOptics), $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-ptzopticspresetrecallspeed-grant")])),
                                (MulticastStreaming, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-ptzopticsmulticaststreaming-grant")])),
                                (NdiQuality, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-ptzopticsndiquality-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-exposuremode-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-exposurecompensation-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-brightnesscontrol-grant")])),
                                (FocusLock, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-focuslock-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-directzoom-grant")])),
                                (Iris, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-iriscontrol-grant")])),
                                (FocusZone, $crate::source_evidence::source!([(B1, "b1-ptzoptics30x-focuszone-grant")])),
                                (FocusZoneInquiry, $crate::source_evidence::source!([(B1, "b1-ptzoptics30x-focuszoneinquiry-grant")])),
                                (UsbAudio, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-usbaudio-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-backlightcompensation-grant")])),
                                (WideDynamicRange, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-widedynamicrange-grant")])),
                                (ColorTemperature, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-colortemperature-grant")])),
                                (ColorTemperatureInquiry, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-colortemperatureinquiry-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-rgbgain-grant")])),
                                (RgbTuning, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-rgbtuning-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-onepushwhitebalance-grant")])),
                                (AutoWhiteBalanceSensitivity, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-autowhitebalancesensitivity-grant")])),
                                (ImageFlip, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-imageflip-grant")])),
                                (ImageMirror, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-imagemirror-grant")])),
                                (CombinedImageFlip, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-combinedimageflip-grant")])),
                                (Contrast, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-contrastcontrol-grant")])),
                                (Sharpness, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-sharpnesscontrol-grant")])),
                                (SharpnessMode, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-sharpnessmode-grant")])),
                                (Saturation, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-saturationcontrol-grant")])),
                                (Hue, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-huecontrol-grant")])),
                                (Luminance, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-luminancecontrol-grant")])),
                                (Gamma, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-gammacontrol-grant")])),
                                (NoiseReduction2D, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-noisereduction2d-grant"), (R22, "r14-ptzoptics30x-noisereduction2dcontrol-grant")])),
                                (NoiseReduction2DMode, $crate::source_evidence::source!([(R22, "r14-ptzoptics30x-noisereduction2dmode-grant")])),
                                (NoiseReduction3D, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-noisereduction3d-grant"), (R22, "r14-ptzoptics30x-noisereduction3dcontrol-grant")])),
                                (PictureEffect, $crate::source_evidence::source!([(R1, "r1-ptzoptics30x-pictureeffect-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: Some(5678),
                            udp_port: Some(1259),
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(150),
                            inquiry_spacing: std::time::Duration::from_millis(150),
                            command_spacing: std::time::Duration::from_millis(100),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: true,
                        supports_command_cancel: false,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("command_cancel", "On the PTZOptics G2 bench (PT30X-NDI G2 firmware ARM 6.3.51THI and PT12X-NDI G2 firmware ARM 6.4.18SHI; 2026-10-05), a socket cancel `81 2y FF` sent while a slow absolute pan/tilt move ran on socket y (ACK `90 4y`) was answered `90 60 02 FF` (syntax error) and the move continued to its target and completed (`90 5y`): the G2 family does not implement socket cancel."),
                        ("focus_range", "R1 and R14 document `81 01 04 48 0p 0q 0r 0s FF` direct focus and the `09 04 48` position reply with `pqrs = Focus Position` but no numeric bounds; the `0x1000..=0xF000` range in docs/visca_reference.md is Axis-only (R6). The profile therefore admits the full 16-bit wire domain and leaves the physical limit to the camera."),
                        ("version_inquiry", "On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795), the legacy G2 models return the 2-byte payload `00 52` for `81 09 00 02 FF`, not the 7-byte Sony layout, and no PTZOptics source documents the reply. Keep the typed version inquiry absent."),
                        ("digital_zoom", "The Axis 0x7AC0 digital endpoint is not applied to PTZOptics; the 30X raw VISCA profile keeps the standard 0x4000 optical endpoint and no typed digital zoom."),
                        ("preset_limit", "Raw PTZOptics VISCA preset commands are limited to the documented 0-127 range until values above 0x7F are target-tested."),
                        ("iris", "The shared PTZOptics Gen-2 source documents iris priority, relative controls, direct `04 4B`, and matching `09 04 4B` position inquiry for the raw 30X profile. It does not establish a model-specific `09 04 2B` iris auto/manual status inquiry, so `IrisControlInquiry` remains absent."),
                        ("focus_zone_inquiry", "The PTZOptics Gen-2 table documents the 81 09 04 AA focus-zone inquiry and its status response for the raw 30X model."),
                        ("focus_zones", "R14/R20 document `CAM_AFZone` Top `00`, Center `01` and Bottom `02`. On the PTZOptics G2 bench (PT30X/PT20X/PT12X-NDI G2, firmware ARM 6.3.51THI, 6.3.76THI, 6.4.18SHI; 2026-10-04; #795) the cameras also report `03`, accept `81 01 04 AA 03 FF` with ACK and completion, and read `03` back, so `FocusZone::Zone03` is admitted for this family."),
                        ("usb_audio", "The PTZOptics Gen-2 UAC table documents the USB-audio command and matching inquiry for the raw 30X model."),
                        ("noise_reduction", "R1 supplies legacy PT30X SDI/NDI G2 09 04 50/53/54 inquiries, including 3D 0..8, but no setters. R14 supplies 04 50/53/54 controls only because R15 explicitly narrows this profile to that legacy PT30X SDI/NDI G2 raw-VISCA family. Encode domains are independently 2D 0..5 and 3D 0..8, not constrained by query output domains; do not generalize this evidence to newer 30X products."),
                        ],
                }

                profile SonyFr7 {
                    doc: "Sony FR7 camera profile.",
                    id: "sony_fr7",
                    id_doc: "Sony FR7 camera",
                    vendor: "Sony",
                    description: "Professional cinema camera with variable ND filter and model-specific exposure controls",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Sony FR7",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -0x09CA7, 0x09CA7),
                                    tilt_ranges: $crate::capabilities::TiltRanges::MountDependent {
                                            desktop: range!(i32, -0x1BA5, 0xB3B0),
                                        ceiling: range!(i32, -0xC183, 0xDD2),
                                    },
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyFr7,
                                        pan_degrees_to_units: -235.9,
                                        tilt_degrees_to_units: 235.9,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::WholeDegrees,
                                },
                                pan_speed: range!(u8, 1, 50),
                                tilt_speed: range!(u8, 1, 50),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: true,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x0000, 0xFFFF),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: false,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::NO_SHARED_EXPOSURE_MODES).to_vec(),
                                iris_range: None,
                                shutter_speeds: (profile_constants::NO_SHUTTER_SPEEDS).to_vec(),
                                gain_range: None,
                                brightness_range: None,
                                backlight_compensation: true,
                                compensation_range: None,
                                wide_dynamic_range: false,
                            },
                            white_balance: {
                                modes: (&[$crate::WhiteBalanceMode::Atw, $crate::WhiteBalanceMode::Manual, $crate::WhiteBalanceMode::Preset]).to_vec(),
                                one_push: true,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: None,
                                rgb_tuning: None,
                            },
                            image: {
                                contrast_range: None,
                                sharpness_range: Some(range!(u8, 0, 14)),
                                saturation_range: None,
                                flip: false,
                                mirror: false,
                                hue_range: None,
                                picture_effects: vec![],
                                luminance_range: None,
                                requires_settings_save_for_flip: false,
                                gamma_range: None,
                            },
                            presets: {
                                highest: 0x63,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 50)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(15),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: true,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Full,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R7, "r7-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R7, "r7-sonyfr7-powerstandby-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R7, "r7-sonyfr7-presetmemory-grant")])),
                                (MenuDisplay, $crate::source_evidence::source!([(R7, "r7-sonyfr7-menudisplay-grant")])),
                                (MenuDisplayInquiry, $crate::source_evidence::source!([(R7, "r7-sonyfr7-menustatusinquiry-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R7, "r7-sonyfr7-versioninquiry-grant")])),
                                (Spotlight, $crate::source_evidence::source!([(R7, "r7-sonyfr7-sonyspotlight-grant")])),
                                (PushAutoFocus, $crate::source_evidence::source!([(R7, "r7-sonyfr7-pushautofocus-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R7, "r7-sonyfr7-directzoom-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R7, "r7-sonyfr7-backlightcompensation-grant")])),
                                (Sharpness, $crate::source_evidence::source!([(R7, "r7-sonyfr7-sharpnesscontrol-grant")])),
                                (Tally, $crate::source_evidence::source!([(R7, "r7-sonyfr7-tally-grant")])),
                                (DirectMenu, $crate::source_evidence::source!([(R7, "r7-sonyfr7-directmenu-grant")])),
                                (NdFilter, $crate::source_evidence::source!([(R7, "r7-sonyfr7-ndfilter-grant")])),
                                (PanTiltSpeedMode, $crate::source_evidence::source!([(R7, "r7-sonyfr7-variablespeed-grant")])),
                                (AeLevel, crate::source_evidence::source!(R7, "r7-grant-sonyfr7aelevel")),
                                (AutoGain, crate::source_evidence::source!(R7, "r7-grant-sonyfr7autogain")),
                                (AutoShutter, crate::source_evidence::source!(R7, "r7-grant-sonyfr7autoshutter")),
                                (IrisSteps, crate::source_evidence::source!(R7, "r7-grant-irissteps")),
                                (AutoIris, crate::source_evidence::source!(R7, "r7-grant-autoiris")),
                                (OnePushWhiteBalance(SonyFr7), crate::source_evidence::source!(R7, "r7-grant-onepushwhitebalance")),
                                (PresetWhite, crate::source_evidence::source!(R7, "r7-grant-sonyfr7presetwhite")),
                                (MemoryColorTemperature, crate::source_evidence::source!(R7, "r7-grant-sonyfr7memorycolortemperature")),
                                (MemoryGain, crate::source_evidence::source!(R7, "r7-grant-sonyfr7memorygain")),
                                (MemoryTint, crate::source_evidence::source!(R7, "r7-grant-sonyfr7memorytint")),
                                (AtwOffsets, crate::source_evidence::source!(R7, "r7-grant-sonyfr7atwoffsets")),
                                (BlackLevels, crate::source_evidence::source!(R7, "r7-grant-sonyfr7black")),
                                (Detail, crate::source_evidence::source!(R7, "r7-grant-sonyfr7detail")),
                                (Knee, crate::source_evidence::source!(R7, "r7-grant-sonyfr7knee")),
                                (HighResolutionZoom, crate::source_evidence::source!(R7, "r7-grant-sonyfr7highresolutionzoom")),
                                (ClearImageZoom, crate::source_evidence::source!(R7, "r7-grant-sonyfr7clearimagezoom")),
                                (SeparatePresetSpeed, crate::source_evidence::source!(R7, "r7-grant-separatepresetspeed")),
                                (PresetRecallSpeed(SonyFr7), crate::source_evidence::source!(R7, "r7-grant-presetrecallspeed")),
                                (PresetRecallSpeedInquiry, crate::source_evidence::source!(R7, "r7-grant-presetrecallspeedinquiry")),
                                (SeparatePresetDuration, crate::source_evidence::source!(R7, "r7-grant-sonyfr7presetduration")),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: None,
                            udp_port: Some(52381),
                            serial: false,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::SonyEncapsulated,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: profile_registry::command_timeouts_ms(8000, 30000, 60000, 300000, 8000),
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(35),
                            inquiry_spacing: std::time::Duration::from_millis(35),
                            command_spacing: std::time::Duration::from_millis(35),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::ND_FILTER)))),
                    },
                    withheld: [],
                    evidence: [
                        ("shutter", "No model-specific shutter-code inventory has been lowered from R7 into this profile. It advertises no shared shutter codes, so typed shutter positions are refused rather than sent with another model's codes. Use the raw command path."),
                        ("transport", "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header."),
                        ("version_inquiry", "The FR7 VISCA command list (R7) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (051E ILME-FR7/FR7K), ROM revision and maximum socket (02)."),
                        ("sony_spotlight", "The FR7 command list (R7 in docs/visca_reference.md) documents the fixed 04 3A spotlight commands, but not the fixed 04 5A auto slow-shutter commands."),
                        ("tally", "Sony professional profile metadata and typed controls expose tally for FR7."),
                        ("color_temperature", "FR7 has dedicated preset-white and Memory A color-temperature families. The shared color-temperature command and inquiry remain unsupported; use the FR7-specific typed values and noun methods."),
                        ("iris", "R7 documents the typed FR7 iris Up/Down family under 7E 04 4B and Auto Iris command/inquiry under 05 34. It does not establish the shared 04 39 AE-mode family, standard absolute iris-direct command, or 09 04 4B position inquiry, so those shared typed surfaces remain absent."),
                        ("nd_filter", "R7 Sony ILME-FR7/FR7K VISCA Command List v4.00 p.21 documents variable-ND inquiry 8x 09 7E 04 42 FF with y0 50 00 00 0p 0p FF; p.25 codes 00..14 mean density 1/4..1/128. The legacy 09 04 64 row is not a sourced FR7 inquiry."),
                        ("brightness", "The FR7 model command list does not establish the exposure-brightness control or inquiry; retain no brightness range or typed marker."),
                        ("focus_zone", "The FR7 model command list does not establish focus-zone selection or its inquiry; keep both typed surfaces absent."),
                        ("af_sensitivity", "The FR7 command list R7 documents Push AF/MF under `7E 04 58`, but not the shared `04 58` autofocus-sensitivity command or inquiry; keep the shared metadata and typed surface absent."),
                        ("variable_speed", "The FR7 command list R7 documents the `06 45` normal/extended pan/tilt speed-step range. Its separate `7E 04 1B` family selects preset-speed behavior and is not used by this typed surface."),
                        ("noise_reduction", "The FR7 command list R7 does not establish the shared `04 50`/`04 53`/`04 54` noise-reduction family; keep the shared metadata and typed surfaces absent."),
                        ("picture_effect", "The FR7 model command list does not establish picture-effect control or inquiry; leave the typed surface unavailable."),
                        ],
                }

                profile SonyBrcH900 {
                    doc: "Sony BRC-H900 camera profile.",
                    id: "sony_brc_h900",
                    id_doc: "Sony BRC-H900 camera",
                    vendor: "Sony",
                    description: "Professional PTZ camera with advanced image processing and 100 presets",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Sony BRC-H900",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -0x09BDE, 0x09BDE),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -0x1BA8, 0x52F8)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                                        pan_degrees_to_units: -236.0,
                                        tilt_degrees_to_units: 236.0,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 24),
                                tilt_speed: range!(u8, 1, 24),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: false,
                                variable: true,
                                optical_ratio: Some(14.0),
                            },
                            focus: {
                                range: range!(u16, 0x1000, 0xC000),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: false,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::BRC_H900_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x19)),
                                shutter_speeds: (profile_constants::NO_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0, 0x19)),
                                brightness_range: None,
                                backlight_compensation: false,
                                compensation_range: None,
                                wide_dynamic_range: false,
                            },
                            white_balance: {
                                modes: (profile_constants::STANDARD_WB_MODES).to_vec(),
                                one_push: false,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: None,
                                rgb_tuning: None,
                            },
                            image: {
                                contrast_range: None,
                                sharpness_range: None,
                                saturation_range: None,
                                flip: false,
                                mirror: false,
                                hue_range: None,
                                picture_effects: vec![],
                                luminance_range: None,
                                requires_settings_save_for_flip: false,
                                gamma_range: None,
                            },
                            presets: {
                                highest: 0x0F,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: None,
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Extended,
                                on_time: std::time::Duration::from_secs(12),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Full,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R11, "r11-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R11, "r11-sonybrch900-powerstandby-grant")])),
                                (FocusInfinity, $crate::source_evidence::source!([(R11, "r11-sonybrch900-focusinfinity-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R11, "r11-sonybrch900-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R11, "r11-sonybrch900-gaincontrol-grant")])),
                                (MenuDisplayInquiry, $crate::source_evidence::source!([(R11, "r11-sonybrch900-menustatusinquiry-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R11, "r11-sonybrch900-versioninquiry-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R11, "r11-sonybrch900-exposuremode-grant")])),
                                (Iris, $crate::source_evidence::source!([(R11, "r11-sonybrch900-iriscontrol-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: None,
                            udp_port: Some(52381),
                            serial: false,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::SonyEncapsulated,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: profile_registry::command_timeouts_ms(6000, 30000, 60000, 300000, 6000),
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(35),
                            inquiry_spacing: std::time::Duration::from_millis(35),
                            command_spacing: std::time::Duration::from_millis(35),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("shutter", "No model-specific shutter-code inventory has been lowered from R11 into this profile. It advertises no shared shutter codes, so typed shutter positions are refused rather than sent with another model's codes. Use the raw command path."),
                        ("transport", "Sony VISCA-over-IP uses UDP 52381 with Sony's 8-byte encapsulation header."),
                        ("version_inquiry", "The BRC-H900 command list (R11) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (050B BRC-H900), ROM revision and maximum socket (02)."),
                        ("sony_spotlight", "The BRC-H900 command list (R11 in docs/visca_reference.md) documents the fixed 04 3A spotlight commands, but not the fixed 04 5A auto slow-shutter commands."),
                        ("tally", "The BRC-H900 source-backed command list does not establish the FR7 red/green tally family; typed tally remains unavailable."),
                        ("exposure_mode", "The BRC-H900 command list R11 lines 706-717 and 1003 documents the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri commands and `09 04 39` inquiry. Bright mode is not listed and is therefore absent from this profile's inventory."),
                        ("iris", "The BRC-H900 command list R11 lines 706-717 and 1012 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry. The distinct `09 04 2B` status inquiry remains unavailable."),
                        ("brightness", "The BRC-H900 model command list does not establish the exposure-brightness control or inquiry; retain no brightness range or typed marker."),
                        ("noise_reduction", "The BRC-H900 model command list R11 does not establish the shared `04 50`/`04 53`/`04 54` noise-reduction family, so both discovery metadata and typed surfaces remain unavailable."),
                        ("picture_effect", "The BRC-H900 model command list does not establish picture-effect control or inquiry; leave the typed surface unavailable."),
                        ],
                }

                profile SonyEviH100 {
                    doc: "Sony EVI-H100 camera profile.",
                    id: "sony_evi_h100",
                    id_doc: "Sony EVI-H100 camera",
                    vendor: "Sony",
                    description: "Compact HD PTZ camera with basic feature set",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Sony EVI-H100",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -0x1E1B, 0x1E1B),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -0x038B, 0x0FF0)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::StandardVisca,
                                        pan_degrees_to_units: 7707.0 / 170.0,
                                        tilt_degrees_to_units: 7707.0 / 170.0,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::WholeDegrees,
                                },
                                pan_speed: range!(u8, 1, 0x18),
                                tilt_speed: range!(u8, 1, 0x17),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: Some(0x7AC0),
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x1000, 0xF000),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: true,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: true,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x11, gaps: [0x01, 0x02, 0x03, 0x04])),
                                shutter_speeds: (profile_constants::SONY_EVI_H100_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0x00, 0x0F)),
                                brightness_range: Some(domain!(u8, 0x00, 0x1F, gaps: [0x01, 0x02, 0x03, 0x04])),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: true,
                            },
                            white_balance: {
                                modes: (profile_constants::STANDARD_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: None,
                            },
                            image: {
                                contrast_range: None,
                                sharpness_range: Some(range!(u8, 0x00, 0x0F)),
                                saturation_range: Some(range!(u8, 0x00, 0x0E)),
                                flip: false,
                                mirror: false,
                                hue_range: Some(range!(u8, 0x00, 0x0E)),
                                picture_effects: vec![$crate::types::PictureEffectMode::Off, $crate::types::PictureEffectMode::BlackAndWhite],
                                luminance_range: None,
                                requires_settings_save_for_flip: false,
                                gamma_range: Some(range!(u8, 0, 4)),
                            },
                            presets: {
                                highest: 5,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: None,
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(8),
                                standby: Some(std::time::Duration::from_secs(1)),
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Partial,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R8, "r8-pantiltpositioning-grant")])),
                                (PowerStandby, $crate::source_evidence::source!([(R8, "r8-sonyevih100-powerstandby-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R8, "r8-sonyevih100-shutterdirect-grant")])),
                                (FocusInfinity, $crate::source_evidence::source!([(R8, "r8-sonyevih100-focusinfinity-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R8, "r8-sonyevih100-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R8, "r8-sonyevih100-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R8, "r8-sonyevih100-gaincontrol-grant")])),
                                (Sharpness, $crate::source_evidence::source!([(R8, "r8-sonyevih100-sharpnesscontrol-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R8, "r8-sonyevih100-versioninquiry-grant")])),
                                (AutoSlowShutter, $crate::source_evidence::source!([(R8, "r8-sonyevih100-sonyautoslowshutter-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R8, "r8-sonyevih100-exposuremode-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R8, "r8-sonyevih100-directzoom-grant")])),
                                (DigitalZoomToggle, $crate::source_evidence::source!([(R8, "r8-sonyevih100-digitalzoomtoggle-grant")])),
                                (DigitalZoomRange, $crate::source_evidence::source!([(R8, "r8-sonyevih100-digitalzoomrange-grant")])),
                                (OnePushFocus, $crate::source_evidence::source!([(R8, "r8-sonyevih100-onepushfocus-grant")])),
                                (Iris, $crate::source_evidence::source!([(R8, "r8-sonyevih100-iriscontrol-grant")])),
                                (FocusNearLimit, $crate::source_evidence::source!([(R8, "r8-sonyevih100-focusnearlimitinquiry-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R8, "r8-sonyevih100-brightnesscontrol-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R8, "r8-sonyevih100-exposurecompensation-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R8, "r8-sonyevih100-backlightcompensation-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R8, "r8-sonyevih100-rgbgain-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R8, "r8-sonyevih100-onepushwhitebalance-grant")])),
                                (Saturation, $crate::source_evidence::source!([(R8, "r8-sonyevih100-saturationcontrol-grant")])),
                                (Hue, $crate::source_evidence::source!([(R8, "r8-sonyevih100-huecontrol-grant")])),
                                (Gamma, $crate::source_evidence::source!([(R8, "r8-sonyevih100-gammacontrol-grant")])),
                                (NoiseReduction2D, $crate::source_evidence::source!([(R8, "r8-sonyevih100-noisereduction2d-grant"), (R8, "r8-sonyevih100-noisereduction2dcontrol-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: None,
                            udp_port: None,
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(240),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(0),
                            inquiry_spacing: std::time::Duration::from_millis(0),
                            command_spacing: std::time::Duration::from_millis(0),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [WideDynamicRange, PictureEffect],
                    evidence: [
                        ("transport", "The EVI-H100S/H100V technical manual (R8) documents VISCA over RS-232C and RS-422 at 9,600 or 38,400 bps; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The EVI-H100S/H100V technical manual (R8) shutter table (VISCA Command Setting Values, exposure control 1/2), 60/30 mode: `00` 1/1 s through `15` 1/10000 s. The 50/25 mode assigns several codes other times (for example `13` 1/3500 s); the profile uses the 60/30 column."),
                        ("one_push_focus", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF`."),
                        ("digital_zoom", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF`, inquiry list (1/3) `CAM_DZoomModeInq` `8x 09 04 06 FF`, and the zoom position table (p. 44): optical `0000`..`4000` (x1..x20) and digital `4000`..`7AC0` (x1..x12), all reached with `CAM_Zoom` Direct `04 47`."),
                        ("af_sensitivity", "The EVI-H100S/H100V technical manual (R8) documents AF Sensitivity Normal `8x 01 04 58 02 FF` and Low `03` only; the typed surface also sends High `01`, which R8 does not list, so the metadata and typed surface stay unavailable."),
                        ("presets", "The EVI-H100S/H100V technical manual (R8) `CAM_Memory` reset/set/recall with memory number p = 0 to 5; it documents no preset recall speed."),
                        ("pan_tilt", "The EVI-H100S/H100V technical manual (R8) command list (4/4) and position table (p. 45): pan `E1E5`..`1E1B` (-170..+170 degrees), tilt `FC75`..`0FF0` (-20..+90 degrees, the range used here) with IMAGE FLIP off (`F010`..`038B` with it on), pan speed `01`..`18`, tilt speed `01`..`17`. One scale, 7707 units per 170 degrees, maps both axes; tilt `FC75` is then -20.0 and `0FF0` +90.0 degrees to within 0.01 degree."),
                        ("gain", "The EVI-H100S/H100V technical manual (R8) gain table (exposure control 1/2): positions `00` (-3 dB) through `0F` (+28 dB)."),
                        ("exposure_compensation", "The EVI-H100S/H100V technical manual (R8) command list (2/4) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`; inquiry list (1/3) `09 04 3E` and `09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The EVI-H100S/H100V technical manual (R8) command list (2/4) `CAM_Bright` up/down `04 0D` and direct `04 4D 00 00 0p 0q`; inquiry `09 04 4D`. Its Bright table (p. 45) lists `00` and `05`..`1F`; positions `01`..`04` are not listed, so the domain excludes them."),
                        ("rgb_gain", "The EVI-H100S/H100V technical manual (R8) command list (1/4) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with R/B gain `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("wide_dynamic_range", "The EVI-H100S/H100V technical manual (R8) documents Wide-D as `CAM_WD` `8x 01 04 3D` with inquiry `09 04 3D`, so the discovery metadata reports it; the typed `WideDynamicRange` surface encodes the different `04 25` dynamic-range command, which R8 does not list, so the typed surface is withheld."),
                        ("exposure_mode", "The EVI-H100S/H100V technical manual (R8) command list (2/4) documents `CAM_AE` Full Auto/Manual/Shutter Priority/Iris Priority/Bright `8x 01 04 39 00/03/0A/0B/0D FF` and inquiry list (1/3) `CAM_AEModeInq` `8x 09 04 39 FF`."),
                        ("iris", "The EVI-H100S/H100V technical manual (R8) command list (2/4) documents `CAM_Iris` reset/up/down `8x 01 04 0B 00/02/03 FF` and direct `8x 01 04 4B 00 00 0p 0q FF`, inquiry list (1/3) `CAM_IrisPosInq` `8x 09 04 4B FF`, and the iris table (p. 44) `00` CLOSE and `05`..`11` (F14..F1.6); positions `01`..`04` are not listed, so the domain excludes them. The distinct `09 04 2B` status inquiry is not listed."),
                        ("busy_timeout", "The EVI-H100S/H100V technical manual (R8) command list note 4: after the completion of a `CAM_Memory` recall, a following command may be answered `Command not executable` for at most 240 ms; the camera asks for it to be sent again."),
                        ("white_balance", "The EVI-H100S/H100V technical manual (R8) `CAM_WB` lists Auto, Indoor, Outdoor, One Push and Manual with the `04 10 05` one-push trigger; it has no color-temperature mode or `04 20` color-temperature command, and its `04 43`/`04 44` rows are absolute R/B gain `00..FF`, not the signed red/blue tuning surface."),
                        ("image_flip", "The EVI-H100S/H100V technical manual (R8) sets image flip with the rear IMAGE FLIP switch only; it lists no `04 61`/`04 66` VISCA flip or mirror command."),
                        ("version_inquiry", "The EVI-H100S/H100V technical manual (R8) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (050E EVI-H100V, 050F EVI-H100S), ROM revision and maximum socket (02)."),
                        ("sony_auto_slow_shutter", "The EVI-H100 technical manual (R8 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("noise_reduction", "The EVI-H100S/H100V technical manual (R8) command list (3/4) `CAM_NR` `8x 01 04 53 0p FF` (p: 0 off, levels 1 to 5) and inquiry list `CAM_NRInq` `8x 09 04 53 FF` -> `y0 50 0p FF`: exactly the 2D noise-reduction level control and inquiry. R8 lists no `04 50` auto/manual mode and no `04 54` 3D level, so those surfaces stay unavailable."),
                        ("saturation_control", "The EVI-H100S/H100V technical manual (R8) `CAM_ColorGain` Direct `8x 01 04 49 00 00 00 0p FF` and `CAM_ColorGainInq` `8x 09 04 49 FF` -> `y0 50 00 00 00 0p FF`, p `0h` (60%) to `Eh` (200%): the saturation control and inquiry byte for byte."),
                        ("hue_control", "The EVI-H100S/H100V technical manual (R8) `CAM_ColorHue` Direct `8x 01 04 4F 00 00 00 0p FF` and `CAM_ColorHueInq` `8x 09 04 4F FF` -> `y0 50 00 00 00 0p FF`, p `0h` (-14 degrees) to `Eh` (+14 degrees): the hue control and inquiry byte for byte."),
                        ("sharpness_control", "The EVI-H100S/H100V technical manual (R8) `CAM_Aperture` reset/up/down `8x 01 04 02 00/02/03 FF`, direct `8x 01 04 42 00 00 0p 0q FF` and `CAM_ApertureInq` `09 04 42`, with Aperture Level `00`..`0F`, so level/step control and its typed inquiry are enabled. R8 does not list the independent `04 05` sharpness auto/manual mode or `09 04 05` inquiry; SharpnessMode remains withheld."),
                        ("picture_effect", "The EVI-H100S/H100V technical manual (R8) `CAM_PictureEffect` Off/Neg.Art/B&W `8x 01 04 63 00/02/04 FF`, so the discovery metadata reports it. Its `CAM_PictureEffectModeInq` reports Off as `00` and Neg.Art as `02`, while the typed inquiry decodes `02` as Off (the PTZOptics layout) and has no Neg.Art mode, so the typed surface is withheld."),
                        ],
                }

                profile SonyBrc300 {
                    doc: "Sony BRC-300 camera profile.",
                    id: "sony_brc_300",
                    id_doc: "Sony BRC-300 camera",
                    vendor: "Sony",
                    description: "Legacy PTZ camera with signed-centered 20-bit pan and 16-bit tilt coordinates",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Sony BRC-300",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -0x08A58, 0x08A58),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -0x186A, 0x493D)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                                        pan_degrees_to_units: -208.0,
                                        tilt_degrees_to_units: 208.0,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 0x18),
                                tilt_speed: range!(u8, 1, 0x18),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: Some(0x7F00),
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x1000, 0xC000),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: true,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x11)),
                                shutter_speeds: (profile_constants::SONY_BRC300_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0x00, 0x07)),
                                brightness_range: Some(domain!(u8, 0x00, 0x17)),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: false,
                            },
                            white_balance: {
                                modes: (profile_constants::STANDARD_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: None,
                            },
                            image: {
                                contrast_range: None,
                                sharpness_range: None,
                                saturation_range: None,
                                flip: true,
                                mirror: false,
                                hue_range: None,
                                picture_effects: vec![],
                                luminance_range: None,
                                requires_settings_save_for_flip: false,
                                gamma_range: None,
                            },
                            presets: {
                                highest: 5,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 24)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(10),
                                standby: None,
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Partial,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R12, "r12-pantiltpositioning-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R12, "r12-sonybrc300-shutterdirect-grant")])),
                                (FocusInfinity, $crate::source_evidence::source!([(R12, "r12-sonybrc300-focusinfinity-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R12, "r12-sonybrc300-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R12, "r12-sonybrc300-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R12, "r12-sonybrc300-gaincontrol-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R12, "r12-sonybrc300-versioninquiry-grant")])),
                                (AutoSlowShutter, $crate::source_evidence::source!([(R12, "r12-sonybrc300-sonyautoslowshutter-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R12, "r12-sonybrc300-exposuremode-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R12, "r12-sonybrc300-directzoom-grant")])),
                                (DigitalZoomToggle, $crate::source_evidence::source!([(R12, "r12-sonybrc300-digitalzoomtoggle-grant")])),
                                (DigitalZoomRange, $crate::source_evidence::source!([(R12, "r12-sonybrc300-digitalzoomrange-grant")])),
                                (OnePushFocus, $crate::source_evidence::source!([(R12, "r12-sonybrc300-onepushfocus-grant")])),
                                (Iris, $crate::source_evidence::source!([(R12, "r12-sonybrc300-iriscontrol-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R12, "r12-sonybrc300-brightnesscontrol-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R12, "r12-sonybrc300-exposurecompensation-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R12, "r12-sonybrc300-backlightcompensation-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R12, "r12-sonybrc300-rgbgain-grant")])),
                                (ImageFlip, $crate::source_evidence::source!([(R12, "r12-sonybrc300-imageflip-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R12, "r12-sonybrc300-onepushwhitebalance-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: None,
                            udp_port: None,
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(0),
                            inquiry_spacing: std::time::Duration::from_millis(0),
                            command_spacing: std::time::Duration::from_millis(0),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("zoom", "The BRC-300 technical manual (R12) `CAM_Zoom` Direct `8x 01 04 47 0p 0q 0r 0s FF` and its zoom position table (x12 lens): optical `0000` (x1) through `4000` (x12), digital `4000` (x1) through `7F00` (x4)."),
                        ("digital_zoom", "The BRC-300 technical manual (R12) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF` and `CAM_DZoomModeInq` `8x 09 04 06 FF` (`y0 50 02/03 FF`); combined optical-plus-digital positions `4000`..`7F00` are reached with `CAM_Zoom` Direct `04 47` in Combine mode (`04 36 00`) with the x4 E-Zoom limit (`7E 01 19 01`); with the x2 limit the camera stops at `6A00`."),
                        ("one_push_focus", "The BRC-300 technical manual (R12) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF` (One Push AF Trigger)."),
                        ("transport", "The BRC-300 technical manual (R12) documents VISCA over RS-232C and RS-422; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The BRC-300 technical manual (R12) shutter table (VISCA command setting values, exposure control 1/2), BRC-300 column: `02` 1/4 s through `15` 1/10000 s."),
                        ("image_flip", "The BRC-300 technical manual (R12) `CAM_ImgFlip` on/off `8x 01 04 66 02/03 FF` and `CAM_ImgFlipInq` `8x 09 04 66 FF`. It lists no `04 61` mirror command, so mirror stays unavailable."),
                        ("gain", "The BRC-300 technical manual (R12) gain table (exposure control 1/2): positions `0` (-3 dB) through `7` (18 dB)."),
                        ("exposure_compensation", "The BRC-300 technical manual (R12) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`, inquiries `09 04 3E`/`09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The BRC-300 technical manual (R12) `CAM_Bright` reset/up/down `04 0D` and direct `04 4D 00 00 0p 0q`, inquiry `09 04 4D`. Its Bright table lists `00`..`17`."),
                        ("rgb_gain", "The BRC-300 technical manual (R12) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("one_push_white_balance", "The BRC-300 technical manual (R12) `CAM_WB` lists One Push WB `8x 01 04 35 03 FF` and the One Push trigger `8x 01 04 10 05 FF` (command list 1/4), and `CAM_WBModeInq` reports `y0 50 03 FF` One Push."),
                        ("focus_near_limit", "The BRC-300 technical manual (R12) lists no `04 28` focus near-limit command or `09 04 28` inquiry, so the typed near-limit surface is unavailable."),
                        ("version_inquiry", "The BRC-300 technical manual R12 documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (040F BRC-300), ROM revision and maximum socket (02)."),
                        ("sony_auto_slow_shutter", "The BRC-300 technical manual (R12 in docs/visca_reference.md) documents the fixed 04 5A auto slow-shutter commands, but not the fixed 04 3A spotlight commands."),
                        ("exposure_mode", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document the shared `04 39` Full Auto/Manual/Shutter Pri/Iris Pri/Bright commands and `09 04 39` inquiry."),
                        ("iris", "The BRC-300 technical manual R12 lines 440-454 and 609-617 document standard iris reset/up/down, direct `04 4B`, and the `09 04 4B` position inquiry, and its iris table (exposure control 1/2) lists `00` CLOSE through `11` F1.6. The distinct `09 04 2B` status inquiry remains unavailable."),
                        ("backlight", "The BRC-300 technical manual R12 pp. 11 and 14 (text rows 404-405 and 539-540 in docs/visca_reference.md's linked source) documents `CAM_BackLight` on/off as `8x 01 04 33 02/03 FF` and `CAM_BackLightModeInq` as `8x 09 04 33 FF` with `y0 50 02/03 FF` replies. Those paired rows support the `image()` backlight surface, not a broad image-processing grant."),
                        ],
                }

                profile NearusBrc300 {
                    doc: "Nearus BRC-300 camera profile.",
                    id: "nearus_brc_300",
                    id_doc: "Nearus BRC-300 camera",
                    vendor: "Nearus",
                    description: "Nearus BRC-300 PTZ camera with the BRC-300 signed-centered 20-bit pan and 16-bit tilt framing",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Nearus BRC-300",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: {
                                    pan_range: range!(i32, -0x08A58, 0x08A58),
                                    tilt_ranges: $crate::capabilities::TiltRanges::Single(range!(i32, -0x186A, 0x493D)),
                                    coordinates: {
                                        coordinate_system: $crate::capabilities::CoordinateSystem::SignedCentered,
                                        wire_codec: $crate::capabilities::PanTiltWireCodec::SonyBrc300,
                                        pan_degrees_to_units: -208.0,
                                        tilt_degrees_to_units: 208.0,
                                    },
                                    endpoint_quantization: $crate::capabilities::EndpointQuantization::Exact,
                                },
                                pan_speed: range!(u8, 1, 0x18),
                                tilt_speed: range!(u8, 1, 0x18),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: Some(0x7F00),
                                speed: range!(u8, 0, 7),
                                direct: true,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x1000, 0xC000),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: true,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x11)),
                                shutter_speeds: (profile_constants::SONY_BRC300_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0x00, 0x07)),
                                brightness_range: Some(domain!(u8, 0x00, 0x17)),
                                backlight_compensation: true,
                                compensation_range: Some(range!(i8, -7, 7)),
                                wide_dynamic_range: false,
                            },
                            white_balance: {
                                modes: (profile_constants::STANDARD_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: {
                                    red: range!(u8, 0x00, 0xFF),
                                    blue: range!(u8, 0x00, 0xFF),
                                },
                                rgb_tuning: None,
                            },
                            image: {
                                contrast_range: None,
                                sharpness_range: None,
                                saturation_range: None,
                                flip: true,
                                mirror: false,
                                hue_range: None,
                                picture_effects: vec![],
                                luminance_range: None,
                                requires_settings_save_for_flip: false,
                                gamma_range: None,
                            },
                            presets: {
                                highest: 5,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: Some(range!(u8, 1, 24)),
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(10),
                                standby: None,
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Partial,
                            typed_support: grants([
                                (PanTiltPositioning, $crate::source_evidence::source!([(R21, "r21-pantiltpositioning-grant")])),
                                (DirectShutter, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-shutterdirect-grant")])),
                                (FocusInfinity, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-focusinfinity-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-gaincontrol-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-versioninquiry-grant")])),
                                (AutoSlowShutter, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-sonyautoslowshutter-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-exposuremode-grant")])),
                                (DirectZoom, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-directzoom-grant")])),
                                (DigitalZoomToggle, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-digitalzoomtoggle-grant")])),
                                (DigitalZoomRange, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-digitalzoomrange-grant")])),
                                (OnePushFocus, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-onepushfocus-grant")])),
                                (Iris, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-iriscontrol-grant")])),
                                (Brightness, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-brightnesscontrol-grant")])),
                                (ExposureCompensation, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-exposurecompensation-grant")])),
                                (BacklightCompensation, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-backlightcompensation-grant")])),
                                (RgbGain, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-rgbgain-grant")])),
                                (ImageFlip, $crate::source_evidence::source!([(R21, "r21-nearusbrc300-imageflip-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R21, "r21-nearusbrc300-onepushwhitebalance-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: None,
                            udp_port: None,
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: $crate::CommandTimeouts::DEFAULT,
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(0),
                            inquiry_spacing: std::time::Duration::from_millis(0),
                            command_spacing: std::time::Duration::from_millis(0),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::PAN_TILT.union($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS)))),
                    },
                    withheld: [],
                    evidence: [
                        ("zoom", "The Nearus command list (R21) `CAM_Zoom` Direct `8x 01 04 47 0p 0q 0r 0s FF` and its zoom position table (x12 lens): optical `0000` (x1) through `4000` (x12), digital `4000` (x1) through `7F00` (x4)."),
                        ("digital_zoom", "The Nearus command list (R21) `CAM_DZoom` on/off `8x 01 04 06 02/03 FF` and `CAM_DZoomModeInq` `8x 09 04 06 FF` (`y0 50 02/03 FF`); combined optical-plus-digital positions `4000`..`7F00` are reached with `CAM_Zoom` Direct `04 47` in Combine mode (`04 36 00`) with the x4 E-Zoom limit (`7E 01 19 01`); with the x2 limit the camera stops at `6A00`."),
                        ("one_push_focus", "The Nearus command list (R21) `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF` (One Push AF Trigger)."),
                        ("pan_tilt", "The Nearus command list (R21) `Pan-tiltDrive` speeds `01`..`18h` on both axes; `AbsolutePosition`/`RelativePosition` `8x 01 06 02/03 VV 00 0Y 0Y 0Y 0Y 0Y 0Z 0Z 0Z 0Z FF` with one speed byte and a five-nibble pan; `Pan-tiltLimitSet` limits pan `0x08A58` (left) / `0xF75A8` (right) and tilt `0x493D` (up) / `0xE796` (down); one degree is about `0xD0`."),
                        ("transport", "The Nearus command list (R21) documents VISCA over RS-232C and RS-422; it documents no IP transport, so the profile is serial-only."),
                        ("shutter", "The Nearus command list (R21) shutter table, BRC-300 column: `02` 1/4 s through `15` 1/10000 s."),
                        ("presets", "The Nearus command list (R21) `CAM_Memory` memory number p = 0 to 5 and `Cmd_PT_M_Speed` preset speed 1 to 24."),
                        ("image_flip", "The Nearus command list (R21) `CAM_ImgFlip` on/off `8x 01 04 66 02/03 FF` and `CAM_ImgFlipInq` `8x 09 04 66 FF`. It lists no `04 61` mirror command, so mirror stays unavailable."),
                        ("gain", "The Nearus command list (R21) gain table (exposure control 1/2): positions `0` (-3 dB) through `7` (18 dB)."),
                        ("exposure_compensation", "The Nearus command list (R21) `CAM_ExpComp` on/off `04 3E 02/03`, reset/up/down `04 0E`, direct `04 4E 00 00 0p 0q`, inquiries `09 04 3E`/`09 04 4E`; table `00` (-7) through `0E` (+7)."),
                        ("brightness", "The Nearus command list (R21) `CAM_Bright` reset/up/down `04 0D` and direct `04 4D 00 00 0p 0q`, inquiry `09 04 4D`. Its Bright table lists `00`..`17`."),
                        ("rgb_gain", "The Nearus command list (R21) `CAM_RGain`/`CAM_BGain` reset/up/down `04 03`/`04 04` and direct `04 43`/`04 44 00 00 0p 0q` with `00`..`FF`; inquiries `09 04 43`/`09 04 44`."),
                        ("sony_vendor_exposure", "The Nearus command list (R21, the BRC-300/300P command list text) documents the fixed `04 5A` auto slow-shutter commands and `09 04 5A` inquiry, but not the fixed `04 3A` spotlight commands."),
                        ("exposure_mode", "The Nearus command list (R21, the BRC-300/300P command list text) documents `CAM_AE` Full Auto/Manual/Shutter Priority/Iris Priority/Bright `8x 01 04 39 00/03/0A/0B/0D FF` and `CAM_AEModeInq` `8x 09 04 39 FF`."),
                        ("iris", "The Nearus command list (R21, the BRC-300/300P command list text) documents `CAM_Iris` reset/up/down `8x 01 04 0B 00/02/03 FF`, direct `8x 01 04 4B 00 00 0p 0q FF`, `CAM_IrisPosInq` `8x 09 04 4B FF`, and the iris table `00` CLOSE through `11` F1.6. The distinct `09 04 2B` status inquiry is not listed."),
                        ("one_push_white_balance", "The Nearus command list (R21, the BRC-300/300P command list text) `CAM_WB` lists One Push WB `8x 01 04 35 03 FF` and the One Push trigger `8x 01 04 10 05 FF`."),
                        ("image_controls", "The Nearus command list (R21, the BRC-300/300P command list text) lists no `04 49` saturation (color gain) command and no `04 28` focus near-limit command or inquiry, so neither typed surface is available."),
                        ("version_inquiry", "The Nearus \"VISCA Protocol via Sony\" document (R21) documents `CAM_VersionInq` `8X 09 00 02 FF` -> `Y0 50 GG GG HH HH JJ JJ KK FF`: vendor ID (0001 Sony), model ID (040F BRC-300/P, 0410 BRU-300/P), ROM revision and maximum socket (02); its command list repeats the row as `y0 50 00 01 mn pq rs tu vw FF` (model code 04xx, socket 02)."),
                        ],
                }

                profile GenericVisca {
                    doc: "Generic VISCA camera profile.",
                    id: "generic_visca",
                    id_doc: "Generic VISCA-compatible camera",
                    vendor: "Generic",
                    description: "Conservative profile for unknown VISCA-compatible cameras",
                    facts: {
                        capabilities: {
                            profile_id: from_id,
                            model_name: "Generic VISCA Camera",
                            default_camera_id: 1,
                            pan_tilt: {
                                geometry: None,
                                pan_speed: range!(u8, 1, 24),
                                tilt_speed: range!(u8, 1, 0x17),
                                simultaneous: true,
                                preset_recovery_time: std::time::Duration::from_millis(0),
                                variable_speed: false,
                                motion_sync_speed_range: None,
                            },
                            zoom: {
                                optical_max: 0x4000,
                                digital_max: None,
                                speed: range!(u8, 0, 7),
                                direct: false,
                                variable: true,
                                optical_ratio: None,
                            },
                            focus: {
                                range: range!(u16, 0x1000, 0xC000),
                                near_end: $crate::capabilities::FocusNearEnd::Maximum,
                                auto_focus: true,
                                one_push: true,
                                zone_inquiry: false,
                                zones: (profile_constants::NO_FOCUS_ZONES).to_vec(),
                                speed: range!(u8, 0, 7),
                                af_sensitivity: false,
                                near_limit_inquiry: false,
                            },
                            exposure: {
                                modes: (profile_constants::STANDARD_EXPOSURE_MODES).to_vec(),
                                iris_range: Some(domain!(u16, 0x00, 0x11, gaps: [0x01, 0x02, 0x03, 0x04])),
                                shutter_speeds: (profile_constants::SONY_BRC300_SHUTTER_SPEEDS).to_vec(),
                                gain_range: Some(range!(u8, 0, 7)),
                                brightness_range: None,
                                backlight_compensation: false,
                                compensation_range: None,
                                wide_dynamic_range: false,
                            },
                            white_balance: {
                                modes: (profile_constants::STANDARD_WB_MODES).to_vec(),
                                one_push: true,
                                color_temp_range: None,
                                color_temperature_steps: std::sync::Arc::from([]),
                                rgb_gain: None,
                                rgb_tuning: None,
                            },
                            image: None,
                            presets: {
                                highest: 5,
                                recall_axes: $crate::AffectedAxes::PAN_TILT
                                    .union($crate::AffectedAxes::ZOOM)
                                    .union($crate::AffectedAxes::FOCUS),
                                recall_speed_range: None,
                                tour: false,
                                thumbnail: false,
                                recall_delay: std::time::Duration::from_millis(0),
                                names: std::num::NonZeroUsize::new(0),
                            },
                            power: {
                                status_query: $crate::capabilities::PowerStatusQuery::Standard,
                                on_time: std::time::Duration::from_secs(30),
                                standby: None,
                                wake_on_lan: false,
                                retains_settings: true,
                                home_on_up: false,
                            },
                            menu: {
                            },
                            inquiry_support: $crate::capabilities::InquirySupport::Partial,
                            typed_support: grants([
                                (DirectShutter, $crate::source_evidence::source!([(R8, "r8-genericvisca-shutterdirect-grant"), (R12, "r12-genericvisca-shutterdirect-grant"), (R21, "r21-genericvisca-shutterdirect-grant")])),
                                (FocusInfinity, $crate::source_evidence::source!([(R8, "r8-genericvisca-focusinfinity-grant"), (R12, "r12-genericvisca-focusinfinity-grant"), (R21, "r21-genericvisca-focusinfinity-grant")])),
                                (PresetMemory, $crate::source_evidence::source!([(R8, "r8-genericvisca-presetmemory-grant"), (R12, "r12-genericvisca-presetmemory-grant"), (R21, "r21-genericvisca-presetmemory-grant")])),
                                (Shutter, $crate::source_evidence::source!([(R8, "r8-genericvisca-shuttercontrol-grant"), (R12, "r12-genericvisca-shuttercontrol-grant"), (R21, "r21-genericvisca-shuttercontrol-grant")])),
                                (Gain, $crate::source_evidence::source!([(R8, "r8-genericvisca-gaincontrol-grant"), (R12, "r12-genericvisca-gaincontrol-grant"), (R21, "r21-genericvisca-gaincontrol-grant")])),
                                (VersionInquiry, $crate::source_evidence::source!([(R8, "r8-genericvisca-versioninquiry-grant"), (R12, "r12-genericvisca-versioninquiry-grant"), (R21, "r21-genericvisca-versioninquiry-grant")])),
                                (ExposureMode, $crate::source_evidence::source!([(R8, "r8-genericvisca-exposuremode-grant"), (R12, "r12-genericvisca-exposuremode-grant"), (R21, "r21-genericvisca-exposuremode-grant")])),
                                (Iris, $crate::source_evidence::source!([(R8, "r8-genericvisca-iriscontrol-grant"), (R12, "r12-genericvisca-iriscontrol-grant"), (R21, "r21-genericvisca-iriscontrol-grant")])),
                                (OnePushFocus, $crate::source_evidence::source!([(R8, "r8-genericvisca-onepushfocus-grant"), (R12, "r12-genericvisca-onepushfocus-grant"), (R21, "r21-genericvisca-onepushfocus-grant")])),
                                (OnePushWhiteBalance(Standard), $crate::source_evidence::source!([(R8, "r8-genericvisca-onepushwhitebalance-grant"), (R12, "r12-genericvisca-onepushwhitebalance-grant"), (R21, "r21-genericvisca-onepushwhitebalance-grant")])),
                                ]),
                        },
                        encodings: from_grants,
                        transports: {
                            tcp_port: Some(5678),
                            udp_port: Some(1259),
                            serial: true,
                        },
                        envelope: $crate::profiles::ProfileEnvelope::RawVisca,
                        timing: {
                            ack_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_ACK_TIMEOUT_MS),
                            inquiry_timeout: std::time::Duration::from_millis(profile_registry::INTERIM_INQUIRY_TIMEOUT_MS),
                            cancellation_timeout: std::time::Duration::from_millis(1000),
                            ambiguity_timeout: std::time::Duration::from_millis(1000),
                            busy_timeout: std::time::Duration::from_millis(0),
                            command_timeouts: profile_registry::command_timeouts_ms(10000, 30000, 60000, 300000, 10000),
                            raw_inquiry_reply_skew: std::time::Duration::from_millis(0),
                            inquiry_spacing: std::time::Duration::from_millis(0),
                            command_spacing: std::time::Duration::from_millis(0),
                        },
                        maximum_command_sockets: crate::types::CommandSocketCount::TWO,
                        supports_operation_complete: false,
                        supports_command_cancel: true,
                        position_inquiries: Some($crate::AffectedAxes::ZOOM.union($crate::AffectedAxes::FOCUS.union($crate::AffectedAxes::IRIS))),
                    },
                    withheld: [],
                    evidence: [
                        ("pan_tilt", "R8 and R12/R21 disagree on pan and tilt ranges, degree scale, position-command grammar and position-reply width. Generic VISCA retains continuous drive and stop without granting degree positioning or position polling."),
                        ("focus", "R8 documents focus positions `1000`..`F000`; R12 and R21 document `1000`..`C000`. Generic VISCA uses the common `1000`..`C000` interval."),
                        ("shutter", "Generic VISCA uses the shutter codes R8, R12 and R21 all list with the same exposure time: `02` 1/4 s through `15` 1/10000 s (R8 also lists `00` 1/1 s and `01` 1/2 s)."),
                        ("presets", "R8, R12 and R21 all document `CAM_Memory` memory numbers 0 to 5; R8 documents no preset recall speed, so Generic VISCA has none."),
                        ("exposure_mode", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): the `04 39` Full Auto/Manual/Shutter Priority/Iris Priority/Bright commands and the `09 04 39` inquiry."),
                        ("iris", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): `04 0B` reset/up/down, direct `04 4B`, the `09 04 4B` position inquiry, and the iris positions all three list: `00` CLOSE and `05`..`11` (R12 and R21 also list `01`..`04`, R8 does not). None documents the distinct `09 04 2B` status inquiry."),
                        ("one_push_white_balance", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): the `04 35 03` One Push WB mode and the `04 10 05` trigger."),
                        ("focus_near_limit", "R12 and R21 list no `04 28` focus near-limit command or `09 04 28` inquiry, so the Sony-standard family does not include it."),
                        ("version_inquiry", "The generic profile models the Sony VISCA baseline, whose `CAM_VersionInq` reply is the 7-byte vendor/model/ROM/socket layout documented in R12; cameras that reply in another layout fail decode with an invalid-length error rather than a mislabelled value."),
                        ("one_push_focus", "Generic VISCA grants only the Sony-standard families that every Sony model source in the register documents identically (EVI-H100 R8, BRC-300 R12, and the Nearus BRC-300 list R21): `CAM_Focus` One Push Trigger `8x 01 04 18 01 FF`."),
                        ("direct_zoom", "R8, R12 and R21 all place the optical zoom endpoint at `4000`; Generic VISCA uses that shared readback range while keeping absolute zoom positioning unavailable."),
                        ("digital_zoom", "R8, R12 and R21 all document `CAM_DZoom` `04 06`, but their digital position tables differ (R8 `4000`..`7AC0`, R12/R21 `4000`..`7F00`) and Generic VISCA has no direct zoom, so it keeps digital zoom unavailable."),
                        ],
                }
            }
        }
    };
}
pub(crate) use builtin_profile_registry;

/// Re-exports every built-in profile type from `camera::profiles`.
macro_rules! reexport_builtin_profiles {
    (
        groups { $($groups:tt)* }
        profiles { $( profile $profile:ident { $($row:tt)* } )* }
    ) => {
        pub use $crate::camera::profiles::{ $($profile),* };
    };
}
pub(crate) use reexport_builtin_profiles;
