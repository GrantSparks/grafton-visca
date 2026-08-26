use grafton_visca::AffectedAxes;

#[test]
fn affected_axes_rejects_empty_and_unknown_bit_sets() {
    assert!(AffectedAxes::new(false, false, false).is_err());
    assert!(AffectedAxes::from_bits(0).is_err());
    assert!(AffectedAxes::from_bits(0b10_0000).is_err());
    assert!(AffectedAxes::from_bits(AffectedAxes::PAN_TILT.bits() | 0b10_0000).is_err());
}

#[test]
fn affected_axes_preserves_explicit_nonempty_combinations() {
    let axes = AffectedAxes::new(true, true, false).expect("valid axes");
    assert!(axes.contains(AffectedAxes::PAN_TILT));
    assert!(axes.contains(AffectedAxes::ZOOM));
    assert!(!axes.contains(AffectedAxes::FOCUS));
}

#[test]
fn affected_axes_supports_iris_nd_and_canonical_iteration() {
    let axes =
        AffectedAxes::new_with_iris_nd(false, false, false, true, true).expect("scalar axes");
    assert!(axes.contains(AffectedAxes::IRIS));
    assert!(axes.contains(AffectedAxes::ND_FILTER));
    assert_eq!(axes.len(), 2);
    assert_eq!(
        axes.iter().collect::<Vec<_>>(),
        vec![
            grafton_visca::AffectedAxis::Iris,
            grafton_visca::AffectedAxis::NdFilter,
        ]
    );
}

#[cfg(feature = "serde")]
#[test]
fn affected_axes_deserialization_revalidates_bits() {
    assert!(serde_json::from_str::<AffectedAxes>("0").is_err());
    assert!(serde_json::from_str::<AffectedAxes>("32").is_err());
    assert_eq!(
        serde_json::from_str::<AffectedAxes>("8").expect("iris axis"),
        AffectedAxes::IRIS
    );
    assert_eq!(
        serde_json::from_str::<AffectedAxes>("3").expect("known nonempty axes"),
        AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM)
    );
}
