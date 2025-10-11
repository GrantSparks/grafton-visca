//! Runtime-agnostic command options for timeout and cancellation support.
//!
//! This module provides a runtime-neutral approach to command timeouts and cancellation,
//! allowing users to control operation behavior without tying to a specific async runtime.

use core::{future::Future, pin::Pin, time::Duration};

/// Trait for runtime-agnostic cancellation tokens.
///
/// This trait allows different runtime implementations to provide their own
/// cancellation mechanisms while maintaining a common interface.
///
/// # Example Implementation
///
/// ```rust,ignore
/// use grafton_visca::CancellationToken;
/// use core::future::Future;
/// use core::pin::Pin;
///
/// /// A no-op cancellation token that never cancels
/// #[derive(Default, Clone, Copy)]
/// pub struct NoCancel;
///
/// impl CancellationToken for NoCancel {
///     fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
///         Box::pin(async move {
///             loop {
///                 core::future::poll_fn(|_| core::task::Poll::Pending::<()>).await;
///             }
///         })
///     }
/// }
/// ```
pub trait CancellationToken: Send + Sync {
    /// Returns a future that resolves when cancellation is requested.
    ///
    /// This future will:
    /// - Never resolve if cancellation is not requested
    /// - Resolve immediately if cancellation has already been requested
    /// - Resolve when cancellation is requested after this call
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

/// A no-op cancellation token that never triggers cancellation.
///
/// This is the default cancellation token used when no cancellation is needed.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoCancel;

impl CancellationToken for NoCancel {
    fn cancelled<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        // Create a future that never completes using async block
        Box::pin(async move {
            // This will never complete, representing no cancellation
            loop {
                // Use a yield point to avoid blocking
                core::future::poll_fn(|_| core::task::Poll::Pending::<()>).await;
            }
        })
    }
}

/// Runtime-agnostic command options for controlling operation behavior.
///
/// Provides timeout and cancellation support without depending on a specific
/// async runtime. This enables consistent behavior across tokio, async-std,
/// smol, and other executors.
///
/// # Examples
///
/// ## Basic usage with timeout
/// ```rust,ignore
/// use grafton_visca::CommandOptions;
/// use core::time::Duration;
///
/// // Create options with a 500ms timeout
/// let opts = CommandOptions::default()
///     .with_timeout(Duration::from_millis(500));
///
/// // Use with camera commands
/// camera.zoom_tele(None, opts).await?;
/// ```
///
/// ## Using cancellation tokens
/// ```rust,ignore
/// use grafton_visca::{CommandOptions, NoCancel};
///
/// // With no cancellation (default)
/// let opts = CommandOptions::<NoCancel>::default();
///
/// // With custom cancellation token
/// let token = MyCustomToken;
/// let opts = CommandOptions::default()
///     .with_cancel(&token);
/// ```
#[derive(Debug)]
pub struct CommandOptions<'a, C: CancellationToken = NoCancel> {
    /// Optional timeout duration for the command.
    pub timeout: Option<Duration>,
    /// Optional cancellation token for the command.
    pub cancel: Option<&'a C>,
}

// Implement Clone manually to avoid requiring C: Clone
impl<'a, C: CancellationToken> Clone for CommandOptions<'a, C> {
    fn clone(&self) -> Self {
        *self
    }
}

// Also implement Copy when possible since we're just copying references
impl<'a, C: CancellationToken> Copy for CommandOptions<'a, C> {}

impl<'a> Default for CommandOptions<'a, NoCancel> {
    fn default() -> Self {
        Self {
            timeout: None,
            cancel: None,
        }
    }
}

impl<'a, C: CancellationToken> CommandOptions<'a, C> {
    /// Create new command options with defaults.
    pub fn new() -> CommandOptions<'a, NoCancel> {
        CommandOptions::default()
    }

    /// Set a timeout duration for the command.
    ///
    /// # Arguments
    /// * `timeout` - The maximum duration to wait for the command to complete
    ///
    /// # Examples
    /// ```rust
    /// use grafton_visca::CommandOptions;
    /// use core::time::Duration;
    ///
    /// let opts = CommandOptions::default()
    ///     .with_timeout(Duration::from_secs(1));
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set a timeout in milliseconds for convenience.
    ///
    /// # Arguments
    /// * `millis` - The timeout duration in milliseconds
    ///
    /// # Examples
    /// ```rust
    /// use grafton_visca::CommandOptions;
    ///
    /// let opts = CommandOptions::default()
    ///     .with_timeout_ms(250);
    /// ```
    pub fn with_timeout_ms(self, millis: u64) -> Self {
        self.with_timeout(Duration::from_millis(millis))
    }

    /// Set a cancellation token for the command.
    ///
    /// # Arguments
    /// * `cancel` - Reference to a cancellation token
    ///
    /// # Examples
    /// ```rust
    /// use grafton_visca::{CommandOptions, NoCancel};
    ///
    /// let token = NoCancel;
    /// let opts = CommandOptions::default()
    ///     .with_cancel(&token);
    /// ```
    pub fn with_cancel<T: CancellationToken>(self, cancel: &'a T) -> CommandOptions<'a, T> {
        CommandOptions {
            timeout: self.timeout,
            cancel: Some(cancel),
        }
    }
}

/// Convenience type for commands that don't need options.
///
/// This unit type implements `Into<CommandOptions>` to allow
/// clean API usage when options aren't needed.
///
/// # Examples
/// ```rust,ignore
/// use grafton_visca::{CommandOptions, NoOpts};
///
/// # async fn example(camera: &impl grafton_visca::ZoomControl) -> Result<(), grafton_visca::Error> {
/// // These are equivalent (commands now only take one argument):
/// camera.zoom_tele(None).await?;
/// camera.zoom_stop().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpts;

impl<'a> From<NoOpts> for CommandOptions<'a, NoCancel> {
    fn from(_: NoOpts) -> Self {
        CommandOptions::default()
    }
}

impl<'a, C: CancellationToken> From<&'a CommandOptions<'a, C>> for CommandOptions<'a, C> {
    fn from(opts: &'a CommandOptions<'a, C>) -> Self {
        *opts
    }
}

// Allow creating CommandOptions from Duration directly for timeout-only cases
impl<'a> From<Duration> for CommandOptions<'a, NoCancel> {
    fn from(timeout: Duration) -> Self {
        CommandOptions::default().with_timeout(timeout)
    }
}

// Allow creating CommandOptions from Option<Duration> for optional timeout cases
impl<'a> From<Option<Duration>> for CommandOptions<'a, NoCancel> {
    fn from(timeout: Option<Duration>) -> Self {
        match timeout {
            Some(d) => CommandOptions::default().with_timeout(d),
            None => CommandOptions::default(),
        }
    }
}
