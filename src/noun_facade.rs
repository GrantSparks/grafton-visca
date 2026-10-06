//! Shared expansion helpers for the noun facades generated from
//! [`crate::noun_table`].

/// The one parser of a noun row's request form.
///
/// A row writes its request as `[<expr>]`, `[checked <expr>]` or
/// `[with_profile |<name>| <expr>]`; this macro evaluates any of them to
/// `crate::Result<$ty>`. `$profile` is substituted only into the
/// `with_profile` form, so a row that does not need the session profile never
/// reads it.
macro_rules! noun_request {
    ($profile:expr; $ty:ty; checked $request:expr) => {{
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr; $ty:ty; with_profile |$binding:ident| $request:expr) => {{
        let $binding = $profile;
        let request: crate::Result<$ty> = $request;
        request
    }};
    ($profile:expr; $ty:ty; $request:expr) => {{
        let request: $ty = $request;
        crate::Result::<$ty>::Ok(request)
    }};
}

pub(crate) use noun_request;
