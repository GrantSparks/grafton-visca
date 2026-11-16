//! Checked values share construction, ordering, hashing, and wire contracts.

use std::collections::{BTreeSet, HashSet};

use grafton_visca::{
    types::{BrightnessLevel, FocusPosition, GainLevel, ShutterSpeed},
    units::{Degrees, UnitInterval},
    wire::WireValue,
    CameraId, IndividualCameraId,
};

#[test]
fn finite_values_have_consistent_signed_zero_equality_order_and_hashing() {
    let degree_zero = Degrees::new(0.0).unwrap();
    let degree_negative_zero = Degrees::new(-0.0).unwrap();
    assert_eq!(degree_zero, degree_negative_zero);
    assert_eq!(
        degree_zero.cmp(&degree_negative_zero),
        std::cmp::Ordering::Equal
    );
    assert_eq!(HashSet::from([degree_zero, degree_negative_zero]).len(), 1);
    assert_eq!(BTreeSet::from([degree_zero, degree_negative_zero]).len(), 1);

    let unit_zero = UnitInterval::new(0.0).unwrap();
    let unit_negative_zero = UnitInterval::new(-0.0).unwrap();
    assert_eq!(unit_zero, unit_negative_zero);
    assert_eq!(HashSet::from([unit_zero, unit_negative_zero]).len(), 1);
    assert_eq!(BTreeSet::from([unit_zero, unit_negative_zero]).len(), 1);
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(Degrees::new(invalid).is_err());
        assert!(UnitInterval::new(invalid).is_err());
    }
}

#[test]
fn formerly_infallible_values_expose_checked_union_bounds() {
    let minimum: grafton_visca::Result<ShutterSpeed> = ShutterSpeed::new(0);
    let maximum: grafton_visca::Result<ShutterSpeed> = ShutterSpeed::new(255);
    assert_eq!(minimum.unwrap(), ShutterSpeed::MIN);
    assert_eq!(maximum.unwrap(), ShutterSpeed::MAX);
    assert_eq!(BrightnessLevel::new(0).unwrap(), BrightnessLevel::MIN);
    assert_eq!(BrightnessLevel::new(255).unwrap(), BrightnessLevel::MAX);
    assert_eq!(FocusPosition::new(0).unwrap(), FocusPosition::MIN);
    assert_eq!(FocusPosition::new(u16::MAX).unwrap(), FocusPosition::MAX);
}

#[test]
fn checked_identity_wire_values_revalidate_their_domains() {
    assert_eq!(GainLevel::from_wire(0).unwrap(), GainLevel::MIN);
    assert_eq!(GainLevel::from_wire(25).unwrap(), GainLevel::MAX);
    assert_eq!(GainLevel::MAX.to_wire(), 25);
    assert!(GainLevel::from_wire(26).is_err());
}

#[test]
fn camera_id_constructors_share_result_and_scalar_getters() {
    let camera: grafton_visca::Result<CameraId> = CameraId::new(1);
    let individual: grafton_visca::Result<IndividualCameraId> = IndividualCameraId::new(1);
    assert_eq!(camera.unwrap().value(), individual.unwrap().value());
    assert!(CameraId::new(0).is_err());
    assert!(IndividualCameraId::new(0).is_err());
    assert!(CameraId::new(8).is_ok());
    assert!(IndividualCameraId::new(8).is_err());
}
