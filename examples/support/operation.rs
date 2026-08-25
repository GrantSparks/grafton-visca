//! Cleanup helpers used specifically by the operation-handle examples.

use grafton_visca::Error;

/// Keep running best-effort cleanup steps while retaining the first failure.
pub fn record_cleanup_error(
    first_error: &mut Option<Error>,
    result: Result<(), Error>,
    context: &str,
) {
    if let Err(error) = result {
        let error = error.with_context(format!("Cleanup failed while {context}"));
        if first_error.is_none() {
            *first_error = Some(error);
        } else {
            eprintln!("{error}");
        }
    }
}

/// Preserve the operation failure while still surfacing cleanup failure.
pub fn finish_cleanup<T>(
    operation: Result<T, Error>,
    cleanup: Result<(), Error>,
) -> Result<T, Error> {
    match (operation, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(operation_error), Ok(())) => Err(operation_error),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
        (Err(operation_error), Err(cleanup_error)) => {
            eprintln!("Cleanup also failed to restore the original state: {cleanup_error}");
            Err(operation_error)
        }
    }
}
