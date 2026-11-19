//! On-screen menu visibility, navigation and button requests.

use crate::command::nouns::{forward_inquiry, forward_request};
use crate::types::{DirectMenuTarget, MenuButton, MenuDirection};

/// Reads whether the on-screen menu is visible.
#[derive(Debug, Clone, Copy)]
pub struct VisibleInquiry;
forward_inquiry!(VisibleInquiry, crate::command::MenuOpenCloseInquiry; crate::command::MenuOpenCloseInquiry);

/// Sets menu visibility and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct SetVisible {
    inner: crate::command::SetMenuDisplay,
}

impl SetVisible {
    /// Creates a menu-visibility setting.
    #[must_use]
    pub fn new(visible: bool) -> Self {
        Self {
            inner: crate::command::SetMenuDisplay::new(visible),
        }
    }

    /// Returns the requested menu visibility.
    #[must_use]
    pub const fn visible(self) -> bool {
        self.inner.on
    }
}
forward_request!(
    SetVisible,
    crate::command::SetMenuDisplay,
    Plain,
    rows = [MenuDisplay]
);

/// Moves the menu cursor with an applied-only navigation request.
///
/// If the menu is closed, the same packet drives physical pan/tilt movement.
/// Send a bounded STOP to end that movement; dropping its handle does not stop it.
///
/// On the Sony FR7, menu navigation shares the cancellation timing and restart
/// gate of other pan/tilt drive requests.
#[derive(Debug, Clone, Copy)]
pub struct Navigate {
    inner: crate::command::MenuNavigate,
    direction: MenuDirection,
}

impl Navigate {
    /// Creates a navigation request in the selected direction.
    #[must_use]
    pub fn new(direction: MenuDirection) -> Self {
        Self {
            inner: crate::command::MenuNavigate::new(direction),
            direction,
        }
    }

    /// Returns the requested navigation direction.
    #[must_use]
    pub const fn direction(self) -> MenuDirection {
        self.direction
    }
}
forward_request!(
    Navigate,
    crate::command::MenuNavigate,
    AppliedOnly,
    rows = [MenuNavigate]
);

/// Enters the current menu selection and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct Enter;
forward_request!(Enter, crate::command::PerformMenuAction, Plain, rows = [MenuSelect];
    crate::command::PerformMenuAction::new(crate::command::MenuAction::Select));

/// Returns to the preceding menu and waits for protocol completion.
#[derive(Debug, Clone, Copy)]
pub struct Back;
forward_request!(Back, crate::command::PerformMenuAction, Plain, rows = [MenuCancel];
    crate::command::PerformMenuAction::new(crate::command::MenuAction::Cancel));

/// Sends one checked direct-menu button state.
#[derive(Debug, Clone, Copy)]
pub struct Direct {
    inner: crate::command::DirectMenuControl,
}

impl Direct {
    /// Creates a direct-menu button request.
    #[must_use]
    pub const fn new(target: DirectMenuTarget, button: MenuButton) -> Self {
        Self {
            inner: crate::command::DirectMenuControl::new(target, button),
        }
    }

    /// Returns the selected direct-menu control.
    #[must_use]
    pub const fn target(self) -> DirectMenuTarget {
        self.inner.target()
    }

    /// Returns the requested button state.
    #[must_use]
    pub const fn button(self) -> MenuButton {
        self.inner.button()
    }
}
forward_request!(
    Direct,
    crate::command::DirectMenuControl,
    Plain,
    rows = [DirectMenu]
);

/// Toggles menu visibility through the documented direct-menu control.
#[derive(Debug, Clone, Copy)]
pub struct ToggleVisible;
forward_request!(ToggleVisible, crate::command::DirectMenuControl, Plain, rows = [DirectMenu];
    crate::command::DirectMenuControl::open_close());
