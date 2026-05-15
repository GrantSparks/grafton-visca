//! Tally capability trait for camera tally light control.

/// Trait for cameras that support tally light control.
///
/// Tally lights are LED indicators on professional cameras that show when the camera
/// is "on air" (red) or in preview (green). This is commonly found on Sony professional
/// cameras but not typically available on PtzOptics or consumer cameras.
pub trait Tally {
    /// Whether this camera supports tally light control.
    /// Defaults to false for cameras without tally support.
    const SUPPORTS_TALLY: bool = false;
}

/// Extension trait that adds tally-related helper methods.
pub trait TallyExt: Tally {
    /// Check if tally is supported by the profile metadata.
    fn has_tally(&self) -> bool {
        Self::SUPPORTS_TALLY
    }
}

// Automatic implementation for all types that support tally
impl<T: Tally> TallyExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    struct SonyCamera;
    impl Tally for SonyCamera {
        const SUPPORTS_TALLY: bool = true;
    }

    #[test]
    fn test_tally_support() {
        let camera = SonyCamera;
        assert!(camera.has_tally());
    }
}
