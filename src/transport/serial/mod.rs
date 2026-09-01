//! Serial transport module for unified configuration and transport implementations.

mod config;
pub(crate) mod handshake;

pub use config::Config;

/// One optional operation performed while bringing up a serial VISCA bus.
///
/// Address assignment must precede I/F Clear when both are requested: the
/// clear resets the command interface after the bus has been addressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupOperation {
    AddressSet,
    InterfaceClear,
}

/// Return the serial startup operations in their protocol-required order.
///
/// Keeping this small plan shared makes blocking and Tokio connection paths
/// execute the same sequence while preserving the single-operation cases.
pub(crate) fn startup_plan(config: &Config) -> [Option<StartupOperation>; 2] {
    [
        config
            .address_set_on_connect
            .then_some(StartupOperation::AddressSet),
        config
            .if_clear_on_connect
            .then_some(StartupOperation::InterfaceClear),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_plan_preserves_requested_operations_in_protocol_order() {
        let neither = Config::default()
            .address_set_on_connect(false)
            .if_clear_on_connect(false);
        assert_eq!(startup_plan(&neither), [None, None]);

        let address_only = neither.clone().address_set_on_connect(true);
        assert_eq!(
            startup_plan(&address_only),
            [Some(StartupOperation::AddressSet), None]
        );

        let clear_only = neither.clone().if_clear_on_connect(true);
        assert_eq!(
            startup_plan(&clear_only),
            [None, Some(StartupOperation::InterfaceClear)]
        );

        let both = address_only.if_clear_on_connect(true);
        assert_eq!(
            startup_plan(&both),
            [
                Some(StartupOperation::AddressSet),
                Some(StartupOperation::InterfaceClear),
            ]
        );
    }
}
