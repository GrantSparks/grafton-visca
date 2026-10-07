//! The getter set shared by every public camera view (#805).
//!
//! The typed `Camera<P>` of each facade and both runtime-profile views expose
//! the same target, profile and submission-class accessors over their erased
//! owner core (a `core` field). [`camera_view_getters!`] writes them, and
//! their contracts, once.

/// Expands the shared camera-view getters inside a view's `impl` block.
///
/// `camera_view_getters!()` emits the getter set every view has;
/// `camera_view_getters!(runtime_profile)` adds the
/// [`supports_typed`](crate::capabilities::Capabilities::supports_typed)
/// query the runtime-profile views offer in place of compile-time markers.
macro_rules! camera_view_getters {
    (runtime_profile) => {
        $crate::camera_view::camera_view_getters!();

        /// Returns whether this view's profile permits one optional typed
        /// surface: the runtime counterpart of the compile-time marker traits.
        #[must_use]
        pub fn supports_typed(&self, surface: $crate::capabilities::TypedSupportSurface) -> bool {
            self.capabilities().supports_typed(surface)
        }
    };
    () => {
        /// Returns this view's fixed camera target.
        #[must_use]
        pub const fn target(&self) -> $crate::CameraId {
            self.core.target()
        }

        /// Returns this view's validated profile facts.
        #[must_use]
        pub fn profile(&self) -> &$crate::ProfileSpec {
            self.core.profile()
        }

        /// Returns this view's validated runtime capability inventory.
        ///
        /// This is the runtime discovery view of the same facts the
        /// compile-time marker traits gate: it reports what the profile
        /// documents, not permission to call a typed API.
        #[must_use]
        pub fn capabilities(&self) -> &$crate::capabilities::Capabilities {
            self.core.profile().capabilities()
        }

        /// Returns a cheap live read-only view of this camera's target-local
        /// state.
        #[must_use]
        pub fn state_cache(&self) -> $crate::StateCache {
            self.core.state_cache()
        }

        /// Returns this view's submission-class default, if it carries one.
        ///
        /// `None` — the initial value — means every request uses its intrinsic
        /// [`ControlClass`](crate::ControlClass). See
        /// [`set_submission_class`](Self::set_submission_class).
        #[must_use]
        pub const fn submission_class(&self) -> Option<$crate::SubmissionClass> {
            self.core.submission_class()
        }

        /// Derives a view whose ordinary work uses `class`.
        ///
        /// The original view is unchanged. Commands, inquiries, operations,
        /// and noun methods submitted through the returned view all inherit
        /// this class; intrinsically urgent stops remain urgent.
        pub fn with_submission_class(&self, class: $crate::SubmissionClass) -> Self {
            let mut selected = self.clone();
            selected.set_submission_class(Some(class));
            selected
        }

        /// Sets the ordinary-work [`SubmissionClass`](crate::SubmissionClass)
        /// every later submission from *this view* uses, or clears it with
        /// `None`.
        ///
        /// The owner dispatches ready work from the highest occupied class
        /// first and FIFO within a class, so lowering this default makes the
        /// view's traffic yield the transport to other views' ordinary work,
        /// and raising it makes the view's traffic overtake work that is still
        /// **queued**. It never interrupts, cancels, or reorders a request
        /// that has already been written to the transport, and it does not
        /// change the class of requests submitted before the call.
        ///
        /// The default is a property of *this view*, not of the camera or the
        /// session: another view onto the same target keeps its own value, and
        /// a [`clone`](Clone::clone) copies the current value and then
        /// diverges. A typed view projected out of a runtime-profile view
        /// starts from the same value. It applies to every request the view
        /// submits — commands, inquiries, and operations, including the ones
        /// the noun accessors submit — with exactly one exception.
        ///
        /// # Urgent requests are never demoted
        ///
        /// A request the crate classifies
        /// [`ControlClass::Urgent`](crate::ControlClass::Urgent) — the typed
        /// stops [`PanTiltStop`](crate::request::builtin::PanTiltStop),
        /// [`ZoomStop`](crate::request::builtin::ZoomStop),
        /// [`FocusStop`](crate::request::builtin::FocusStop), and owner-issued
        /// protocol cancellation — ignores this default and stays urgent. A
        /// view demoted to
        /// [`SubmissionClass::Background`](crate::SubmissionClass::Background)
        /// for telemetry polling therefore still preempts with an emergency
        /// stop. Per-submission overrides obey the same safety floor, and
        /// [`SubmissionClass`](crate::SubmissionClass) deliberately has no
        /// urgent variant for callers to manufacture.
        ///
        /// Owner-internal traffic that no caller submitted — the settlement
        /// polling behind a targeted operation's `settled` wait and the
        /// observation inquiries behind the motion view — keeps its own
        /// built-in class.
        pub fn set_submission_class(&mut self, class: Option<$crate::SubmissionClass>) {
            self.core.set_submission_class(class);
        }
    };
}

pub(crate) use camera_view_getters;
