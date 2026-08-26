//! Closed request-class markers.
//!
//! Request classes describe semantic admission capabilities. They are distinct
//! from the engine's private command/inquiry dispatch representation.

use core::marker::PhantomData;

/// A closed request class.
pub trait Class: private::Sealed + Send + Sync + 'static {}

/// A command that returns its final applied result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Plain;

/// A request that returns a typed inquiry response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Inquiry;

/// A physical operation with the completion semantics selected by `K`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Operation<K>(PhantomData<fn() -> K>);

impl Class for Plain {}
impl Class for Inquiry {}
impl<K> Class for Operation<K> where K: crate::completion::Kind {}

mod private {
    pub trait Sealed {}

    impl Sealed for super::Plain {}
    impl Sealed for super::Inquiry {}
    impl<K> Sealed for super::Operation<K> where K: crate::completion::Kind {}
}

/// Homogeneous built-in requests for the final typed request surface.
pub mod builtin;
