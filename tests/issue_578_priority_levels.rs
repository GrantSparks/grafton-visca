//! Regression test for issue #578.
//!
//! `Priority::High` and `Priority::Critical` used to be compile-gated behind
//! `all(feature = "mode-async", feature = "test-utils")` or `cfg(test)`, so a
//! production build of this crate exposed only `Low` and `Normal` while the
//! type documented four levels including an "emergency/safety" lane.
//!
//! This file deliberately uses no feature gate: it is compiled and run by the
//! default (blocking, no `test-utils`) build, so it fails to compile if the
//! upper two levels are ever gated out of the production surface again.

use grafton_visca::runtime::Priority;

/// All four levels must be nameable from a plain downstream build.
#[test]
fn all_four_priority_levels_exist_in_production_builds() {
    let levels = [
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Critical,
    ];

    // Sorting by the derived `Ord` is the only thing the scheduler relies on.
    let mut sorted = levels;
    sorted.sort_unstable();
    assert_eq!(
        sorted, levels,
        "levels must sort Low < Normal < High < Critical"
    );
}

/// The scheduler selects work by `Ord` alone, so the ordering relation is the
/// entire observable contract of this type.
#[test]
fn critical_outranks_every_other_level() {
    assert!(Priority::Critical > Priority::High);
    assert!(Priority::High > Priority::Normal);
    assert!(Priority::Normal > Priority::Low);
    assert_eq!(
        [
            Priority::Low,
            Priority::Normal,
            Priority::High,
            Priority::Critical
        ]
        .iter()
        .copied()
        .max(),
        Some(Priority::Critical)
    );
}

/// The discriminants are documented as `Low = 0 … Critical = 3`; pin them so a
/// reordering that silently changes scheduling cannot slip through.
#[test]
fn priority_discriminants_are_stable() {
    assert_eq!(Priority::Low as u8, 0);
    assert_eq!(Priority::Normal as u8, 1);
    assert_eq!(Priority::High as u8, 2);
    assert_eq!(Priority::Critical as u8, 3);
}

/// `Priority` is `Copy` + `Debug` + `Eq`, which downstream schedulers rely on
/// when they thread a level through their own request types.
#[test]
fn priority_is_copy_debug_and_comparable() {
    fn assert_bounds<T: Copy + std::fmt::Debug + Eq + Ord + Send + Sync + 'static>() {}
    assert_bounds::<Priority>();

    let critical = Priority::Critical;
    let copied = critical;
    assert_eq!(critical, copied);
    assert_eq!(format!("{critical:?}"), "Critical");
}
