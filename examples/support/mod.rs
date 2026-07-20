//! Shared helpers for runnable examples.

use grafton_visca::Error;

/// Preserve an operation error while still surfacing a simultaneous close error.
pub fn finish_session<T, C>(
    operation: Result<T, Error>,
    close: Result<C, Error>,
) -> Result<T, Error> {
    match (operation, close) {
        (Ok(value), Ok(_)) => Ok(value),
        (Err(operation_error), Ok(_)) => Err(operation_error),
        (Ok(_), Err(close_error)) => Err(close_error),
        (Err(operation_error), Err(close_error)) => {
            eprintln!("The camera session also failed to close cleanly: {close_error}");
            Err(operation_error)
        }
    }
}
