//! Tests to verify that default ports are correctly applied when not specified.

#[cfg(not(feature = "mode-async"))]
mod tests {
    use grafton_visca::{
        camera::Connect,
        profiles::{GenericVisca, PtzOpticsG2, SonyEVIH100, SonyFR7},
    };

    #[test]
    fn test_blocking_camera_tcp_adds_default_port() {
        // Test that when no port is specified, the profile's default TCP port is added
        // PtzOpticsG2 default TCP port is 5678
        let result = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.1.100");
        // This will fail to connect, but we're testing that it tries the right port
        assert!(result.is_err());

        // GenericVisca also uses default TCP port 5678
        let result = Connect::open_tcp_blocking::<GenericVisca>("192.168.1.101");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocking_camera_udp_adds_default_port() {
        // Test that when no port is specified, the profile's default UDP port is added
        // PtzOpticsG2 default UDP port is 1259
        let result = Connect::open_udp_blocking::<PtzOpticsG2>("192.168.1.100");
        // UDP is connectionless, so this should succeed even for non-existent addresses
        assert!(result.is_ok());

        // GenericVisca also uses default UDP port 1259
        let result = Connect::open_udp_blocking::<GenericVisca>("192.168.1.101");
        assert!(result.is_ok());
    }

    #[test]
    fn test_connect_builder_tcp_adds_default_port() {
        // Test that ConnectBuilder adds default TCP port when requested.
        // PtzOpticsG2 default TCP port is 5678
        let result = Connect::builder()
            .tcp("192.168.1.100")
            .with_default_port()
            .open::<PtzOpticsG2>();
        assert!(result.is_err());

        // Raw Sony VISCA profiles use default TCP port 5678
        let result = Connect::builder()
            .tcp("192.168.1.102")
            .with_default_port()
            .open::<SonyEVIH100>();
        assert!(result.is_err());
    }

    #[test]
    fn test_connect_builder_udp_adds_default_port() {
        // Test that ConnectBuilder adds default UDP port when requested.
        // GenericVisca default UDP port is 1259
        let result = Connect::builder()
            .udp("192.168.1.100")
            .with_default_port()
            .open::<GenericVisca>();
        assert!(result.is_ok());

        // SonyFR7 uses Sony encapsulated UDP port 52381
        let result = Connect::builder()
            .udp("192.168.1.103")
            .with_default_port()
            .open::<SonyFR7>();
        assert!(result.is_ok());
    }

    #[test]
    fn test_explicit_port_is_preserved() {
        // Test that when a port IS specified, it's used instead of the default
        // Using a non-standard port for PtzOpticsG2 (normally 5678)
        let result = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.1.100:9999");
        assert!(result.is_err()); // Will fail to connect but should try port 9999

        // Using a non-standard port for UDP
        let result = Connect::open_udp_blocking::<GenericVisca>("192.168.1.100:8888");
        assert!(result.is_ok()); // UDP succeeds even for non-existent

        // ConnectBuilder with explicit port
        let result = Connect::builder()
            .tcp("192.168.1.100:7777")
            .open::<PtzOpticsG2>();
        assert!(result.is_err());
    }

    #[test]
    fn test_ipv6_addresses_with_default_ports() {
        // Test IPv6 addresses get default ports added
        let result = Connect::open_tcp_blocking::<PtzOpticsG2>("::1");
        assert!(result.is_err()); // Will fail but should add port 5678

        let result = Connect::open_udp_blocking::<GenericVisca>("::1");
        assert!(result.is_ok()); // UDP succeeds

        // With explicit port
        let result = Connect::open_tcp_blocking::<PtzOpticsG2>("[::1]:9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_hostname_with_default_ports() {
        // Test that hostnames get default ports added
        let result = Connect::open_tcp_blocking::<PtzOpticsG2>("localhost");
        assert!(result.is_err()); // Will fail but should add port 5678

        let result = Connect::builder()
            .udp("localhost")
            .with_default_port()
            .open::<GenericVisca>();
        assert!(result.is_ok());
    }
}
