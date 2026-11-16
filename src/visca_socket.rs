//! Checked identifiers for the two numbered VISCA command sockets.

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
