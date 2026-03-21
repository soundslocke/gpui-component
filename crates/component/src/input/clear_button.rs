use gpui::App;

use crate::{
    Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};

#[inline]
pub(crate) fn clear_button(icon: Option<Icon>, _: &App) -> Button {
    Button::new("clean")
        .icon(icon.unwrap_or_else(|| Icon::new(IconName::Close)))
        .text()
        .xsmall()
        .tab_stop(false)
}
