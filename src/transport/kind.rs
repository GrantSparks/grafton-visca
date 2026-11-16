//! Closed transport kinds used by profile checks and connection plans.

/// Network transports whose endpoint can have a profile-selected default port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkTransport {
    Tcp,
    Udp,
}

impl NetworkTransport {
    pub(crate) const fn standard(self) -> StandardTransport {
        match self {
            Self::Tcp => StandardTransport::Tcp,
            Self::Udp => StandardTransport::Udp,
        }
    }
}

/// Standard transports that participate in the profile support matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandardTransport {
    Tcp,
    Udp,
    Serial,
}

impl StandardTransport {
    /// Projects the closed internal kind into the public configuration vocabulary.
    pub(crate) const fn configuration_kind(self) -> crate::camera::TransportKind {
        match self {
            Self::Tcp => crate::camera::TransportKind::Tcp,
            Self::Udp => crate::camera::TransportKind::Udp,
            Self::Serial => crate::camera::TransportKind::Serial,
        }
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl From<super::SendSemantics> for crate::runtime::engine::TransportKind {
    fn from(semantics: super::SendSemantics) -> Self {
        match semantics {
            super::SendSemantics::Datagram => Self::Datagram,
            super::SendSemantics::Stream => Self::Stream,
        }
    }
}
