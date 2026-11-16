//! Release admission ownership before exposing a request's terminal observer.

use std::sync::Arc;

use super::{AdmissionPermit, ObserverCell, RequestSummary, TerminalSlot};
use crate::Error;

#[derive(Debug)]
pub(super) struct ActiveRequest {
    permit: AdmissionPermit,
    observer: TerminalSlot,
    summary: RequestSummary,
    /// The one cancellation intent retained for this request.
    pub(super) cancellation: Option<Arc<ObserverCell<Error>>>,
}

pub(super) struct RetiredRequest {
    pub(super) observer: TerminalSlot,
    pub(super) summary: RequestSummary,
}

impl ActiveRequest {
    pub(super) fn new(
        permit: AdmissionPermit,
        observer: TerminalSlot,
        summary: RequestSummary,
    ) -> Self {
        Self {
            permit,
            observer,
            summary,
            cancellation: None,
        }
    }

    pub(super) const fn summary(&self) -> RequestSummary {
        self.summary
    }

    /// Returns the terminal observer only after all admission ownership ends.
    pub(super) fn retire(self) -> RetiredRequest {
        let Self {
            permit,
            observer,
            summary,
            cancellation,
        } = self;
        drop(permit);
        drop(cancellation);
        RetiredRequest { observer, summary }
    }
}
