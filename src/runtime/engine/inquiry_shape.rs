//! Bounded exact inquiry declarations retained by a raw target.
//!
//! Overlap is a scheduling predicate, never an equivalence relation. In
//! particular, retaining `Unknown` must not merge two disjoint declarations.

use crate::PayloadShape;

/// Maximum distinct raw inquiry declarations simultaneously admitted, owed,
/// or quarantined on one target. No new declaration is admitted at capacity;
/// existing declarations and the command lane keep their existing behavior.
pub(crate) const MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET: usize = 16;

/// An inline exact set. It is derived from authoritative entries/debts/holds
/// when reserving admission, and returned by a ledger decision when evidence
/// retires inquiry debts. It is never a second persistent reservation index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InquiryShapes {
    declarations: [PayloadShape; MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET],
    len: u8,
}

impl Default for InquiryShapes {
    fn default() -> Self {
        Self {
            declarations: [PayloadShape::Unknown; MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET],
            len: 0,
        }
    }
}

impl InquiryShapes {
    pub(super) fn iter(&self) -> impl Iterator<Item = PayloadShape> + '_ {
        self.declarations[..self.len()].iter().copied()
    }

    pub(super) const fn len(&self) -> usize {
        self.len as usize
    }

    #[cfg(test)]
    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) fn contains(&self, shape: PayloadShape) -> bool {
        self.iter().any(|declaration| declaration == shape)
    }

    /// Returns false only for a seventeenth distinct declaration. Callers
    /// retaining protocol evidence must treat that as an invariant violation,
    /// rather than forget an unpaid answer.
    pub(super) fn try_insert(&mut self, shape: PayloadShape) -> bool {
        if self.contains(shape) {
            return true;
        }
        if self.len() == MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET {
            return false;
        }
        self.declarations[self.len()] = shape;
        self.len += 1;
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::num::NonZeroUsize;

    use super::*;

    #[test]
    fn exact_reservations_do_not_merge_overlap_or_transitive_unknown() {
        let mut shapes = InquiryShapes::default();
        for shape in [
            PayloadShape::Bytes(NonZeroUsize::new(4).unwrap()),
            PayloadShape::Nibbles(NonZeroUsize::new(4).unwrap()),
            PayloadShape::Unknown,
        ] {
            assert!(shapes.try_insert(shape));
            assert!(shapes.try_insert(shape));
        }
        assert_eq!(shapes.len(), 3);
        for length in 5..18 {
            assert!(shapes.try_insert(PayloadShape::Bytes(NonZeroUsize::new(length).unwrap())));
        }
        assert_eq!(shapes.len(), MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET);
        assert!(!shapes.try_insert(PayloadShape::Bytes(NonZeroUsize::new(18).unwrap())));
        assert!(shapes.try_insert(PayloadShape::Unknown));
        assert_eq!(shapes.len(), MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET);
    }
}
