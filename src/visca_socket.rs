//! Checked identifiers for the two numbered VISCA command sockets.

/// Number of physical command sockets available on a VISCA target.
///
/// Construction and deserialization accept only `1` or `2`. Profile tuning may
/// lower this capacity but cannot exceed the selected profile's limit.
///
/// ```
/// use grafton_visca::types::CommandSocketCount;
///
/// assert_eq!(CommandSocketCount::ONE.value(), 1);
/// assert_eq!(CommandSocketCount::new(2)?, CommandSocketCount::TWO);
/// assert!(CommandSocketCount::new(0).is_err());
/// # Ok::<(), grafton_visca::Error>(())
/// ```
#[crate::value(min = 1, max = 2)]
pub struct CommandSocketCount(u8);

impl CommandSocketCount {
    /// One physical command socket.
    pub const ONE: Self = Self(1);

    /// Two physical command sockets.
    pub const TWO: Self = Self(2);

    #[cfg(test)]
    pub(crate) fn sockets(self) -> impl Iterator<Item = ViscaSocket> {
        [ViscaSocket::S1, ViscaSocket::S2]
            .into_iter()
            .take(self.get())
    }

    #[cfg(any(feature = "blocking", feature = "async", test))]
    pub(crate) fn get(self) -> usize {
        usize::from(self.value())
    }
}

/// One numbered VISCA command-execution slot.
///
/// The canonical wire values are `1` and `2`. A protocol response with socket
/// nibble `0` has no numbered socket and is represented by `Option::None`.
/// Use [`crate::wire::WireValue`] for wire conversion; array indices remain
/// zero-based through [`Self::as_index`].
#[crate::value_enum]
#[derive(Default)]
pub enum ViscaSocket {
    /// First command socket, with protocol number `1`.
    #[default]
    #[value(display = "Socket1")]
    S1 = 1,
    /// Second command socket, with protocol number `2`.
    #[value(display = "Socket2")]
    S2 = 2,
}

impl ViscaSocket {
    /// Returns the zero-based array index, `0` or `1`.
    #[must_use]
    pub const fn as_index(self) -> usize {
        match self {
            Self::S1 => 0,
            Self::S2 => 1,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::wire::WireValue;

    #[test]
    fn command_socket_counts_preserve_the_checked_physical_domain() {
        assert_eq!(CommandSocketCount::MIN, CommandSocketCount::ONE);
        assert_eq!(CommandSocketCount::MAX, CommandSocketCount::TWO);
        for value in 0..=u8::MAX {
            assert_eq!(
                CommandSocketCount::new(value).is_ok(),
                (1..=2).contains(&value)
            );
            if let Ok(count) = CommandSocketCount::try_from(value) {
                assert_eq!(u8::from(count), value);
                assert_eq!(count.get(), usize::from(value));
                assert_eq!(count.sockets().count(), usize::from(value));
            }
        }
        assert_eq!(
            CommandSocketCount::ONE.sockets().collect::<Vec<_>>(),
            [ViscaSocket::S1]
        );
        assert_eq!(
            CommandSocketCount::TWO.sockets().collect::<Vec<_>>(),
            [ViscaSocket::S1, ViscaSocket::S2]
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn command_socket_count_persistence_is_numeric_and_checked(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for count in [CommandSocketCount::ONE, CommandSocketCount::TWO] {
            let value = serde_json::to_value(count)?;
            assert_eq!(value, serde_json::json!(count.value()));
            assert_eq!(serde_json::from_value::<CommandSocketCount>(value)?, count);
        }
        for value in [0, 3, 255] {
            assert!(
                serde_json::from_value::<CommandSocketCount>(serde_json::json!(value)).is_err()
            );
        }
        Ok(())
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn command_socket_count_schema_and_typescript_use_the_numeric_domain() {
        let schema = schemars::schema_for!(CommandSocketCount);
        assert_eq!(schema.get("type").unwrap(), "integer");
        assert_eq!(schema.get("minimum").unwrap(), 1);
        assert_eq!(schema.get("maximum").unwrap(), 2);
        assert_eq!(
            <CommandSocketCount as ts_rs::TS>::decl(&ts_rs::Config::default()),
            "type CommandSocketCount = number;"
        );
    }

    #[test]
    fn canonical_wire_domain_excludes_socketless_and_cancel_bytes() {
        let accepted: Vec<_> = (0..=u8::MAX)
            .filter_map(|byte| {
                ViscaSocket::from_wire(byte)
                    .ok()
                    .map(|socket| (byte, socket))
            })
            .collect();
        assert_eq!(accepted, [(1, ViscaSocket::S1), (2, ViscaSocket::S2)]);
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            assert_eq!(ViscaSocket::from_wire(socket.to_wire()).unwrap(), socket);
        }
    }

    #[test]
    fn array_indices_are_distinct_from_protocol_numbers() {
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
        assert_eq!(ViscaSocket::S1.to_wire(), 1);
        assert_eq!(ViscaSocket::S2.to_wire(), 2);
        assert!(ViscaSocket::S1 < ViscaSocket::S2);
    }

    #[test]
    fn default_and_display_keep_the_numbered_socket_meaning() {
        assert_eq!(ViscaSocket::default(), ViscaSocket::S1);
        assert_eq!(ViscaSocket::S1.to_string(), "Socket1");
        assert_eq!(ViscaSocket::S2.to_string(), "Socket2");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_is_the_snake_case_value_enum_not_a_second_wire_codec() {
        for (socket, name) in [(ViscaSocket::S1, "s1"), (ViscaSocket::S2, "s2")] {
            let json = serde_json::json!(name);
            assert_eq!(serde_json::to_value(socket).unwrap(), json);
            assert_eq!(serde_json::from_value::<ViscaSocket>(json).unwrap(), socket);
        }
        assert!(serde_json::from_value::<ViscaSocket>(serde_json::json!("S1")).is_err());
        assert!(serde_json::from_value::<ViscaSocket>(serde_json::json!(1)).is_err());
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn schema_and_typescript_share_the_serialized_socket_domain() {
        let schema = schemars::schema_for!(ViscaSocket);
        let values = schema
            .get("enum")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_else(|| {
                schema
                    .get("oneOf")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|variant| variant["const"].clone())
                    .collect()
            });
        assert_eq!(
            values,
            serde_json::json!(["s1", "s2"]).as_array().unwrap().to_vec()
        );
        assert_eq!(
            <ViscaSocket as ts_rs::TS>::decl(&ts_rs::Config::default()),
            "type ViscaSocket = \"s1\" | \"s2\";"
        );
    }
}
