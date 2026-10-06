//! Canonical static noun views for the blocking owner-backed camera.
//!
//! The accessors in this module are deliberately small borrowed views.  They
//! do not own a transport, create a runtime, or introduce another command
//! path: every operation goes through [`super::BlockingCameraCore`].  The
//! method spelling follows the closed ledger in `command::surface` and the
//! return class is visible in the operation handle (`AppliedOnly` or
//! `Targeted`).
//!
//! Nothing in this module is written by hand: the accessors, their getters
//! and their methods are generated from [`crate::noun_table`] by the static
//! facade consumers in [`crate::noun_facade`], which expand the async facade
//! from the same rows.  This module only selects the synchronous mode: no
//! `async`, no `.await`.

use crate::{
    command,
    noun_facade::{static_motion_facade, static_noun_facade},
    noun_table::{motion_table, noun_table},
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    ZoomDomain,
};

use super::{BlockingCameraCore, Camera, Operation};

noun_table!(All => static_noun_facade, [], []);

motion_table!(static_motion_facade, [], [], BlockingCameraCore);
