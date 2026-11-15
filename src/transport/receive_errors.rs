//! The OS evidence that proves a receive-side connection is unusable.

macro_rules! fatal_receive_io_kinds {
    ($($kind:ident => $reason:literal),+ $(,)?) => {
        #[doc = concat!(
            "Raw I/O errors that end a live session when reported by a failed read.\n\n",
            "| I/O error kind | Evidence |\n| --- | --- |\n",
            $("| `", stringify!($kind), "` | ", $reason, " |\n",)+
            "\nAn application-owned idle timer should return [`crate::Error::io_timeout`]. ",
            "`WouldBlock` and `Interrupted` also mean no input. Other raw I/O errors ",
            "are transient; for example, UDP `ConnectionRefused` does not prove session death."
        )]
        pub const FATAL_RECEIVE_IO_KINDS: &[std::io::ErrorKind] = &[
            $(std::io::ErrorKind::$kind,)+
        ];
    };
}

fatal_receive_io_kinds! {
    TimedOut => "OS connection timeout, including exhausted TCP keepalive",
    ConnectionReset => "Peer reset the connection",
    ConnectionAborted => "Connection was aborted",
    BrokenPipe => "Connection can no longer carry data",
    UnexpectedEof => "Connection ended unexpectedly",
    NotConnected => "Transport has no live connection",
    InvalidInput => "Validated transport cannot satisfy its live I/O contract",
}
