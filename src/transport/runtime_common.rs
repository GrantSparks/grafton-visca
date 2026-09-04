//! Internal macro for generating runtime-specific TCP/UDP transport wrappers.
//!
//! This module provides a declarative macro that eliminates duplication across
//! runtime-specific transport implementations. The macro generates identical
//! connection methods and split logic for each runtime (tokio, smol)
//! while maintaining the existing public API and zero-cost abstractions.

/// Generate runtime-specific TCP and UDP transport implementations.
///
/// This macro eliminates code duplication by generating the connection methods
/// and split logic for TCP/UDP transports across different async runtimes.
///
/// # Parameters
///
/// - `runtime`: The runtime name (e.g., "tokio", "smol")
/// - `tcp_stream`: The runtime-specific TCP stream type
/// - `udp_socket`: The runtime-specific UDP socket type
/// - `tcp_connect`: The runtime-specific TCP connector function
/// - `udp_connect`: The runtime-specific UDP connector function
/// - `tcp_split`: The split strategy (either "owned" for tokio or "clone" for others)
/// - `tcp_reader_ty`: Type for TcpReader field
/// - `tcp_writer_ty`: Type for TcpWriter field
///
/// # Example Usage
///
/// ```ignore
/// declare_net_transport!(
///     runtime = "tokio",
///     tcp_stream = TokioTcpStream,
///     udp_socket = tokio::net::UdpSocket,
///     tcp_connect = connect_tcp,
///     udp_connect = connect_udp,
///     tcp_split = owned {
///         reader: TokioBufferedReader<tokio::net::tcp::OwnedReadHalf>,
///         writer: TokioWriter<tokio::net::tcp::OwnedWriteHalf>
///     }
/// );
/// ```
#[macro_export]
macro_rules! declare_net_transport {
    (
        runtime = $runtime:literal,
        tcp_stream = $tcp_stream:ty,
        udp_socket = $udp_socket:ty,
        tcp_connect = $tcp_connect:path,
        udp_connect = $udp_connect:path,
        tcp_split = owned {
            reader: $reader_ty:ty,
            writer: $writer_ty:ty
        }
    ) => {
        // TCP module
        pub mod tcp {
            #![doc = concat!("TCP transport implementation using ", $runtime, ".")]

            use std::time::Duration;

            use $crate::{
                transport::{
                    async_io::{write_all_flush, AsyncReadExt},
                    socket_options::TcpConnectionConfig,
                    buffer::BufferConfig,
                    builder::TransportConfig,
                },
                Error,
            };

            #[doc = concat!("TCP transport for async VISCA communication using ", $runtime, ".")]
            #[doc = ""]
            #[doc = concat!("This is a type alias for the generic TCP transport specialized for ", $runtime, "'s TCP stream.")]
            pub type Tcp = $crate::transport::async_tcp::Tcp<$tcp_stream>;

            /// Helper methods for creating TCP transports.
            impl Tcp {
                /// Connect to a TCP endpoint.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                pub async fn connect(address: &str) -> Result<Self, Error> {
                    let config = TransportConfig {
                        buffer_config: BufferConfig::for_raw_ip(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a custom timeout.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
                    let config = TransportConfig {
                        connect_timeout: timeout,
                        buffer_config: BufferConfig::for_raw_ip(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a full configuration.
                ///
                /// This method provides full control over connection and socket parameters.
                pub async fn connect_with_config(
                    address: &str,
                    config: TransportConfig,
                ) -> Result<Self, Error> {
                    config.validate()?;
                    let tcp_config = TcpConnectionConfig::from(config);
                    let canonical_addr =
                        $crate::transport::address::canonicalize_endpoint(address, None)?;
                    let stream = $tcp_connect(&canonical_addr, tcp_config).await?;

                    Ok(Self::new(stream, config))
                }

                /// Split the TCP transport into separate reader and writer halves.
                ///
                /// This allows for concurrent reading and writing without needing mutable
                /// access to the entire transport. Useful for full-duplex communication patterns.
                #[doc = ""]
                #[doc = "# Example"]
                #[doc = "```no_run"]
                #[doc = concat!("# use grafton_visca::runtime_adapters::", $runtime, "::TcpTransport as Tcp;")]
                #[doc = "# async fn example() -> Result<(), Box<dyn std::error::Error>> {"]
                #[doc = "let transport = Tcp::connect(\"192.168.1.100:5678\").await?;"]
                #[doc = "let (mut reader, mut writer) = transport.split();"]
                #[doc = ""]
                #[doc = "// Can now read and write concurrently"]
                #[doc = "# Ok(())"]
                #[doc = "# }"]
                #[doc = "```"]
                pub fn split(self) -> (TcpReader, TcpWriter) {
                    let stream_wrapper = self.stream;

                    let tcp_reader = TcpReader {
                        reader: stream_wrapper.reader,
                    };
                    let tcp_writer = TcpWriter {
                        writer: stream_wrapper.writer,
                    };

                    (tcp_reader, tcp_writer)
                }
            }

            /// Reader half of a split TCP transport.
            ///
            /// This type allows reading from an async TCP connection that has been split
            /// into separate reader and writer halves.
            #[derive(Debug)]
            pub struct TcpReader {
                reader: $reader_ty,
            }

            impl TcpReader {
                /// Receive data from the TCP connection into the provided buffer.
                ///
                /// Returns the number of bytes read. Returns an error on connection close.
                pub async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
                    // Read chunk of data directly into the provided buffer
                    let n = self.reader.read(dst).await?;

                    if n == 0 {
                        return Err(Error::ConnectionClosed {
                            reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
                        });
                    }

                    Ok(n)
                }
            }

            /// Writer half of a split TCP transport.
            ///
            /// This type allows writing to an async TCP connection that has been split
            /// into separate reader and writer halves.
            #[derive(Debug)]
            pub struct TcpWriter {
                writer: $writer_ty,
            }

            impl TcpWriter {
                /// Send data over the TCP connection.
                pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
                    write_all_flush(&mut self.writer, data).await
                }
            }
        }

        // UDP module
        pub mod udp {
            #![doc = concat!("UDP transport implementation using ", $runtime, ".")]

            use $crate::{
                transport::{
                    socket_options::UdpSocketConfig,
                    buffer::BufferConfig,
                    builder::TransportConfig,
                },
                Error,
            };

            #[doc = concat!("UDP transport for async VISCA communication using ", $runtime, ".")]
            #[doc = ""]
            #[doc = concat!("This is a type alias for the generic UDP transport specialized for ", $runtime, "'s UdpSocket.")]
            pub type Udp = $crate::transport::async_udp::Udp<$udp_socket>;

            /// Helper methods for creating UDP transports.
            impl Udp {
                /// Connect to a UDP endpoint.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                /// The socket will bind to the appropriate unspecified address based on the
                /// target address family.
                pub async fn connect(address: &str) -> Result<Self, Error> {
                    let config = TransportConfig {
                        buffer_config: BufferConfig::for_udp(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a full configuration.
                ///
                /// This method provides full control over connection and socket parameters.
                pub async fn connect_with_config(
                    address: &str,
                    config: TransportConfig,
                ) -> Result<Self, Error> {
                    let socket = Self::preflight_udp_setup(
                        address,
                        config,
                        |canonical_addr, udp_config| async move {
                            $udp_connect(&canonical_addr, udp_config).await
                        },
                    )
                    .await?;

                    Ok(Self::new(socket, config))
                }

                /// Run endpoint parsing and connector setup only after buffer preflight.
                pub(super) async fn preflight_udp_setup<T, F, Fut>(
                    address: &str,
                    config: TransportConfig,
                    setup: F,
                ) -> Result<T, Error>
                where
                    F: FnOnce(String, UdpSocketConfig) -> Fut,
                    Fut: std::future::Future<Output = Result<T, Error>>,
                {
                    config.validate()?;
                    let udp_config = UdpSocketConfig::from(config);
                    let canonical_addr =
                        $crate::transport::address::canonicalize_endpoint(address, None)?;
                    setup(canonical_addr, udp_config).await
                }
            }
        }
    };

    (
        runtime = $runtime:literal,
        tcp_stream = $tcp_stream:ty,
        udp_socket = $udp_socket:ty,
        tcp_connect = $tcp_connect:path,
        udp_connect = $udp_connect:path,
        tcp_split = clone
    ) => {
        // TCP module
        pub mod tcp {
            #![doc = concat!("TCP transport implementation using ", $runtime, ".")]

            use std::time::Duration;

            use $crate::{
                transport::{
                    async_io::{write_all_flush, AsyncReadExt},
                    socket_options::TcpConnectionConfig,
                    buffer::BufferConfig,
                    builder::TransportConfig,
                },
                Error,
            };

            #[doc = concat!("TCP transport for async VISCA communication using ", $runtime, ".")]
            #[doc = ""]
            #[doc = concat!("This is a type alias for the generic TCP transport specialized for ", $runtime, "'s TCP stream.")]
            pub type Tcp = $crate::transport::async_tcp::Tcp<$tcp_stream>;

            /// Helper methods for creating TCP transports.
            impl Tcp {
                /// Connect to a TCP endpoint.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                pub async fn connect(address: &str) -> Result<Self, Error> {
                    let config = TransportConfig {
                        buffer_config: BufferConfig::for_raw_ip(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a custom timeout.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                pub async fn connect_timeout(address: &str, timeout: Duration) -> Result<Self, Error> {
                    let config = TransportConfig {
                        connect_timeout: timeout,
                        buffer_config: BufferConfig::for_raw_ip(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a full configuration.
                ///
                /// This method provides full control over connection and socket parameters.
                pub async fn connect_with_config(
                    address: &str,
                    config: TransportConfig,
                ) -> Result<Self, Error> {
                    config.validate()?;
                    let tcp_config = TcpConnectionConfig::from(config);
                    let canonical_addr =
                        $crate::transport::address::canonicalize_endpoint(address, None)?;
                    let stream = $tcp_connect(&canonical_addr, tcp_config).await?;
                    Ok(Self::new(stream, config))
                }

                /// Split the TCP transport into separate reader and writer halves.
                ///
                /// This allows for concurrent reading and writing without needing mutable
                /// access to the entire transport. Useful for full-duplex communication patterns.
                #[doc = ""]
                #[doc = "# Example"]
                #[doc = "```no_run"]
                #[doc = concat!("# use grafton_visca::runtime_adapters::", $runtime, "::TcpTransport as Tcp;")]
                #[doc = "# async fn example() -> Result<(), Box<dyn std::error::Error>> {"]
                #[doc = "let transport = Tcp::connect(\"192.168.1.100:5678\").await?;"]
                #[doc = "let (mut reader, mut writer) = transport.split();"]
                #[doc = ""]
                #[doc = "// Can now read and write concurrently"]
                #[doc = "# Ok(())"]
                #[doc = "# }"]
                #[doc = "```"]
                pub fn split(self) -> (TcpReader, TcpWriter) {
                    let cloned_stream = <$tcp_stream>::new(self.stream.clone_stream());

                    let reader = TcpReader {
                        stream: self.stream,
                    };

                    let writer = TcpWriter {
                        stream: cloned_stream,
                    };

                    (reader, writer)
                }
            }

            /// Reader half of a split TCP transport.
            ///
            /// This type allows reading from an async TCP connection that has been split
            /// into separate reader and writer halves.
            #[derive(Debug)]
            pub struct TcpReader {
                stream: $tcp_stream,
            }

            impl TcpReader {
                /// Receive data from the TCP connection into the provided buffer.
                ///
                /// Returns the number of bytes read. Returns an error on connection close.
                pub async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
                    // Read chunk of data directly into the provided buffer
                    let n = self.stream.read(dst).await?;

                    if n == 0 {
                        return Err(Error::ConnectionClosed {
                            reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
                        });
                    }

                    Ok(n)
                }
            }

            /// Writer half of a split TCP transport.
            ///
            /// This type allows writing to an async TCP connection that has been split
            /// into separate reader and writer halves.
            #[derive(Debug)]
            pub struct TcpWriter {
                stream: $tcp_stream,
            }

            impl TcpWriter {
                /// Send data over the TCP connection.
                pub async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
                    write_all_flush(&mut self.stream, data).await
                }
            }
        }

        // UDP module
        pub mod udp {
            #![doc = concat!("UDP transport implementation using ", $runtime, ".")]

            use $crate::{
                transport::{
                    socket_options::UdpSocketConfig,
                    buffer::BufferConfig,
                    builder::TransportConfig,
                },
                Error,
            };

            #[doc = concat!("UDP transport for async VISCA communication using ", $runtime, ".")]
            #[doc = ""]
            #[doc = concat!("This is a type alias for the generic UDP transport specialized for ", $runtime, "'s UdpSocket.")]
            pub type Udp = $crate::transport::async_udp::Udp<$udp_socket>;

            /// Helper methods for creating UDP transports.
            impl Udp {
                /// Connect to a UDP endpoint.
                ///
                /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
                /// The socket will bind to the appropriate unspecified address based on the
                /// target address family.
                pub async fn connect(address: &str) -> Result<Self, Error> {
                    let config = TransportConfig {
                        buffer_config: BufferConfig::for_udp(),
                        ..Default::default()
                    };
                    Self::connect_with_config(address, config).await
                }

                /// Connect with a full configuration.
                ///
                /// This method provides full control over connection and socket parameters.
                pub async fn connect_with_config(
                    address: &str,
                    config: TransportConfig,
                ) -> Result<Self, Error> {
                    let socket = Self::preflight_udp_setup(
                        address,
                        config,
                        |canonical_addr, udp_config| async move {
                            $udp_connect(&canonical_addr, udp_config).await
                        },
                    )
                    .await?;

                    Ok(Self::new(socket, config))
                }

                /// Run endpoint parsing and connector setup only after buffer preflight.
                pub(super) async fn preflight_udp_setup<T, F, Fut>(
                    address: &str,
                    config: TransportConfig,
                    setup: F,
                ) -> Result<T, Error>
                where
                    F: FnOnce(String, UdpSocketConfig) -> Fut,
                    Fut: std::future::Future<Output = Result<T, Error>>,
                {
                    config.validate()?;
                    let udp_config = UdpSocketConfig::from(config);
                    let canonical_addr =
                        $crate::transport::address::canonicalize_endpoint(address, None)?;
                    setup(canonical_addr, udp_config).await
                }
            }
        }
    };
}
