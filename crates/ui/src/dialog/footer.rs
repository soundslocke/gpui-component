use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
    RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled, Window, div, relative,
};

use crate::{
    ActiveTheme as _, StyledExt as _,
    dialog::{CancelDialog, ConfirmDialog},
    h_flex,
};

/// Footer section of a dialog, typically contains action buttons.
///
/// # Examples
///
/// ```ignore
/// DialogFooter::new()
///     .child(DialogClose::new().child(Button::new("cancel").label("Cancel")))
///     .child(Button::new("confirm").label("Confirm"))
/// ```
#[derive(IntoElement)]
pub struct DialogFooter {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DialogFooter {
    pub fn new() -> Self {
        Self { style: StyleRefinement::default(), children: Vec::new() }
    }
}

impl ParentElement for DialogFooter {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DialogFooter {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for DialogFooter {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_2()
            .justify_end()
            .line_height(relative(1.))
            .rounded_b(cx.theme().radius_lg)
            .refine_style(&self.style)
            .children(self.children)
    }
}

pub trait DialogFooterButton {
    fn is_cancel(&self) -> bool {
        false
    }

    fn is_action(&self) -> bool {
        false
    }
}

/// Retrieve or create a persistent focus handle for a dialog footer button,
/// keyed by its element ID so it survives across renders.
fn dialog_button_focus_handle(id: &str, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(id.to_string(), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
}

#[derive(IntoElement)]
pub struct DialogClose {
    children: Vec<AnyElement>,
}

impl DialogClose {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }
}

impl ParentElement for DialogClose {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogClose {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = dialog_button_focus_handle("dialog-close", window, cx);

        div()
            .size_full()
            .id("dialog-close")
            .track_focus(&focus_handle)
            .on_mouse_down(MouseButton::Left, {
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    // Explicitly focus this element so that the action dispatched in
                    // on_click will route through the Dialog's on_action handler.
                    // Button children call window.prevent_default() which suppresses
                    // automatic focus, so we must focus explicitly here.
                    window.focus(&focus_handle, cx);
                }
            })
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(CancelDialog), cx))
            .children(self.children)
    }
}

#[derive(IntoElement)]
pub struct DialogAction {
    children: Vec<AnyElement>,
}

impl DialogAction {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }
}

impl ParentElement for DialogAction {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DialogAction {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = dialog_button_focus_handle("dialog-action", window, cx);

        div()
            .size_full()
            .id("dialog-action")
            .track_focus(&focus_handle)
            .on_mouse_down(MouseButton::Left, {
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    window.focus(&focus_handle, cx);
                }
            })
            .on_click(move |_, window, cx| window.dispatch_action(Box::new(ConfirmDialog), cx))
            .children(self.children)
    }
}
