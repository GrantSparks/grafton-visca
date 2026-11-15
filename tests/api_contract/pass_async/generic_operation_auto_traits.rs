#![cfg(feature = "async")]

use grafton_visca::{completion, Operation};

fn require_handle_traits<T: Send + Sync + Unpin>() {}

fn generic_operation<K: completion::Kind>() {
    require_handle_traits::<Operation<K>>();
}

fn main() {
    generic_operation::<completion::Targeted>();
    generic_operation::<completion::AppliedOnly>();
}
