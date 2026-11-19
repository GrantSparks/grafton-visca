//! Canonical noun requests and their private wire-contract forwarding.

/// The source facts used to connect a noun query to the inquiry registry.
///
/// The noun table owns the gate; the underlying inquiry inventory owns its
/// kind and source. Keeping these projections separate avoids another query
/// inventory while permitting queries with a different decoded value.
pub(crate) trait NounInquiryContract: crate::Inquiry {
    const KIND: crate::command::inquiry_structs::InquiryKind;
    const SOURCE: crate::source_evidence::Source;
}

/// A target-bound intent validates its ledger row and lowers its checked
/// values against the receiving registration.
macro_rules! movement_intent {
    (@policy Plain, $policy:expr) => { ($policy).command() };
    (@policy $class:ident, $policy:expr) => { ($policy).operation() };
    (@operation $type:ty, Plain) => {};
    (@operation $type:ty, $class:ident) => {
        impl $crate::OperationCommand<$crate::completion::$class> for $type {
            fn affected_axes(&self) -> $crate::AffectedAxes {
                const { $crate::command::semantics::fixed_axes::<Self>() }
            }
        }
    };
    ($type:ty, $inner:ty, $class:ident, $row:ident) => {
        impl $type {
            fn resolve(&self, target: &$crate::TargetRegistration) -> $crate::Result<$inner> {
                let row = $crate::command::semantics::BuiltinCommand::$row;
                $crate::request::builtin::validate_movement_row(row, target)?;
                self.lower(target)
            }
        }

        impl $crate::Request for $type {
            type Class = $crate::command::nouns::forward_request!(@class $class);
            const MAX_SIZE: usize = <$inner as $crate::Request>::MAX_SIZE;

            fn policy(&self) -> <Self::Class as $crate::request::Class>::Policy {
                $crate::command::nouns::movement_intent!(@policy $class,
                    <Self as $crate::command::semantics::BuiltinRequestContract>::POLICY)
            }

            #[allow(private_interfaces)]
            fn admission(
                &self,
                _authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::request::policy::CommandAdmission {
                <Self as $crate::command::semantics::BuiltinRequestContract>::ADMISSION
            }

            fn write_into(&self, _: $crate::CameraId, _: &mut [u8]) -> $crate::Result<usize> {
                Err($crate::requests::target_required_for_encoding())
            }

            #[allow(private_interfaces)]
            fn write_for_target(
                &self,
                target: &$crate::TargetRegistration,
                buffer: &mut [u8],
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<usize> {
                self.lower(target)?.write_for_target(target, buffer, authority)
            }

            fn validate_for_target(&self, target: &$crate::TargetRegistration) -> $crate::Result<()> {
                self.resolve(target).map(|_| ())
            }

            #[allow(private_interfaces)]
            fn lower_for_target<T>(
                &self,
                target: &$crate::TargetRegistration,
                buffer: &mut [u8],
                before_write: impl FnOnce() -> $crate::Result<T>,
                _authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<($crate::requests::LoweredRequest, T)> {
                let lowered = self.resolve(target)?;
                let result = before_write()?;
                Ok(($crate::requests::write_lowered(&lowered, target, buffer)?, result))
            }
        }

        $crate::command::nouns::movement_intent!(@operation $type, $class);

        impl $crate::command::semantics::BuiltinRequestContract for $type {
            const POLICY: $crate::request::builtin_policy::BuiltinPolicy =
                <$inner as $crate::command::semantics::BuiltinRequestContract>::POLICY;
            const LEDGER_ROW: $crate::command::semantics::BuiltinCommand =
                $crate::command::semantics::BuiltinCommand::$row;
            const LEDGER_ROWS: &'static [$crate::command::semantics::BuiltinCommand] =
                &[$crate::command::semantics::BuiltinCommand::$row];
            type State = <$inner as $crate::command::semantics::BuiltinRequestContract>::State;
        }
        const _: () = <$type as $crate::command::semantics::BuiltinRequestContract>::CLASS_MATCHES_ROW;
        $crate::command::nouns::forward_request!(@check_rows $inner, [$row]);
    };
}

pub(crate) use movement_intent;

/// Forwards a canonical request to its private wire request.
///
/// The ordinary form borrows a private `inner` field. A unit request supplies
/// its constant wire value after a semicolon. Parameterized requests may
/// instead supply `|request| expression` to construct their wire value from
/// checked fields. Constructors, fields, getters and derives belong to the
/// concrete request declaration.
///
/// `rows = [FirstRow, OtherRow]` declares the exact semantic rows of a
/// narrower wrapper. Its first row is the representative; every listed row
/// must preserve the inner request's class, axes and state effect.
///
/// The `select` form names every `(inner type, ledger row, inner value)` arm.
/// The optional fourth element checks the selected arm's value for the target.
/// The fallback supplies target-independent hooks. All hooks and state
/// descriptors come from this one arm list.
macro_rules! forward_request {
    (@class Plain) => { $crate::request::PlainClass };
    (@class Targeted) => { $crate::request::OperationClass<$crate::completion::Targeted> };
    (@class AppliedOnly) => { $crate::request::OperationClass<$crate::completion::AppliedOnly> };

    (@inner $this:ident [field]) => { &$this.inner };
    (@inner $this:ident [unit $inner:expr]) => { &const { $inner } };
    (@inner $this:ident [project |$request:ident| $inner:expr]) => {
        &{ let $request = $this; $inner }
    };

    (@validate $this:ident, $target:ident []) => { ::core::result::Result::<(), $crate::Error>::Ok(()) };
    (@validate $this:ident, $target:ident [|$request:ident, $registration:ident| $validate:expr]) => {{
        let $request = $this;
        let _ = &$request;
        let $registration = $target;
        let _ = &$registration;
        let _ = &$request;
        let _ = &$registration;
        $validate
    }};

    // The full Request contract is shared with inquiry forwarding. In
    // particular the authority-bearing hooks must reach the inner request;
    // falling back to their trait defaults would lose reviewed admission,
    // profile-selected bytes, or exact applied-state projection.
    (@request $type:ty, $inner:ty, $class:ty, [$($value:tt)*], [$($validate:tt)*]) => {
        impl $crate::Request for $type {
            type Class = $class;
            const MAX_SIZE: usize = <$inner as $crate::Request>::MAX_SIZE;
            fn policy(&self) -> <Self::Class as $crate::request::Class>::Policy {
                <$inner as $crate::Request>::policy(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn motion_axes(&self) -> ::core::option::Option<$crate::AffectedAxes> {
                <$inner as $crate::Request>::motion_axes(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            #[allow(private_interfaces)]
            fn admission(
                &self,
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::request::policy::AdmissionOf<Self::Class> {
                <$inner as $crate::Request>::admission(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            #[allow(private_interfaces)]
            fn declared_max_size(&self, authority: $crate::requests::RequestContractAuthority) -> usize {
                <$inner as $crate::Request>::declared_max_size(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            fn write_into(&self, camera_id: $crate::CameraId, buffer: &mut [u8]) -> $crate::Result<usize> {
                <$inner as $crate::Request>::write_into(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    camera_id,
                    buffer,
                )
            }
            #[allow(private_interfaces)]
            fn write_for_target(
                &self,
                target: &$crate::TargetRegistration,
                buffer: &mut [u8],
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<usize> {
                <$inner as $crate::Request>::write_for_target(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    target,
                    buffer,
                    authority,
                )
            }
            fn validate_for_target(&self, target: &$crate::TargetRegistration) -> $crate::Result<()> {
                $crate::command::nouns::forward_request!(@validate self, target [$($validate)*])?;
                <$inner as $crate::Request>::validate_for_target(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    target,
                )
            }
            #[allow(private_interfaces)]
            fn pan_tilt_max_speed(
                &self,
                authority: $crate::requests::RequestContractAuthority,
            ) -> ::core::option::Option<u8> {
                <$inner as $crate::Request>::pan_tilt_max_speed(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            #[cfg(any(feature = "blocking", feature = "async"))]
            #[allow(private_interfaces)]
            fn is_pan_tilt_drive(&self, authority: $crate::requests::RequestContractAuthority) -> bool {
                <$inner as $crate::Request>::is_pan_tilt_drive(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
            #[cfg(any(feature = "blocking", feature = "async"))]
            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                authority: $crate::requests::AppliedStateAuthority,
            ) -> ::core::option::Option<$crate::runtime::engine::AppliedStateProjection> {
                <$inner as $crate::Request>::applied_state_projection(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
        }
    };

    (@operation $type:ty, $inner:ty, Plain, [$($value:tt)*]) => {};
    (@operation $type:ty, $inner:ty, $class:ident, [$($value:tt)*]) => {
        impl $crate::OperationCommand<$crate::completion::$class> for $type {
            fn affected_axes(&self) -> $crate::AffectedAxes {
                <$inner as $crate::OperationCommand<$crate::completion::$class>>::affected_axes(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            #[allow(private_interfaces)]
            fn affected_axes_for_target(
                &self,
                target: &$crate::TargetRegistration,
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<$crate::AffectedAxes> {
                <$inner as $crate::OperationCommand<$crate::completion::$class>>::affected_axes_for_target(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    target,
                    authority,
                )
            }
        }
    };
    (@row $inner:ty, []) => {
        <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW
    };
    (@row $inner:ty, [$first:ident $(, $other:ident)*]) => {
        $crate::command::semantics::BuiltinCommand::$first
    };
    (@rows $inner:ty, []) => {
        <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS
    };
    (@rows $inner:ty, [$($row:ident),+]) => {
        &[$($crate::command::semantics::BuiltinCommand::$row),+]
    };
    (@check_rows $inner:ty, [$($row:ident),*]) => {
        $(const _: () = ::core::assert!(
            $crate::command::semantics::BuiltinCommand::$row.classification().same_contract(
                <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW.classification(),
            ),
            "a forwarded request row changes its inner class, axes or state-effect contract",
        );)*
        const _: () = {
            let declared: &[$crate::command::semantics::BuiltinCommand] =
                &[$($crate::command::semantics::BuiltinCommand::$row),*];
            let inherited = <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS;
            let mut index = 0;
            while index < declared.len() {
                let mut known = false;
                let mut candidate = 0;
                while candidate < inherited.len() {
                    if declared[index] as usize == inherited[candidate] as usize {
                        known = true;
                    }
                    candidate += 1;
                }
                ::core::assert!(known, "a forwarded request declares a row outside its inner request");
                let mut previous = 0;
                while previous < index {
                    ::core::assert!(
                        declared[index] as usize != declared[previous] as usize,
                        "a forwarded request repeats a ledger row",
                    );
                    previous += 1;
                }
                index += 1;
            }
        };
    };
    (@command $type:ty, $inner:ty, $class:ident, [$($value:tt)*], [$($row:ident),*]) => {
        $crate::command::nouns::forward_request!(
            @request $type, $inner,
            $crate::command::nouns::forward_request!(@class $class),
            [$($value)*], []
        );
        impl $crate::command::semantics::BuiltinRequestContract for $type {
            const POLICY: $crate::request::builtin_policy::BuiltinPolicy = <$inner as $crate::command::semantics::BuiltinRequestContract>::POLICY;
            const LEDGER_ROW: $crate::command::semantics::BuiltinCommand =
                $crate::command::nouns::forward_request!(@row $inner, [$($row),*]);
            type State = <$inner as $crate::command::semantics::BuiltinRequestContract>::State;
            const LEDGER_ROWS: &'static [$crate::command::semantics::BuiltinCommand] =
                $crate::command::nouns::forward_request!(@rows $inner, [$($row),*]);
            #[cfg(test)]
            fn selected_ledger_row(&self) -> $crate::command::semantics::BuiltinCommand {
                <$inner as $crate::command::semantics::BuiltinRequestContract>::selected_ledger_row(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
        }
        const _: () = <$type as $crate::command::semantics::BuiltinRequestContract>::CLASS_MATCHES_ROW;
        $crate::command::nouns::forward_request!(@check_rows $inner, [$($row),*]);
        $crate::command::nouns::forward_request!(@operation $type, $inner, $class, [$($value)*]);
    };

    (@selected_class $primary:ty, Inherited) => { <$primary as $crate::Request>::Class };
    (@selected_class $primary:ty, $class:ident) => { $crate::command::nouns::forward_request!(@class $class) };

    // Both field-selected and profile-selected wrappers expand through this
    // one forwarding contract. The declarations below are the only arm map.
    (@selection $this:ident, [] [$request:ident $registration:ident ($selector:expr) ($fallback:expr)]) => {{
        let $request = $this;
        let _ = &$request;
        $fallback
    }};
    (@selection $this:ident, [$target:ident] [$request:ident $registration:ident ($selector:expr) ($fallback:expr)]) => {{
        let $request = $this;
        let _ = &$request;
        let $registration = $target;
        let _ = &$registration;
        $selector
    }};
    (@write_selected value, $camera_id:ident, $buffer:ident, $encode:block) => { $encode };
    (@write_selected target, $camera_id:ident, $buffer:ident, $encode:block) => {{
        let _ = ($camera_id, $buffer);
        Err($crate::requests::target_required_for_encoding())
    }};
    (@selected $type:ty, $primary:ty, $class:ident, $encoding:ident,
        [$request:ident $registration:ident ($selector:expr) ($fallback:expr)],
        {$($pattern:pat => ($inner:ty, $row:ident, $value:expr $(, $validate:expr)?)),+ $(,)?}
    ) => {
        impl $crate::Request for $type {
            type Class = $crate::command::nouns::forward_request!(@selected_class $primary, $class);
            const MAX_SIZE: usize = {
                let mut size = <$primary as $crate::Request>::MAX_SIZE;
                $(if <$inner as $crate::Request>::MAX_SIZE > size { size = <$inner as $crate::Request>::MAX_SIZE; })+
                size
            };
            fn policy(&self) -> <Self::Class as $crate::request::Class>::Policy {
                let $request = self;
                let _ = &$request;
                match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::policy(&$value)),+
                }
            }
            #[allow(private_interfaces)]
            fn motion_axes(&self) -> Option<$crate::AffectedAxes> {
                let $request = self;
                let _ = &$request;
                match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::motion_axes(&$value)),+
                }
            }
            #[allow(private_interfaces)]
            #[allow(private_interfaces)]
            fn admission(&self, authority: $crate::requests::RequestContractAuthority) -> $crate::request::policy::AdmissionOf<Self::Class> {
                let $request = self;
                let _ = &$request;
                match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::admission(&$value, authority)),+
                }
            }
            #[allow(private_interfaces)]
            fn pan_tilt_max_speed(&self, authority: $crate::requests::RequestContractAuthority) -> Option<u8> {
                let $request = self;
                let _ = &$request;
                match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::pan_tilt_max_speed(&$value, authority)),+
                }
            }
            #[allow(private_interfaces)]
            fn write_into(&self, camera_id: $crate::CameraId, buffer: &mut [u8]) -> $crate::Result<usize> {
                $crate::command::nouns::forward_request!(@write_selected $encoding, camera_id, buffer, {
                    let $request = self;
                    let _ = &$request;
                    match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                        $($pattern => <$inner as $crate::Request>::write_into(&$value, camera_id, buffer)),+
                    }
                })
            }
            #[allow(private_interfaces)]
            fn write_for_target(&self, target: &$crate::TargetRegistration, buffer: &mut [u8], authority: $crate::requests::RequestContractAuthority) -> $crate::Result<usize> {
                let $request = self;
                let _ = &$request;
                let $registration = target;
                let _ = &$registration;
                match $crate::command::nouns::forward_request!(@selection self, [target] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::write_for_target(&$value, target, buffer, authority)),+
                }
            }
            #[allow(private_interfaces)]
            fn validate_for_target(&self, target: &$crate::TargetRegistration) -> $crate::Result<()> {
                let $request = self;
                let _ = &$request;
                let $registration = target;
                let _ = &$registration;
                match $crate::command::nouns::forward_request!(@selection self, [target] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => {
                        $($validate?;)?
                        <$inner as $crate::Request>::validate_for_target(&$value, target)
                    }),+
                }
            }
            #[allow(private_interfaces)]
            fn lower_for_target<T>(
                &self,
                target: &$crate::TargetRegistration,
                buffer: &mut [u8],
                before_write: impl FnOnce() -> $crate::Result<T>,
                authority: $crate::requests::RequestContractAuthority,
            ) -> $crate::Result<($crate::requests::LoweredRequest, T)> {
                let $request = self;
                let _ = &$request;
                let $registration = target;
                let _ = &$registration;
                match $crate::command::nouns::forward_request!(@selection self, [target] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => {
                        $($validate?;)?
                        <$inner as $crate::Request>::lower_for_target(&$value, target, buffer, before_write, authority)
                    }),+
                }
            }

            #[cfg(any(feature = "blocking", feature = "async"))]
            #[allow(private_interfaces)]
            fn applied_state_projection(&self, authority: $crate::requests::AppliedStateAuthority) -> Option<$crate::runtime::engine::AppliedStateProjection> {
                let $request = self;
                let _ = &$request;
                match $crate::command::nouns::forward_request!(@selection self, [] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => <$inner as $crate::Request>::applied_state_projection(&$value, authority)),+
                }
            }
        }
        impl $crate::command::semantics::BuiltinRequestContract for $type {
            const POLICY: $crate::request::builtin_policy::BuiltinPolicy = <$primary as $crate::command::semantics::BuiltinRequestContract>::POLICY;
            const LEDGER_ROW: $crate::command::semantics::BuiltinCommand = <$primary as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW;
            const LEDGER_ROWS: &'static [$crate::command::semantics::BuiltinCommand] = &[$($crate::command::semantics::BuiltinCommand::$row),+];
            type State = Self;
            #[cfg(test)]
            fn selected_ledger_row(&self) -> $crate::command::semantics::BuiltinCommand {
                self.ledger_row()
            }
            #[cfg(test)]
            fn selected_ledger_row_for_target(&self, target: &$crate::TargetRegistration) -> $crate::Result<$crate::command::semantics::BuiltinCommand> {
                let $request = self;
                let _ = &$request;
                let $registration = target;
                let _ = &$registration;
                Ok(match $crate::command::nouns::forward_request!(@selection self, [target] [$request $registration ($selector) ($fallback)]) {
                    $($pattern => $crate::command::semantics::BuiltinCommand::$row),+
                })
            }
        }
        impl $type {
            #[cfg(test)]
            pub(crate) const fn ledger_row(&self) -> $crate::command::semantics::BuiltinCommand {
                let $request = self;
                let _ = &$request;
                match $fallback { $($pattern => $crate::command::semantics::BuiltinCommand::$row),+ }
            }
        }
        #[cfg(any(feature = "blocking", feature = "async"))]
        impl $crate::request::builtin_contract::BuiltinStateProjection<$type> for $type {
            fn project(request: &$type) -> Option<$crate::request::builtin_contract::AppliedStateProjection> {
                $crate::Request::applied_state_projection(request, $crate::requests::AppliedStateAuthority::new())
            }
        }
        #[cfg(all(test, any(feature = "blocking", feature = "async")))]
        impl $crate::request::builtin_contract::StateDescriptor for $type {
            const KEY: Option<$crate::StateKey> = None;
            const BY_ROW: &'static [($crate::command::semantics::BuiltinCommand, Option<$crate::StateKey>)] = &[
                $(($crate::command::semantics::BuiltinCommand::$row,
                    <<$inner as $crate::command::semantics::BuiltinRequestContract>::State as $crate::request::builtin_contract::StateDescriptor>::KEY)),+
            ];
        }
        const _: () = <$type as $crate::command::semantics::BuiltinRequestContract>::CLASS_MATCHES_ROW;
        $(const _: () = {
            assert!(<$inner as $crate::Request>::MAX_SIZE <= <$type as $crate::Request>::MAX_SIZE, "selector exceeds its declared bound");
            assert!(<$inner as $crate::command::semantics::BuiltinRequestContract>::POLICY.timeout as u8 == <$type as $crate::command::semantics::BuiltinRequestContract>::POLICY.timeout as u8, "selector changes timeout class");
            assert!(<$inner as $crate::command::semantics::BuiltinRequestContract>::POLICY.retry as u8 == <$type as $crate::command::semantics::BuiltinRequestContract>::POLICY.retry as u8, "selector changes retry class");
            assert!(<$inner as $crate::command::semantics::BuiltinRequestContract>::POLICY.class as u8 == <$type as $crate::command::semantics::BuiltinRequestContract>::POLICY.class as u8, "selector changes submission class");
            let selected = $crate::command::semantics::BuiltinCommand::$row.classification();
            let primary = <$type as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW.classification();
            assert!(matches!((selected, primary), (
                $crate::command::semantics::BuiltinRequestClass::Plain { .. },
                $crate::command::semantics::BuiltinRequestClass::Plain { .. },
            )) || selected.same_contract(primary), "selector changes request class, axes or STOP authority");
            assert!($crate::command::semantics::BuiltinCommand::$row as usize == <$inner as $crate::command::semantics::BuiltinRequestContract>::LEDGER_ROW as usize, "selector row differs from inner request");
        };)+
    };
    // Profile-selected requests use the selected arm's validation, wire and
    // state projection. The fallback supplies only target-independent hooks.
    ($type:ty, $primary:ty, $class:ident;
        select target |$request:ident, $registration:ident| ($selector:expr) default ($fallback:expr)
        { $($pattern:pat => ($inner:ty, $row:ident, $value:expr $(, $validate:expr)?)),+ $(,)? }
    ) => {
        $crate::command::nouns::forward_request!(@selected $type, $primary, $class, target,
            [$request $registration ($selector) ($fallback)],
            {$($pattern => ($inner, $row, $value $(, $validate)?)),+}
        );
    };
    ($type:ty, $primary:ty, $class:ident;
        select |$request:ident, $registration:ident| ($selector:expr) default ($fallback:expr)
        { $($pattern:pat => ($inner:ty, $row:ident, $value:expr $(, $validate:expr)?)),+ $(,)? }
    ) => {
        $crate::command::nouns::forward_request!(@selected $type, $primary, $class, value,
            [$request $registration ($selector) ($fallback)],
            {$($pattern => ($inner, $row, $value $(, $validate)?)),+}
        );
    };

    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [field], [$($($row),+)?]);
    };
    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?; |$request:ident| $value:expr) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [project |$request| $value], [$($($row),+)?]);
    };
    ($type:ty, $inner:ty, $class:ident $(, rows = [$($row:ident),+ $(,)?])?; $value:expr) => {
        $crate::command::nouns::forward_request!(@command $type, $inner, $class, [unit $value], [$($($row),+)?]);
    };
}

pub(crate) use forward_request;

/// Forwards query encoding and decoding, including bound profile context.
///
/// `@request` supplies only Request and source metadata forwarding, for a
/// query with a different decoded response. It may add a target validator;
/// the concrete type then implements every Inquiry hook explicitly.
macro_rules! forward_inquiry {
    (@request_impl $type:ty, $inner:ty, [$($value:tt)*], [$($validate:tt)*]) => {
        $crate::command::nouns::forward_request!(
            @request $type, $inner, $crate::request::InquiryClass, [$($value)*],
            [|request, registration| {
                <$type as $crate::command::surface::BuiltinInquiryGate>::GATE.validate(
                    registration.profile(),
                    concat!("inquiry ", stringify!($type)),
                )?;
                $crate::command::nouns::forward_request!(@validate request, registration [$($validate)*])
            }]
        );
        impl $crate::command::nouns::NounInquiryContract for $type {
            const KIND: $crate::command::inquiry_structs::InquiryKind =
                <$inner as $crate::command::inquiry_structs::GeneratedInquiry>::KIND;
            const SOURCE: $crate::source_evidence::Source =
                <$inner as $crate::command::inquiry_structs::GeneratedInquiry>::SOURCE;
        }
    };
    (@inquiry $type:ty, $inner:ty, [$($value:tt)*]) => {
        impl $crate::Inquiry for $type {
            type Response = <$inner as $crate::Inquiry>::Response;
            fn payload_shape(&self) -> $crate::PayloadShape {
                <$inner as $crate::Inquiry>::payload_shape(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn payload_shape_for_profile(&self, profile: &$crate::ProfileSpec) -> $crate::PayloadShape {
                <$inner as $crate::Inquiry>::payload_shape_for_profile(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    profile,
                )
            }
            fn decoder(&self) -> $crate::ResponseDecoder<Self::Response> {
                <$inner as $crate::Inquiry>::decoder(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                )
            }
            fn decoder_for_profile(&self, profile: &$crate::ProfileSpec) -> $crate::Result<$crate::ResponseDecoder<Self::Response>> {
                <$inner as $crate::Inquiry>::decoder_for_profile(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    profile,
                )
            }
            #[allow(private_interfaces)]
            fn builtin_inquiry_syntax_retry(
                &self,
                authority: $crate::requests::BuiltinInquiryAuthority,
            ) -> $crate::requests::BuiltinInquiryProtocol {
                <$inner as $crate::Inquiry>::builtin_inquiry_syntax_retry(
                    $crate::command::nouns::forward_request!(@inner self [$($value)*]),
                    authority,
                )
            }
        }
    };

    (@request $type:ty, $inner:ty $(, validate = |$request:ident, $target:ident| $validate:expr)?) => {
        $crate::command::nouns::forward_inquiry!(
            @request_impl $type, $inner, [field], [$(|$request, $target| $validate)?]
        );
    };
    (@request $type:ty, $inner:ty $(, validate = |$request:ident, $target:ident| $validate:expr)?; $value:expr) => {
        $crate::command::nouns::forward_inquiry!(
            @request_impl $type, $inner, [unit $value], [$(|$request, $target| $validate)?]
        );
    };
    ($type:ty, $inner:ty) => {
        $crate::command::nouns::forward_inquiry!(@request_impl $type, $inner, [field], []);
        $crate::command::nouns::forward_inquiry!(@inquiry $type, $inner, [field]);
    };
    ($type:ty, $inner:ty; $value:expr) => {
        $crate::command::nouns::forward_inquiry!(@request_impl $type, $inner, [unit $value], []);
        $crate::command::nouns::forward_inquiry!(@inquiry $type, $inner, [unit $value]);
    };
}

pub(crate) use forward_inquiry;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{
        command::{
            inquiry_structs::ZoomPositionInquiry,
            semantics::{BuiltinCommand, BuiltinRequestContract},
            surface::{BuiltinInquiryGate, RowGate},
        },
        request::builtin::{ImageFlipCommand, ZoomDrive, ZoomStop},
        CameraId, Inquiry, OperationCommand, Request,
    };

    struct Flip {
        enabled: bool,
    }
    super::forward_request!(Flip, ImageFlipCommand, Plain; |request| ImageFlipCommand::new(request.enabled));

    struct Stop;
    super::forward_request!(Stop, ZoomStop, AppliedOnly; ZoomStop);

    struct Tele;
    super::forward_request!(Tele, ZoomDrive, AppliedOnly, rows = [ZoomTele]; ZoomDrive::Tele);

    struct Query;
    super::forward_inquiry!(Query, ZoomPositionInquiry; ZoomPositionInquiry);
    impl BuiltinInquiryGate for Query {
        const GATE: RowGate = RowGate::Always;
    }

    struct RestrictedQuery;
    super::forward_inquiry!(RestrictedQuery, ZoomPositionInquiry; ZoomPositionInquiry);
    impl BuiltinInquiryGate for RestrictedQuery {
        const GATE: RowGate = RowGate::Typed(crate::capabilities::TypedSupportSet::from_surface(
            crate::capabilities::TypedSupportSurface::UsbAudio,
        ));
    }

    struct CheckedQuery {
        inner: ZoomPositionInquiry,
        allowed: bool,
    }
    super::forward_inquiry!(@request CheckedQuery, ZoomPositionInquiry,
        validate = |request, _target| {
            if request.allowed {
                Ok(())
            } else {
                Err(crate::Error::FeatureNotSupported { feature: "query domain" })
            }
        }
    );
    super::forward_inquiry!(@inquiry CheckedQuery, ZoomPositionInquiry, [field]);
    impl BuiltinInquiryGate for CheckedQuery {
        const GATE: RowGate = RowGate::Always;
    }

    fn target() -> crate::TargetRegistration {
        crate::TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                .expect("generic profile"),
        )
    }

    #[test]
    fn projections_preserve_value_selected_rows_and_applied_state() {
        for enabled in [false, true] {
            let request = Flip { enabled };
            let inner = ImageFlipCommand::new(enabled);
            assert_eq!(
                request.selected_ledger_row(),
                if enabled {
                    BuiltinCommand::ImageFlipVertical
                } else {
                    BuiltinCommand::ImageFlipOff
                }
            );
            let mut actual = [0; 6];
            let mut expected = [0; 6];
            assert_eq!(
                request
                    .write_into(CameraId::CAMERA_1, &mut actual)
                    .expect("forwarded frame"),
                inner
                    .write_into(CameraId::CAMERA_1, &mut expected)
                    .expect("inner frame")
            );
            assert_eq!(actual, expected);
            #[cfg(any(feature = "blocking", feature = "async"))]
            assert_eq!(
                crate::requests::applied_state_projection(&request),
                crate::requests::applied_state_projection(&inner)
            );
        }
        assert_eq!(
            <Flip as BuiltinRequestContract>::LEDGER_ROWS,
            <ImageFlipCommand as BuiltinRequestContract>::LEDGER_ROWS
        );
    }

    #[test]
    fn unit_operations_retain_reserved_admission_authority_and_exact_axes() {
        let request = Stop;
        assert_eq!(
            request.admission(crate::requests::RequestContractAuthority::new()),
            crate::request::policy::CommandAdmission::Stop
        );
        assert_eq!(request.affected_axes(), crate::AffectedAxes::ZOOM);
        assert_eq!(request.selected_ledger_row(), BuiltinCommand::ZoomStop);
        let mut actual = [0; 6];
        let mut expected = [0; 6];
        assert_eq!(
            request
                .write_into(CameraId::CAMERA_1, &mut actual)
                .expect("forwarded frame"),
            ZoomStop
                .write_into(CameraId::CAMERA_1, &mut expected)
                .expect("inner frame")
        );
        assert_eq!(actual, expected);
        assert_eq!(
            <Tele as BuiltinRequestContract>::LEDGER_ROWS,
            &[BuiltinCommand::ZoomTele]
        );
        assert_eq!(Tele.selected_ledger_row(), BuiltinCommand::ZoomTele);
        assert_eq!(Tele.affected_axes(), crate::AffectedAxes::ZOOM);
    }

    #[test]
    fn unit_inquiries_retain_profile_decoding_and_source_kind() {
        let target = target();
        let query = Query;
        assert_eq!(
            query.payload_shape_for_profile(target.profile()),
            ZoomPositionInquiry.payload_shape_for_profile(target.profile())
        );
        assert_eq!(
            query
                .decoder_for_profile(target.profile())
                .expect("profile-bound decoder")
                .decode(&[0, 1, 2, 3])
                .expect("forwarded response"),
            ZoomPositionInquiry
                .decoder_for_profile(target.profile())
                .expect("profile-bound decoder")
                .decode(&[0, 1, 2, 3])
                .expect("inner response")
        );
        assert_eq!(
            <Query as super::NounInquiryContract>::KIND,
            <ZoomPositionInquiry as crate::command::inquiry_structs::GeneratedInquiry>::KIND
        );
        let _ = <Query as super::NounInquiryContract>::SOURCE;
        #[cfg(any(feature = "blocking", feature = "async"))]
        assert!(crate::requests::builtin_inquiry_protocol(&query).syntax_retry);
    }

    #[test]
    fn request_only_inquiry_forwarding_checks_each_target() {
        let target = target();
        let mut query = CheckedQuery {
            inner: ZoomPositionInquiry,
            allowed: true,
        };
        assert!(query.validate_for_target(&target).is_ok());
        query.allowed = false;
        assert!(matches!(
            query.validate_for_target(&target),
            Err(crate::Error::FeatureNotSupported {
                feature: "query domain"
            })
        ));
    }

    #[test]
    fn canonical_inquiry_gate_is_checked_before_the_private_wire_query() {
        let target = target();
        assert!(ZoomPositionInquiry.validate_for_target(&target).is_ok());
        assert!(matches!(
            RestrictedQuery.validate_for_target(&target),
            Err(crate::Error::FeatureNotSupported {
                feature: "inquiry RestrictedQuery"
            })
        ));
    }
}
