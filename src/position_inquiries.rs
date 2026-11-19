//! The inquiry selected to poll each physical axis during settlement.
//!
//! This declaration generates the validation gates below, the typed
//! position-query plan in `prepared`, and the owner sampling loop, so a gate
//! or prepared axis cannot drift from the inquiry actually sampled.

use crate::{
    capabilities::Capabilities, command::surface::RowGate, requests::AffectedAxis, AffectedAxes,
};

macro_rules! settlement_inquiries {
    ($consumer:path $(, $($context:tt)*)?) => {
        $consumer! {
            $($($context)*)?
            PanTilt, pan_tilt => PanTiltPositionInquiry, decoder: crate::ResponseDecoder::for_pan_tilt_sample;
            Zoom, zoom => ZoomPositionInquiry, decoder: crate::ResponseDecoder::for_zoom_sample;
            Focus, focus => FocusPositionInquiry, decoder: crate::ResponseDecoder::for_focus_sample;
            Iris, iris => IrisInquiry, decoder: crate::ResponseDecoder::for_iris_sample;
            NdFilter, nd_filter => NdFilterInquiry;
        }
    };
}
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) use settlement_inquiries;

macro_rules! inquiry_gates {
    ($($axis:ident, $field:ident => $inquiry:ident $(, decoder: $decoder:path)?;)*) => {
        // Keep the declaration exhaustive when the closed axis vocabulary grows.
        const fn inquiry_gate(axis: AffectedAxis) -> RowGate {
            match axis {
                $(AffectedAxis::$axis =>
                    <crate::command::inquiry::$inquiry as crate::command::inquiry_structs::GeneratedInquiry>::GATE,)*
            }
        }

        /// Each axis and the gate of the exact inquiry used to poll it.
        pub(crate) const SETTLEMENT_INQUIRIES: &[(AffectedAxis, RowGate)] = &[
            $((AffectedAxis::$axis, inquiry_gate(AffectedAxis::$axis)),)*
        ];
    };
}
settlement_inquiries!(inquiry_gates);

/// Whether every selected axis's settlement inquiry admits these capabilities.
pub(crate) fn permits(capabilities: &Capabilities, axes: AffectedAxes) -> bool {
    axes.iter().all(|axis| {
        SETTLEMENT_INQUIRIES
            .iter()
            .find(|(candidate, _)| *candidate == axis)
            .is_some_and(|(_, gate)| gate.permits(capabilities))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_inquiries_cover_every_axis_once() {
        let axes = AffectedAxes::PAN_TILT
            | AffectedAxes::ZOOM
            | AffectedAxes::FOCUS
            | AffectedAxes::IRIS
            | AffectedAxes::ND_FILTER;
        assert_eq!(SETTLEMENT_INQUIRIES.len(), axes.iter().len());
        for axis in axes.iter() {
            assert_eq!(
                SETTLEMENT_INQUIRIES
                    .iter()
                    .filter(|(candidate, _)| *candidate == axis)
                    .count(),
                1,
                "{axis:?} must select exactly one settlement inquiry"
            );
        }
    }
}
