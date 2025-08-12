//! Internal utilities for command processing.

use crate::command::Response;
use crate::Error;

/// Maps acknowledgment responses to unit result.
///
/// Converts successful command acknowledgments (CmdAck, Completion) to Ok(()),
/// and all other responses to appropriate errors.
#[allow(dead_code)]
pub(crate) fn map_ack_to_unit(resp: Response) -> Result<(), Error> {
    match resp {
        Response::Completion | Response::CmdAck => Ok(()),
        Response::Error(e) => Err(e),
        _ => Err(Error::UnexpectedResponseType),
    }
}
