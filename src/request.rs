//! Closed request-class markers.
//!
//! Request classes describe semantic admission capabilities. They are distinct
//! from the engine's private command/inquiry dispatch representation.

use core::marker::PhantomData;

/// A closed request class.
pub trait Class: private::Sealed + Send + Sync + 'static {
    /// The policy meaningful for this request lifecycle.
    type Policy: policy::Sealed;
}

/// A command that returns its final applied result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PlainClass;

/// A request that returns a typed inquiry response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct InquiryClass;

/// A physical operation with the completion semantics selected by `K`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct OperationClass<K>(PhantomData<fn() -> K>);

impl Class for PlainClass {
    type Policy = CommandPolicy;
}
impl Class for InquiryClass {
    type Policy = InquiryPolicy;
}
impl<K> Class for OperationClass<K>
where
    K: crate::completion::Kind,
{
    type Policy = OperationPolicy;
}

mod private {
    pub trait Sealed {}

    impl Sealed for super::PlainClass {}
    impl Sealed for super::InquiryClass {}
    impl<K> Sealed for super::OperationClass<K> where K: crate::completion::Kind {}
}

/// Homogeneous built-in requests for the final typed request surface.
pub(crate) mod builtin;

#[cfg_attr(not(any(feature = "blocking", feature = "async")), allow(dead_code))]
pub(crate) mod builtin_contract;
pub(crate) mod builtin_policy;

pub use crate::requests::{
    CommandRetryClass, Inquiry, InquiryRetryClass, OperationCommand, PayloadShape, PlainCommand,
    Request, ResponseDecoder, SubmissionClass,
};

pub(crate) mod policy;
pub use policy::{CommandPolicy, InquiryPolicy, OperationPolicy};
