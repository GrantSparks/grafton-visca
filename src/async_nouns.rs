//! Static, owner-backed noun accessors for the async camera facade.
//!
//! Each accessor is a thin view over [`crate::async_session::AsyncCameraCore`]
//! through its typed [`crate::Camera`] owner.  Request preparation, profile
//! validation, admission, and operation completion therefore remain in the
//! same path as [`crate::Camera::execute`], [`crate::Camera::inquire`], and
//! [`crate::Camera::submit`].
//!
//! An async noun method borrows its accessor for the returned future. Bind the
//! accessor before passing multiple noun futures to `join!`, `select!`, or a
//! task collection; see the concurrency note in the 2.0 usage guide. A direct
//! `camera.power().on().await` remains valid.
//!
//! Nothing in this module is written by hand: the accessors, their getters
//! and their methods are generated from [`crate::noun_table`] by the static
//! facade consumers in [`crate::noun_facade`], which expand the blocking
//! facade from the same rows.  This module only selects the async mode:
//! `async fn` plus `.await`.

#![cfg(feature = "async")]

use crate::{
    async_session::{AsyncCameraCore, Camera},
    command,
    noun_facade::{static_motion_facade, static_noun_facade},
    noun_table::{motion_table, noun_table},
    operation::Operation,
    request::builtin,
    types,
    units::{Degrees, UnitInterval},
    ZoomDomain,
};

noun_table!(All => static_noun_facade, [async], [.await]);

motion_table!(static_motion_facade, [async], [.await], AsyncCameraCore);
