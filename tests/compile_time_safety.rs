//! Tests to verify compile-time safety of the generic camera API.

#[cfg(feature = "async")]
use grafton_visca::camera::AsyncCamera;

#[cfg(not(feature = "async"))]
use grafton_visca::BlockingCamera;

use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    capabilities::{PanTilt, Profile, ProfileMetadata, Zoom},
};

#[test]
fn test_profile_super_trait() {
    // These types should compile because they implement Profile
    fn accepts_profile<P: Profile>() {}

    accepts_profile::<PtzOpticsG2>();
    accepts_profile::<SonyFR7>();
    accepts_profile::<GenericVisca>();
}

#[test]
fn test_profile_constants() {
    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    assert_eq!(GenericVisca::MODEL_NAME, "Generic VISCA Camera");

    assert_eq!(PtzOpticsG2::ZOOM_SPEED_RANGE, 0..8);
    assert_eq!(SonyFR7::ZOOM_SPEED_RANGE, 0..8);

    assert_eq!(PtzOpticsG2::MAX_PAN_SPEED, 24);
    assert_eq!(SonyFR7::MAX_PAN_SPEED, 24);
}

#[test]
fn test_generic_camera_methods() {
    #[cfg(not(feature = "async"))]
    type _G2Camera<T> = BlockingCamera<PtzOpticsG2, T>;

    #[cfg(feature = "async")]
    type _G2Camera<T, E> = AsyncCamera<PtzOpticsG2, T, E>;

    #[cfg(not(feature = "async"))]
    type _FR7Camera<T> = BlockingCamera<SonyFR7, T>;

    #[cfg(feature = "async")]
    type _FR7Camera<T, E> = AsyncCamera<SonyFR7, T, E>;

    // Test that the camera types exist and have proper generics
}
