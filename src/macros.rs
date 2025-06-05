//! Declarative macros for reducing VISCA command boilerplate

/// Create a simple VISCA command enum with byte sequences
/// 
/// # Example
/// ```
/// use grafton_visca::visca_command;
/// 
/// visca_command! {
///     #[category = "Movement"]
///     enum PanTiltCommand {
///         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
///         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    (
        #[category = $category:literal]
        enum $name:ident {
            $(
                $variant:ident => [$($byte:expr),+ $(,)?]
            ),+ $(,)?
        }
    ) => {
        #[derive(Debug)]
        pub enum $name {
            $(
                $variant,
            )+
        }

        impl $crate::command::ViscaCommand for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::ViscaError> {
                match self {
                    $(
                        Self::$variant => Ok(vec![$($byte),+]),
                    )+
                }
            }

            fn response_type(&self) -> Option<$crate::command::ViscaResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };
}