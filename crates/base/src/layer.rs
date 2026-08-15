//! Deferred paint priorities for the window's overlay surfaces.
//!
//! Every surface that escapes its parent's paint order renders through
//! [`gpui::deferred`]. GPUI paints deferred draws in ascending priority order,
//! so priority is what decides which overlay covers which, regardless of where
//! each one sits in the element tree.
//!
//! The ladder, from bottom to top:
//!
//! - [`DIALOG_PRIORITY`] plus the dialog's stack index, so a dialog opened on
//!   top of another one covers it.
//! - [`POPUP_PRIORITY`] plus the submenu depth, so menus, selects, and
//!   popovers stay above the dialog that hosts them.
//! - [`TOOLTIP_PRIORITY`], above everything: any of the surfaces below can own
//!   a tooltip trigger.
//!
//! The gaps between the tiers leave room for the per-surface offsets.

/// Base priority for dialogs. The dialog's stack index is added to it.
pub const DIALOG_PRIORITY: usize = 10;

/// Base priority for interactive surfaces that must appear above dialogs.
/// Nested menus add their depth to it.
pub const POPUP_PRIORITY: usize = 100;

/// Priority for the window's tooltip overlay, the topmost surface.
pub const TOOLTIP_PRIORITY: usize = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_are_ordered_with_room_for_offsets() {
        assert!(DIALOG_PRIORITY < POPUP_PRIORITY);
        assert!(POPUP_PRIORITY < TOOLTIP_PRIORITY);
        // Stacked dialogs and nested menus offset from their tier, so no
        // realistic depth may reach the tier above.
        assert!(POPUP_PRIORITY - DIALOG_PRIORITY > 16);
        assert!(TOOLTIP_PRIORITY - POPUP_PRIORITY > 16);
    }
}
