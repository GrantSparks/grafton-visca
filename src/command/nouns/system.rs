//! Camera identity and nonvolatile-settings requests.

use crate::command::nouns::{forward_inquiry, forward_request};

/// Reads the documented camera vendor, model, ROM revision and socket limit.
#[derive(Debug, Clone, Copy)]
pub struct VersionInquiry;
forward_inquiry!(VersionInquiry, crate::command::VersionInquiry; crate::command::VersionInquiry);

/// Saves current settings to nonvolatile camera storage.
#[derive(Debug, Clone, Copy)]
pub struct SaveSettings;
forward_request!(SaveSettings, crate::command::SettingsSaveCommand, Plain, rows = [SettingsSave]; crate::command::SettingsSaveCommand::new());
