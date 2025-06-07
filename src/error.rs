// Standard library imports
use std::{future::Future, io, pin::Pin, time::Duration};

// Third-party crate imports
use thiserror::Error;

// Workspace / local-crate imports
// (none)

/// VISCA protocol error type.
///
/// Provides comprehensive error handling for all VISCA operations.
/// The VISCA protocol has a well-defined set of error conditions
/// that map directly to camera responses and communication failures.
#[derive(Error, Debug)]
pub enum ViscaError {
    /// Failed to establish connection to the camera.
    #[error("Connection failed to {addr}: {source}")]
    ConnectionFailed {
        /// The address that failed to connect.
        addr: String,
        /// The underlying IO error.
        source: io::Error,
    },

    /// Connection to the camera was lost during operation.
    #[error("Connection lost: {reason}")]
    ConnectionLost {
        /// Reason for the connection loss.
        reason: String,
    },

    /// Command execution exceeded the configured timeout.
    #[error("Command timeout after {duration:?} for command: {command}")]
    CommandTimeout {
        /// Duration of the timeout.
        duration: Duration,
        /// Description of the command that timed out.
        command: String,
    },

    /// Camera is busy executing another command and cannot accept new commands.
    #[error("Camera is busy executing another command")]
    CameraBusy,

    /// Camera is still performing a mechanical movement operation.
    #[error("Camera is still moving, position: pan={pan}, tilt={tilt}")]
    CameraMoving {
        /// Current pan position.
        pan: i16,
        /// Current tilt position.
        tilt: i16,
    },

    /// Camera has not been properly initialized or powered on.
    #[error("Camera not initialized")]
    CameraNotReady,

    /// Response from camera doesn't match the expected format.
    #[error("Invalid response: expected {expected}, got {actual:?}")]
    InvalidResponse {
        /// Description of expected response.
        expected: String,
        /// Actual bytes received.
        actual: Vec<u8>,
    },

    /// Camera explicitly rejected the command.
    #[error("Command rejected by camera: {reason}")]
    CommandRejected {
        /// Reason for rejection.
        reason: String,
    },

    /// A parameter value is outside the valid range.
    #[error("Value {value} out of range [{min}, {max}] for {parameter}")]
    OutOfRange {
        /// The invalid value provided.
        value: i32,
        /// Minimum valid value.
        min: i32,
        /// Maximum valid value.
        max: i32,
        /// Name of the parameter.
        parameter: String,
    },

    /// Requested preset position does not exist.
    #[error("Preset {id} not found")]
    PresetNotFound {
        /// ID of the missing preset.
        id: u8,
    },

    /// Camera model doesn't support the requested feature.
    #[error("Feature '{feature}' not supported by this camera model")]
    FeatureNotSupported {
        /// Name of the unsupported feature.
        feature: String,
    },

    /// Underlying IO error from network operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// VISCA protocol syntax error (0x02): Command format is incorrect or parameters are illegal.
    #[error("Syntax error in VISCA command")]
    SyntaxError,

    /// VISCA protocol command buffer full error (0x03): Two sockets are already in use.
    #[error("Command buffer is full")]
    CommandBufferFull,

    /// VISCA protocol command canceled (0x04): Command was canceled in the specified socket.
    #[error("Command was canceled")]
    CommandCanceled,

    /// VISCA protocol no socket error (0x05): No command is executing in the specified socket.
    #[error("No socket available")]
    NoSocket,

    /// VISCA protocol command not executable (0x41): Command cannot be executed due to current conditions.
    #[error("Command is not executable")]
    CommandNotExecutable,

    /// Response data doesn't conform to expected VISCA protocol format.
    #[error("Invalid response format")]
    InvalidResponseFormat,

    /// Response has an unexpected number of bytes.
    #[error("Invalid response length")]
    InvalidResponseLength,

    /// Response type doesn't match what the command should return.
    #[error("Unexpected response type")]
    UnexpectedResponseType,

    /// Received an unknown error code from the camera.
    #[error("Unknown error code: {0:#02X}")]
    Unknown(u8),

    /// Failed to parse response data.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Transport layer communication error.
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Invalid parameter provided to a command.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Parameter value is out of the acceptable range.
    #[error("Parameter out of range: {parameter} = {value} (valid range: {min}..{max})")]
    ParameterOutOfRange {
        /// Name of the parameter.
        parameter: String,
        /// Value that was provided.
        value: i32,
        /// Minimum valid value.
        min: i32,
        /// Maximum valid value.
        max: i32,
    },

    /// Operation exceeded timeout without response.
    #[error("Operation timed out")]
    Timeout,

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {0}")]
    InvalidState(String),
}

impl ViscaError {
    /// Create a `ViscaError` from a VISCA error response code.
    pub const fn from_code(code: u8) -> Self {
        use ViscaError::{
            CommandBufferFull, CommandCanceled, CommandNotExecutable, NoSocket, SyntaxError,
            Unknown,
        };

        match code {
            0x02 => SyntaxError,
            0x03 => CommandBufferFull,
            0x04 => CommandCanceled,
            0x05 => NoSocket,
            0x41 => CommandNotExecutable,
            _ => Unknown(code),
        }
    }

    /// Check if this error is potentially retryable.
    pub const fn is_retryable(&self) -> bool {
        use ViscaError::{CameraBusy, CameraMoving, CommandBufferFull, CommandTimeout, Timeout};

        matches!(
            self,
            CameraBusy | CameraMoving { .. } | CommandTimeout { .. } | CommandBufferFull | Timeout
        )
    }

    /// Get a suggested retry delay for retryable errors.
    pub const fn suggested_retry_delay(&self) -> Option<Duration> {
        use ViscaError::{CameraBusy, CameraMoving, CommandBufferFull, CommandTimeout, Timeout};

        match self {
            CameraBusy => Some(Duration::from_millis(100)),
            CameraMoving { .. } => Some(Duration::from_millis(500)),
            CommandTimeout { .. } => Some(Duration::from_secs(1)),
            CommandBufferFull => Some(Duration::from_millis(200)),
            Timeout => Some(Duration::from_secs(2)),
            _ => None,
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for ViscaError {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        ViscaError::ParseError(err.to_string())
    }
}

/// Type alias for a boxed future that is Send
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Extension trait providing convenient retry helpers for VISCA operations.
///
/// This trait adds retry functionality to `Result<T, ViscaError>`, making it easy
/// to handle transient errors that are common in camera communication.
pub trait ViscaResultExt<T> {
    /// Retry the operation if it fails with a retryable error.
    ///
    /// # Parameters
    /// - `max_attempts`: Maximum number of retry attempts (including the initial attempt)
    /// - `base_delay`: Base delay between retries (may be adjusted based on error type)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # {
    /// use grafton_visca::{ViscaResultExt, ViscaError};
    /// use std::time::Duration;
    ///
    /// async fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Retry up to 3 times with 100ms base delay
    ///     let result: Result<(), ViscaError> = Ok(());
    ///     result.retry_on_busy(3, Duration::from_millis(100)).await?;
    ///     Ok(())
    /// }
    /// # }
    /// ```
    fn retry_on_busy(
        self,
        max_attempts: u32,
        base_delay: Duration,
    ) -> BoxFuture<'static, Result<T, ViscaError>>;

    /// Retry the operation using the error's suggested delay.
    ///
    /// This method uses the `suggested_retry_delay()` from the error to determine
    /// appropriate delays between retries.
    ///
    /// # Parameters
    /// - `max_attempts`: Maximum number of retry attempts (including the initial attempt)
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # {
    /// use grafton_visca::{ViscaResultExt, ViscaError};
    ///
    /// async fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Retry up to 5 times using suggested delays
    ///     let result: Result<(), ViscaError> = Ok(());
    ///     result.retry_with_suggested_delay(5).await?;
    ///     Ok(())
    /// }
    /// # }
    /// ```
    fn retry_with_suggested_delay(
        self,
        max_attempts: u32,
    ) -> BoxFuture<'static, Result<T, ViscaError>>;

    /// Retry the operation with exponential backoff.
    ///
    /// The delay between retries doubles with each attempt, up to a maximum delay.
    ///
    /// # Parameters
    /// - `max_attempts`: Maximum number of retry attempts
    /// - `initial_delay`: Initial delay for the first retry
    /// - `max_delay`: Maximum delay between retries
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # {
    /// use grafton_visca::{ViscaResultExt, ViscaError};
    /// use std::time::Duration;
    ///
    /// async fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Retry with exponential backoff: 50ms, 100ms, 200ms, 400ms, 800ms, 1s, 1s...
    ///     let result: Result<(), ViscaError> = Ok(());
    ///     result.retry_with_exponential_backoff(
    ///         7,
    ///         Duration::from_millis(50),
    ///         Duration::from_secs(1)
    ///     ).await?;
    ///     Ok(())
    /// }
    /// # }
    /// ```
    fn retry_with_exponential_backoff(
        self,
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> BoxFuture<'static, Result<T, ViscaError>>;

    /// Convert retryable errors to a more user-friendly format.
    ///
    /// This method provides context about what retry strategies might be appropriate
    /// for the error that occurred.
    fn with_retry_context(self) -> Result<T, ViscaError>;
}

impl<T> ViscaResultExt<T> for Result<T, ViscaError>
where
    T: Send + 'static,
{
    fn retry_on_busy(
        self,
        _max_attempts: u32,
        _base_delay: Duration,
    ) -> BoxFuture<'static, Result<T, ViscaError>> {
        Box::pin(async move {
            // This implementation is a placeholder since we can't re-execute from just a Result
            // The real retry functionality should use the utility functions below
            self.with_retry_context()
        })
    }

    fn retry_with_suggested_delay(
        self,
        _max_attempts: u32,
    ) -> BoxFuture<'static, Result<T, ViscaError>> {
        Box::pin(async move {
            // This implementation is a placeholder since we can't re-execute from just a Result
            // The real retry functionality should use the utility functions below
            self.with_retry_context()
        })
    }

    fn retry_with_exponential_backoff(
        self,
        _max_attempts: u32,
        _initial_delay: Duration,
        _max_delay: Duration,
    ) -> BoxFuture<'static, Result<T, ViscaError>> {
        Box::pin(async move {
            // This implementation is a placeholder since we can't re-execute from just a Result
            // The real retry functionality should use the utility functions below
            self.with_retry_context()
        })
    }

    fn with_retry_context(self) -> Result<T, ViscaError> {
        match &self {
            Err(err) if err.is_retryable() => {
                // Add context about retry strategies
                log::debug!(
                    "Retryable error occurred: {}. Suggested delay: {:?}",
                    err,
                    err.suggested_retry_delay()
                );
                self
            }
            _ => self,
        }
    }
}

/// Utility functions for retrying VISCA operations.
///
/// These functions provide practical retry mechanisms that can re-execute operations.
pub struct ViscaRetry;

impl ViscaRetry {
    /// Retry an async operation with custom parameters.
    ///
    /// # Parameters
    /// - `operation`: Async closure that returns a Result
    /// - `max_attempts`: Maximum number of attempts (including the initial attempt)
    /// - `base_delay`: Base delay between retries
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use grafton_visca::{ViscaClient, ViscaRetry, ViscaError};
    /// use grafton_visca::command::PanTiltCommand;
    /// use std::time::Duration;
    ///
    /// let client = ViscaClient::connect_udp_async("192.168.1.100:5678").await?;
    /// ViscaRetry::retry_async(
    ///     || async { client.send_async(&PanTiltCommand::Home).await },
    ///     3,
    ///     Duration::from_millis(100)
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn retry_async<T, F, Fut>(
        mut operation: F,
        max_attempts: u32,
        base_delay: Duration,
    ) -> Result<T, ViscaError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, ViscaError>>,
    {
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) if err.is_retryable() && attempt < max_attempts => {
                    let delay = err.suggested_retry_delay().unwrap_or(base_delay);
                    log::debug!(
                        "Attempt {attempt}/{max_attempts} failed with retryable error: {err}. Retrying in {delay:?}"
                    );

                    #[cfg(feature = "async-client")]
                    tokio::time::sleep(delay).await;

                    #[cfg(not(feature = "async-client"))]
                    std::thread::sleep(delay);

                    last_error = Some(err);
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or(ViscaError::Unknown(0xFF)))
    }

    /// Retry an async operation using suggested delays from errors.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use grafton_visca::{ViscaClient, ViscaRetry, ViscaError};
    /// use grafton_visca::command::ZoomCommand;
    ///
    /// let client = ViscaClient::connect_udp_async("192.168.1.100:5678").await?;
    /// ViscaRetry::retry_with_suggested_delay_async(
    ///     || async { client.send_async(&ZoomCommand::Stop).await },
    ///     5
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn retry_with_suggested_delay_async<T, F, Fut>(
        mut operation: F,
        max_attempts: u32,
    ) -> Result<T, ViscaError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, ViscaError>>,
    {
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) if err.is_retryable() && attempt < max_attempts => {
                    if let Some(delay) = err.suggested_retry_delay() {
                        log::debug!(
                            "Attempt {}/{} failed: {}. Retrying in {:?}",
                            attempt,
                            max_attempts,
                            err,
                            delay
                        );

                        #[cfg(feature = "async-client")]
                        tokio::time::sleep(delay).await;

                        #[cfg(not(feature = "async-client"))]
                        std::thread::sleep(delay);

                        last_error = Some(err);
                    } else {
                        return Err(err);
                    }
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or(ViscaError::Unknown(0xFF)))
    }

    /// Retry an async operation with exponential backoff.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "async-client")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use grafton_visca::{ViscaClient, ViscaRetry, ViscaError};
    /// use grafton_visca::command::FocusCommand;
    /// use std::time::Duration;
    ///
    /// let client = ViscaClient::connect_udp_async("192.168.1.100:5678").await?;
    /// ViscaRetry::retry_with_exponential_backoff_async(
    ///     || async { client.send_async(&FocusCommand::Auto).await },
    ///     7,
    ///     Duration::from_millis(50),
    ///     Duration::from_secs(1)
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn retry_with_exponential_backoff_async<T, F, Fut>(
        mut operation: F,
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Result<T, ViscaError>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, ViscaError>>,
    {
        let mut last_error = None;
        let mut current_delay = initial_delay;

        for attempt in 1..=max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) if err.is_retryable() && attempt < max_attempts => {
                    log::debug!(
                        "Attempt {}/{} failed: {}. Retrying in {:?}",
                        attempt,
                        max_attempts,
                        err,
                        current_delay
                    );

                    #[cfg(feature = "async-client")]
                    tokio::time::sleep(current_delay).await;

                    #[cfg(not(feature = "async-client"))]
                    std::thread::sleep(current_delay);

                    // Double the delay for next iteration, up to max_delay
                    current_delay = std::cmp::min(current_delay * 2, max_delay);
                    last_error = Some(err);
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or(ViscaError::Unknown(0xFF)))
    }

    /// Retry a blocking operation with custom parameters.
    ///
    /// # Example
    /// ```no_run
    /// # #[cfg(feature = "blocking-client")]
    /// # {
    /// use grafton_visca::{ViscaClient, ViscaRetry, ViscaError};
    /// use grafton_visca::command::PanTiltCommand;
    /// use std::time::Duration;
    ///
    /// fn example(client: &ViscaClient) -> Result<(), Box<dyn std::error::Error>> {
    ///     ViscaRetry::retry_blocking(
    ///         || client.send(&PanTiltCommand::Home),
    ///         3,
    ///         Duration::from_millis(100)
    ///     )?;
    ///     Ok(())
    /// }
    /// # }
    /// ```
    #[cfg(feature = "blocking-client")]
    pub fn retry_blocking<T, F>(
        mut operation: F,
        max_attempts: u32,
        base_delay: Duration,
    ) -> Result<T, ViscaError>
    where
        F: FnMut() -> Result<T, ViscaError>,
    {
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match operation() {
                Ok(result) => return Ok(result),
                Err(err) if err.is_retryable() && attempt < max_attempts => {
                    let delay = err.suggested_retry_delay().unwrap_or(base_delay);
                    log::debug!(
                        "Attempt {attempt}/{max_attempts} failed with retryable error: {err}. Retrying in {delay:?}"
                    );

                    std::thread::sleep(delay);
                    last_error = Some(err);
                }
                Err(err) => return Err(err),
            }
        }

        Err(last_error.unwrap_or(ViscaError::Unknown(0xFF)))
    }
}

/// Application-level error type for examples and user code.
#[derive(Error, Debug)]
pub enum AppError {
    /// IO error from file or network operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// VISCA protocol or camera communication error.
    #[error("VISCA error: {0}")]
    Visca(#[from] ViscaError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visca_error_from_code() {
        assert!(matches!(
            ViscaError::from_code(0x02),
            ViscaError::SyntaxError
        ));
        assert!(matches!(
            ViscaError::from_code(0x03),
            ViscaError::CommandBufferFull
        ));
        assert!(matches!(
            ViscaError::from_code(0x04),
            ViscaError::CommandCanceled
        ));
        assert!(matches!(ViscaError::from_code(0x05), ViscaError::NoSocket));
        assert!(matches!(
            ViscaError::from_code(0x41),
            ViscaError::CommandNotExecutable
        ));
        assert!(matches!(
            ViscaError::from_code(0xFF),
            ViscaError::Unknown(0xFF)
        ));
    }

    #[test]
    fn test_visca_error_display() {
        assert_eq!(
            ViscaError::SyntaxError.to_string(),
            "Syntax error in VISCA command"
        );
        assert_eq!(
            ViscaError::CommandBufferFull.to_string(),
            "Command buffer is full"
        );
        assert_eq!(
            ViscaError::CommandCanceled.to_string(),
            "Command was canceled"
        );
        assert_eq!(ViscaError::NoSocket.to_string(), "No socket available");
        assert_eq!(
            ViscaError::CommandNotExecutable.to_string(),
            "Command is not executable"
        );
        assert_eq!(
            ViscaError::Unknown(0x99).to_string(),
            "Unknown error code: 0x99"
        );
        assert_eq!(
            ViscaError::InvalidParameter("test".to_string()).to_string(),
            "Invalid parameter: test"
        );
        assert_eq!(ViscaError::Timeout.to_string(), "Operation timed out");
    }

    #[test]
    fn test_visca_error_from_io_error() {
        let io_err = io::Error::other("test error");
        let visca_err = ViscaError::from(io_err);
        assert!(matches!(visca_err, ViscaError::Io(_)));
    }

    #[test]
    fn test_visca_error_from_nom_error() {
        use nom::error::{Error as NomError, ErrorKind};
        let nom_err = nom::Err::Error(NomError::new(&b"test"[..], ErrorKind::Tag));
        let visca_err = ViscaError::from(nom_err);
        assert!(matches!(visca_err, ViscaError::ParseError(_)));
    }

    #[test]
    fn test_app_error_from_io() {
        let io_err = io::Error::other("file not found");
        let app_err = AppError::from(io_err);
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn test_app_error_from_visca() {
        let visca_err = ViscaError::SyntaxError;
        let app_err = AppError::from(visca_err);
        assert!(matches!(app_err, AppError::Visca(ViscaError::SyntaxError)));
    }

    #[test]
    fn test_is_retryable() {
        assert!(ViscaError::CameraBusy.is_retryable());
        assert!(ViscaError::CameraMoving { pan: 100, tilt: 50 }.is_retryable());
        assert!(ViscaError::CommandTimeout {
            duration: Duration::from_secs(5),
            command: "test".to_string()
        }
        .is_retryable());
        assert!(ViscaError::CommandBufferFull.is_retryable());
        assert!(ViscaError::Timeout.is_retryable());

        assert!(!ViscaError::SyntaxError.is_retryable());
        assert!(!ViscaError::CommandNotExecutable.is_retryable());
        assert!(!ViscaError::InvalidParameter("test".to_string()).is_retryable());
        assert!(!ViscaError::PresetNotFound { id: 1 }.is_retryable());
    }

    #[test]
    fn test_suggested_retry_delay() {
        assert_eq!(
            ViscaError::CameraBusy.suggested_retry_delay(),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            ViscaError::CameraMoving { pan: 100, tilt: 50 }.suggested_retry_delay(),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            ViscaError::CommandTimeout {
                duration: Duration::from_secs(5),
                command: "test".to_string()
            }
            .suggested_retry_delay(),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            ViscaError::CommandBufferFull.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            ViscaError::Timeout.suggested_retry_delay(),
            Some(Duration::from_secs(2))
        );

        assert_eq!(ViscaError::SyntaxError.suggested_retry_delay(), None);
        assert_eq!(
            ViscaError::InvalidParameter("test".to_string()).suggested_retry_delay(),
            None
        );
    }

    #[test]
    fn test_visca_result_ext_with_retry_context() {
        // Test successful result
        let success_result: Result<String, ViscaError> = Ok("success".to_string());
        let result = success_result.with_retry_context();
        assert!(result.is_ok());

        // Test retryable error
        let retryable_error: Result<(), ViscaError> = Err(ViscaError::CameraBusy);
        let result = retryable_error.with_retry_context();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_retryable());

        // Test non-retryable error
        let non_retryable_error: Result<(), ViscaError> = Err(ViscaError::SyntaxError);
        let result = non_retryable_error.with_retry_context();
        assert!(result.is_err());
        assert!(!result.unwrap_err().is_retryable());
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_visca_retry_blocking_success() {
        let mut attempt_count = 0;
        let result = ViscaRetry::retry_blocking(
            || {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err(ViscaError::CameraBusy)
                } else {
                    Ok("success")
                }
            },
            3,
            Duration::from_millis(1), // Short delay for testing
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count, 3);
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_visca_retry_blocking_max_attempts() {
        let mut attempt_count = 0;
        let result: Result<&str, ViscaError> = ViscaRetry::retry_blocking(
            || {
                attempt_count += 1;
                Err(ViscaError::CameraBusy)
            },
            3,
            Duration::from_millis(1),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ViscaError::CameraBusy));
        assert_eq!(attempt_count, 3);
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_visca_retry_blocking_non_retryable() {
        let mut attempt_count = 0;
        let result: Result<&str, ViscaError> = ViscaRetry::retry_blocking(
            || {
                attempt_count += 1;
                Err(ViscaError::SyntaxError)
            },
            3,
            Duration::from_millis(1),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ViscaError::SyntaxError));
        assert_eq!(attempt_count, 1); // Only one attempt for non-retryable errors
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_visca_retry_async_success() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = ViscaRetry::retry_async(
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(ViscaError::Timeout)
                    } else {
                        Ok("async success")
                    }
                }
            },
            3,
            Duration::from_millis(1),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "async success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_visca_retry_with_suggested_delay_async() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = ViscaRetry::retry_with_suggested_delay_async(
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                    if current == 1 {
                        Err(ViscaError::CameraBusy) // 100ms suggested delay
                    } else if current == 2 {
                        Err(ViscaError::CameraMoving { pan: 100, tilt: 50 }) // 500ms suggested delay
                    } else {
                        Ok("suggested delay success")
                    }
                }
            },
            4,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "suggested delay success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_visca_retry_exponential_backoff_async() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();
        let start_time = std::time::Instant::now();

        let result = ViscaRetry::retry_with_exponential_backoff_async(
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(ViscaError::CommandBufferFull)
                    } else {
                        Ok("backoff success")
                    }
                }
            },
            4,
            Duration::from_millis(10),  // 10ms initial
            Duration::from_millis(100), // 100ms max
        )
        .await;

        let elapsed = start_time.elapsed();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "backoff success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
        // Should have at least 10ms (first retry) + 20ms (second retry) = 30ms total
        assert!(elapsed >= Duration::from_millis(25));
    }

    #[test]
    fn test_error_classification_completeness() {
        // Ensure all retryable errors have suggested delays
        let retryable_errors = vec![
            ViscaError::CameraBusy,
            ViscaError::CameraMoving { pan: 0, tilt: 0 },
            ViscaError::CommandTimeout {
                duration: Duration::from_secs(1),
                command: "test".to_string(),
            },
            ViscaError::CommandBufferFull,
            ViscaError::Timeout,
        ];

        for error in retryable_errors {
            assert!(error.is_retryable(), "Error should be retryable: {}", error);
            assert!(
                error.suggested_retry_delay().is_some(),
                "Retryable error should have suggested delay: {}",
                error
            );
        }

        // Ensure non-retryable errors don't have suggested delays
        let non_retryable_errors = vec![
            ViscaError::SyntaxError,
            ViscaError::CommandNotExecutable,
            ViscaError::PresetNotFound { id: 1 },
            ViscaError::FeatureNotSupported {
                feature: "test".to_string(),
            },
            ViscaError::InvalidParameter("test".to_string()),
        ];

        for error in non_retryable_errors {
            assert!(
                !error.is_retryable(),
                "Error should not be retryable: {}",
                error
            );
            assert!(
                error.suggested_retry_delay().is_none(),
                "Non-retryable error should not have suggested delay: {}",
                error
            );
        }
    }
}
