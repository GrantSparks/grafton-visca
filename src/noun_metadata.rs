//! Test-only projections of the authoritative noun and motion tables.
//!
//! Convention checks read these rows instead of keeping a second API list.

#![cfg(test)]

use crate::command::surface::RowGate;
use crate::noun_table::noun_table;

/// Public completion shape of one generated method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NounKind {
    /// Decodes a protocol inquiry.
    Inquiry,
    /// Waits for a plain command's protocol completion.
    Plain,
    /// Returns an applied-only operation handle.
    Applied,
    /// Returns an operation handle with profile-selected settlement.
    Targeted,
    /// Calls an owner motion-safety or observation method directly.
    Motion,
}

/// One argument as declared in the shared table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NounArgument {
    /// Parameter spelling.
    pub(crate) name: &'static str,
    /// Concrete value type shared by the static and dynamic facades.
    pub(crate) ty: &'static str,
}

/// A method's single-source API, documentation and capability contract.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NounRow {
    /// Noun accessor spelling, such as `pan_tilt`.
    pub(crate) noun: &'static str,
    /// Noun header identifier, such as `PanTilt`.
    pub(crate) noun_name: &'static str,
    /// Completion shape.
    pub(crate) kind: NounKind,
    /// Method spelling.
    pub(crate) method: &'static str,
    /// Ordered concrete arguments; static speed inputs additionally accept `Into`.
    pub(crate) arguments: &'static [NounArgument],
    /// Protocol request type, or an empty string for a direct owner motion method.
    pub(crate) request_type: &'static str,
    /// The successful payload inside the facade's `Result`.
    pub(crate) return_type: &'static str,
    /// Every static marker, including the noun header and refining row markers.
    pub(crate) gate_markers: &'static [&'static str],
    /// Runtime gate generated from the same header and complete refining list.
    pub(crate) gate: RowGate,
    /// Authored method documentation.
    pub(crate) docs: &'static str,
    /// Generated completion wording, shared with facade rustdoc.
    pub(crate) completion_doc: &'static str,
    /// Public request identifier carried by a protocol row, if present.
    pub(crate) command: Option<&'static str>,
}

macro_rules! noun_kind {
    (inquiry) => {
        NounKind::Inquiry
    };
    (plain) => {
        NounKind::Plain
    };
    (applied) => {
        NounKind::Applied
    };
    (targeted) => {
        NounKind::Targeted
    };
}

macro_rules! noun_return_type {
    (inquiry; $ret:ty) => {
        stringify!($ret)
    };
    (plain; $ret:ty) => {
        "()"
    };
    (applied; $ret:ty) => {
        "Operation<AppliedOnly>"
    };
    (targeted; $ret:ty) => {
        "Operation<Targeted>"
    };
}

// An inquiry declares its response after `->`; recover the request path from
// the unit value or `::new(...)` expression without maintaining an inquiry list.
macro_rules! inquiry_request_type {
    (with_target |$binding:ident| $($request:tt)*) => {
        inquiry_request_type!($($request)*)
    };
    (with_profile |$binding:ident| $($request:tt)*) => {
        inquiry_request_type!($($request)*)
    };
    (checked $($request:tt)*) => {
        inquiry_request_type!($($request)*)
    };
    (mapped [$($request:tt)*] |$reply:ident, $profile:ident| $mapping:expr) => {
        inquiry_request_type!($($request)*)
    };
    (@path [$($path:tt)*]) => { stringify!($($path)*) };
    (@path [$($path:tt)*] :: new $arguments:tt) => { stringify!($($path)*) };
    (@path [$($path:tt)*] :: for_target $arguments:tt) => { stringify!($($path)*) };
    (@path [$($path:tt)*] :: $segment:ident $($remaining:tt)*) => {
        inquiry_request_type!(@path [$($path)* :: $segment] $($remaining)*)
    };
    ($head:ident $($remaining:tt)*) => {
        inquiry_request_type!(@path [$head] $($remaining)*)
    };
}

macro_rules! noun_request_type {
    (inquiry; $ret:ty; [$($request:tt)*]) => {
        inquiry_request_type!($($request)*)
    };
    ($kind:ident; $ret:ty; [$($request:tt)*]) => { stringify!($ret) };
}

macro_rules! noun_gate_markers {
    ([always]; [$($marker:ident)*]) => { &[$(stringify!($marker)),*] };
    ([$kind:ident $base:ident]; [$($marker:ident)*]) => {
        &[stringify!($base), $(stringify!($marker)),*]
    };
}

macro_rules! noun_command {
    ([]) => {
        None
    };
    ([$command:ident]) => {
        Some(stringify!($command))
    };
}

macro_rules! project_noun_rows {
    (
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                gate: $base:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[doc = $row_doc:literal])*
                $kind:ident [$($command:ident)?] $method:ident($($(#[$coercion:ident])? $argument:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        const NOUN_ROWS: &[NounRow] = &[
            $($(
                NounRow {
                    noun: stringify!($getter),
                    noun_name: stringify!($noun),
                    kind: noun_kind!($kind),
                    method: stringify!($method),
                    arguments: &[$(NounArgument { name: stringify!($argument), ty: stringify!($ty) }),*],
                    request_type: noun_request_type!($kind; $ret; [$($request)*]),
                    return_type: noun_return_type!($kind; $ret),
                    gate_markers: noun_gate_markers!($base; [$($gate $($extra)*)?]),
                    gate: crate::command::surface::noun_row_gate!([$($gate $(, $extra)*)?], $base),
                    docs: concat!($($row_doc, "\n",)*),
                    completion_doc: crate::noun_table::completion_doc!($kind),
                    command: noun_command!([$($command)?]),
                },
            )*)*
        ];
    };
}

noun_table!(project_noun_rows);

macro_rules! project_motion_rows {
    (
        @motion {
            accessor: $accessor:ident,
            getter: $getter:ident,
            doc: $doc:literal $(,)?
        };
        $(
            $(#[doc = $row_doc:literal])*
            fn $name:ident(&self $(, $argument:ident: $ty:ty)*) -> $ret:ty
                => $core_method:ident($($call:expr),*);
        )*
    ) => {
        const MOTION_ROWS: &[NounRow] = &[$(
            NounRow {
                noun: stringify!($getter),
                noun_name: "Motion",
                kind: NounKind::Motion,
                method: stringify!($name),
                arguments: &[$(NounArgument { name: stringify!($argument), ty: stringify!($ty) }),*],
                request_type: "",
                return_type: stringify!($ret),
                gate_markers: &[],
                gate: RowGate::Always,
                docs: concat!($($row_doc, "\n",)*),
                completion_doc: "Returns the owner's motion-safety or observation result.",
                command: None,
            },
        )*];
    };
}

crate::noun_table::motion_table!(project_motion_rows);

static ROWS: [NounRow; NOUN_ROWS.len() + MOTION_ROWS.len()] = {
    let mut rows = [NOUN_ROWS[0]; NOUN_ROWS.len() + MOTION_ROWS.len()];
    let mut index = 0;
    while index < NOUN_ROWS.len() {
        rows[index] = NOUN_ROWS[index];
        index += 1;
    }
    let mut motion = 0;
    while motion < MOTION_ROWS.len() {
        rows[index + motion] = MOTION_ROWS[motion];
        motion += 1;
    }
    rows
};

/// Returns every facade method, including the motion view when a facade is enabled.
pub(crate) const fn rows() -> &'static [NounRow] {
    &ROWS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn projection_retains_unique_methods_and_complete_row_contracts() {
        let mut methods = HashSet::new();
        for row in rows() {
            assert!(methods.insert((row.noun, row.method)), "duplicate {row:?}");
            assert!(!row.noun_name.is_empty());
            assert!(
                !row.docs.trim().is_empty(),
                "missing authored docs: {row:?}"
            );
            assert!(!row.completion_doc.is_empty());
            assert!(!row.return_type.is_empty());
            assert!(row
                .arguments
                .iter()
                .all(|argument| !argument.name.is_empty() && !argument.ty.is_empty()));
            if row.kind != NounKind::Motion {
                assert!(!row.request_type.is_empty());
                assert!(
                    !row.request_type.contains('('),
                    "constructor leaked into request type: {row:?}"
                );
                let path = row.request_type.replace(' ', "");
                assert_eq!(row.command, path.rsplit("::").next());
            }
            if matches!(row.gate, RowGate::Always) {
                assert!(row.gate_markers.is_empty());
            }
        }
    }

    #[test]
    fn inquiry_types_are_paths_for_units_and_argument_bearing_constructors() {
        assert_eq!(
            inquiry_request_type!(command::PowerInquiry),
            "command :: PowerInquiry"
        );
        assert_eq!(
            inquiry_request_type!(command::presets::SeparateModeInquiry::new(preset)),
            "command :: presets :: SeparateModeInquiry"
        );
        assert_eq!(
            inquiry_request_type!(
                mapped[command::pan_tilt::PositionInquiry] | reply,
                profile | profile.to_degrees(reply)
            ),
            "command :: pan_tilt :: PositionInquiry"
        );
    }

    #[test]
    fn target_bound_query_factories_project_the_request_type() {
        assert_eq!(
            inquiry_request_type!(
                with_target | target | command::zoom::NormalizedInquiry::for_target(target, domain)
            ),
            "command :: zoom :: NormalizedInquiry"
        );
    }

    #[test]
    fn marker_projection_keeps_the_header_and_every_refinement() {
        let markers: &[&str] =
            noun_gate_markers!([typed HasNdFilter]; [HasDirectNdFilter HasNdFilterDensityInquiry]);
        assert_eq!(
            markers,
            &[
                "HasNdFilter",
                "HasDirectNdFilter",
                "HasNdFilterDensityInquiry"
            ]
        );
    }
}
