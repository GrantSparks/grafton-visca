#![cfg(feature = "async")]

use grafton_visca::{operation, r#async::Operation};

fn require_handle_traits<T: Send + Sync + Unpin>() {}

fn generic_operation<K: operation::Kind>() {
    require_handle_traits::<Operation<K>>();
}

fn main() {
    generic_operation::<operation::Targeted>();
    generic_operation::<operation::AppliedOnly>();
}
