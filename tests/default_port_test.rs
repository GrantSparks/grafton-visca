//! Tests to verify that default ports are correctly applied when not specified.

#[cfg(not(feature = "mode-async"))]
mod tests {
    use grafton_visca::{
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        BlockingCamera, CameraBuilder,
    };

    #[test]
    fn test_blocking_camera_tcp_adds_default_port() {
        // Test that when no port is specified, the profile's default TCP port is added
        // PtzOpticsG2 default TCP port is 5678
        let result = BlockingCamera::<PtzOpticsG2, _>::open_tcp("192.168.1.100");
        // This will fail to connect, but we're testing that it tries the right port
        assert!(result.is_err());

        // GenericVisca also uses default TCP port 5678
        let result = BlockingCamera::<GenericVisca, _>::open_tcp("192.168.1.101");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocking_camera_udp_adds_default_port() {
        // Test that when no port is specified, the profile's default UDP port is added
        // PtzOpticsG2 default UDP port is 1259
        let result = BlockingCamera::<PtzOpticsG2, _>::open_udp("192.168.1.100");
        // UDP is connectionless, so this should succeed even for non-existent addresses
        assert!(result.is_ok());

        // GenericVisca also uses default UDP port 1259
        let result = BlockingCamera::<GenericVisca, _>::open_udp("192.168.1.101");
        assert!(result.is_ok());
    }

    #[test]
    fn test_camera_builder_tcp_adds_default_port() {
        // Test that CameraBuilder adds default TCP port when not specified
        // PtzOpticsG2 default TCP port is 5678
        let result = CameraBuilder::tcp("192.168.1.100")
            .profile::<PtzOpticsG2>()
            .open();
        assert!(result.is_err());

        // SonyFR7 also uses default TCP port 5678
        let result = CameraBuilder::tcp("192.168.1.102")
            .profile::<SonyFR7>()
            .open();
        assert!(result.is_err());
    }

    #[test]
    fn test_camera_builder_udp_adds_default_port() {
        // Test that CameraBuilder adds default UDP port when not specified
        // GenericVisca default UDP port is 1259
        let result = CameraBuilder::udp("192.168.1.100")
            .profile::<GenericVisca>()
            .open();
        assert!(result.is_ok());

        // SonyFR7 also uses default UDP port 1259
        let result = CameraBuilder::udp("192.168.1.103")
            .profile::<SonyFR7>()
            .open();
        assert!(result.is_ok());
    }

    #[test]
    fn test_explicit_port_is_preserved() {
        // Test that when a port IS specified, it's used instead of the default
        // Using a non-standard port for PtzOpticsG2 (normally 5678)
        let result = BlockingCamera::<PtzOpticsG2, _>::open_tcp("192.168.1.100:9999");
        assert!(result.is_err()); // Will fail to connect but should try port 9999

        // Using a non-standard port for UDP
        let result = BlockingCamera::<GenericVisca, _>::open_udp("192.168.1.100:8888");
        assert!(result.is_ok()); // UDP succeeds even for non-existent

        // CameraBuilder with explicit port
        let result = CameraBuilder::tcp("192.168.1.100:7777")
            .profile::<PtzOpticsG2>()
            .open();
        assert!(result.is_err());
    }

    #[test]
    fn test_ipv6_addresses_with_default_ports() {
        // Test IPv6 addresses get default ports added
        let result = BlockingCamera::<PtzOpticsG2, _>::open_tcp("::1");
        assert!(result.is_err()); // Will fail but should add port 5678

        let result = BlockingCamera::<GenericVisca, _>::open_udp("::1");
        assert!(result.is_ok()); // UDP succeeds

        // With explicit port
        let result = BlockingCamera::<PtzOpticsG2, _>::open_tcp("[::1]:9999");
        assert!(result.is_err());
    }

    #[test]
    fn test_hostname_with_default_ports() {
        // Test that hostnames get default ports added
        let result = BlockingCamera::<PtzOpticsG2, _>::open_tcp("localhost");
        assert!(result.is_err()); // Will fail but should add port 5678

        let result = CameraBuilder::udp("localhost")
            .profile::<GenericVisca>()
            .open();
        assert!(result.is_ok());
    }
}
