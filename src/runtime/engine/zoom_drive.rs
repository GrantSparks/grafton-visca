//! Wire evidence used to select the matching STOP for a continuous zoom drive.

use super::{RequestId, TransmissionId};
use crate::command::semantics::BuiltinCommand;

#[derive(Debug, Default)]
pub(super) struct ZoomDriveHistory {
    // An unacknowledged ordinary command uses one of the target's two command
    // slots. Its evidence retires on ACK, backoff or terminal completion.
    active: [Option<(RequestId, ZoomDriveWrite)>; crate::CommandSocketCount::MAX.value() as usize],
    retired: Option<ZoomDriveWrite>,
}

impl ZoomDriveHistory {
    pub(super) fn record(&mut self, id: RequestId, write: ZoomDriveWrite) {
        let slot = self.active.iter_mut().find(|slot| slot.is_none());
        debug_assert!(
            slot.is_some(),
            "a zoom drive uses an available command socket"
        );
        if let Some(slot) = slot {
            *slot = Some((id, write));
        }
    }

    pub(super) fn retire(&mut self, id: RequestId, possibly_applied: bool) {
        let retired = self
            .active
            .iter_mut()
            .find(|slot| slot.is_some_and(|(request, _)| request == id))
            .and_then(Option::take);
        if let Some((_, write)) = retired.filter(|(_, write)| {
            possibly_applied
                && self
                    .retired
                    .is_none_or(|last| last.transmission < write.transmission)
        }) {
            self.retired = Some(write);
        }
    }

    pub(super) fn use_high_resolution_stop(&self) -> bool {
        self.active
            .iter()
            .flatten()
            .map(|&(_, write)| write)
            .chain(self.retired)
            .max_by_key(|write| write.transmission)
            .is_some_and(|write| write.family == ZoomDriveFamily::HighResolution)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ZoomDriveWrite {
    transmission: TransmissionId,
    family: ZoomDriveFamily,
}

impl ZoomDriveWrite {
    pub(super) const fn new(transmission: TransmissionId, family: ZoomDriveFamily) -> Self {
        Self {
            transmission,
            family,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZoomDriveFamily {
    Standard,
    HighResolution,
}

impl ZoomDriveFamily {
    /// Recognizes complete addressed drives using the reviewed packet patterns
    /// and operand bounds for each family.
    pub(crate) fn classify(bytes: &[u8], target_address: u8) -> Option<Self> {
        let (&address, body) = bytes.split_first()?;
        if address != target_address {
            return None;
        }
        let (commands, family): (&[BuiltinCommand], _) = match body {
            [_, _, _, speed, 0xff] if speed & 0xf <= 7 => (
                &[
                    BuiltinCommand::ZoomTele,
                    BuiltinCommand::ZoomWide,
                    BuiltinCommand::ZoomTeleVariable,
                    BuiltinCommand::ZoomWideVariable,
                ],
                Self::Standard,
            ),
            [_, _, _, _, _, high @ 0..=7, mid_high @ 0..=15, mid_low @ 0..=15, low @ 0..=15, 0xff]
                if [*high, *mid_high, *mid_low, *low] != [7, 15, 15, 15] =>
            {
                (
                    &[
                        BuiltinCommand::Fr7ZoomHighResolutionTele,
                        BuiltinCommand::Fr7ZoomHighResolutionWide,
                    ],
                    Self::HighResolution,
                )
            }
            _ => return None,
        };
        commands
            .iter()
            .any(|row| {
                row.packet()
                    .is_some_and(|packet| packet.matches_bytes(body))
            })
            .then_some(family)
    }
}
