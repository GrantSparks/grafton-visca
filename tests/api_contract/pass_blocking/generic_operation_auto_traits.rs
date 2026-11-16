#![cfg(feature = "blocking")]

use grafton_visca::{blocking::Operation, operation};

fn require_handle_traits<T: Send + Sync + Unpin>() {}

fn generic_operation<K: operation::Kind>() {
    require_handle_traits::<Operation<K>>();
}

fn main() {
    generic_operation::<operation::Targeted>();
    generic_operation::<operation::AppliedOnly>();
}
