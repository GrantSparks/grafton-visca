//! Individual-camera keys and fixed, seven-slot target storage.

use std::ops::{Index, IndexMut};

use crate::{CameraId, Error};

/// A validated individual camera, never the controller or broadcast address.
///
/// The representation is a zero-based index, private to this module. Public
/// camera addresses cross into owner state through `TryFrom<CameraId>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TargetSlot(u8);

impl TargetSlot {
    /// The protocol's fixed number of individual camera addresses.
    pub(crate) const COUNT: usize = 7;

    /// Every individual target in camera-number order.
    pub(crate) const ALL: [Self; Self::COUNT] = [
        Self(0),
        Self(1),
        Self(2),
        Self(3),
        Self(4),
        Self(5),
        Self(6),
    ];

    #[cfg(test)]
    pub(crate) const CAMERA_1: Self = Self::ALL[0];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_2: Self = Self::ALL[1];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_3: Self = Self::ALL[2];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_4: Self = Self::ALL[3];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_5: Self = Self::ALL[4];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_6: Self = Self::ALL[5];

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const CAMERA_7: Self = Self::ALL[6];

    /// The corresponding wire camera address.
    pub(crate) const fn camera(self) -> CameraId {
        const CAMERAS: [CameraId; TargetSlot::COUNT] = [
            CameraId::CAMERA_1,
            CameraId::CAMERA_2,
            CameraId::CAMERA_3,
            CameraId::CAMERA_4,
            CameraId::CAMERA_5,
            CameraId::CAMERA_6,
            CameraId::CAMERA_7,
        ];
        CAMERAS[self.index()]
    }

    /// The bounded array index for this key, in `0..7`.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl From<crate::IndividualCameraId> for TargetSlot {
    fn from(camera: crate::IndividualCameraId) -> Self {
        Self(camera.value() - 1)
    }
}

impl TryFrom<CameraId> for TargetSlot {
    type Error = Error;

    fn try_from(camera: CameraId) -> Result<Self, Self::Error> {
        match camera.id() {
            1..=7 => Ok(Self(camera.id() - 1)),
            _ => Err(Error::InvalidRequest(
                "a session target must be an individual camera".into(),
            )),
        }
    }
}

/// Fixed storage indexed only by a validated individual target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PerTarget<T>([T; TargetSlot::COUNT]);

impl<T> PerTarget<T> {
    /// Initializes each slot exactly once in camera-number order.
    pub(crate) fn from_fn(mut create: impl FnMut(TargetSlot) -> T) -> Self {
        Self(std::array::from_fn(|index| create(TargetSlot::ALL[index])))
    }

    /// Borrows each individual camera's value in camera-number order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (TargetSlot, &T)> {
        TargetSlot::ALL.into_iter().zip(self.0.iter())
    }

    /// Mutably borrows each individual camera's value in camera-number order.
    #[cfg(any(feature = "blocking", feature = "async", test))]
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (TargetSlot, &mut T)> {
        TargetSlot::ALL.into_iter().zip(self.0.iter_mut())
    }
}

impl<T: Default> Default for PerTarget<T> {
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T> Index<TargetSlot> for PerTarget<T> {
    type Output = T;

    fn index(&self, target: TargetSlot) -> &Self::Output {
        &self.0[target.index()]
    }
}

impl<T> IndexMut<TargetSlot> for PerTarget<T> {
    fn index_mut(&mut self, target: TargetSlot) -> &mut Self::Output {
        &mut self.0[target.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn individual_keys_round_trip_and_reject_other_addresses() {
        for (index, target) in TargetSlot::ALL.into_iter().enumerate() {
            assert_eq!(target.index(), index);
            assert_eq!(usize::from(target.camera().id()), index + 1);
            assert_eq!(TargetSlot::try_from(target.camera()).ok(), Some(target));
        }
        assert!(CameraId::new(0).is_err());
        assert!(matches!(
            TargetSlot::try_from(CameraId::BROADCAST),
            Err(Error::InvalidRequest(reason))
                if reason == "a session target must be an individual camera"
        ));
        assert_eq!(size_of::<TargetSlot>(), 1);
    }

    #[test]
    fn target_tables_index_and_iterate_only_individual_cameras() {
        let mut table = PerTarget::from_fn(|target| target.camera().id());
        for (target, value) in table.iter_mut() {
            assert_eq!(*value, target.camera().id());
            *value *= 2;
        }
        for (target, value) in table.iter() {
            assert_eq!(table[target], target.camera().id() * 2);
            assert_eq!(*value, table[target]);
        }
        let empty = PerTarget::<Option<u8>>::default();
        assert!(empty.iter().all(|(_, value)| value.is_none()));
        assert_eq!(table.iter().count(), TargetSlot::COUNT);
        assert_eq!(size_of::<PerTarget<u8>>(), 7);
    }
}
