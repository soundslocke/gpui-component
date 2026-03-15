use crate::{ActiveTheme, Collapsible, h_flex, sidebar::SidebarItem, v_flex};
use gpui::{
    App, ElementId, IntoElement, ParentElement, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

/// A group of items in the [`super::Sidebar`].
#[derive(Clone)]
pub struct SidebarGroup<E: SidebarItem + 'static> {
    label: SharedString,
    uppercase: bool,
    bottom_border: bool,
    collapsed: bool,
    children: Vec<E>,
}

impl<E: SidebarItem> SidebarGroup<E> {
    /// Create a new [`SidebarGroup`] with the given label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            uppercase: false,
            bottom_border: false,
            collapsed: false,
            children: Vec::new(),
        }
    }

    /// Render the group label in uppercase.
    pub fn uppercase(mut self, uppercase: bool) -> Self {
        self.uppercase = uppercase;
        self
    }

    /// Add a 1px bottom border below the group label.
    pub fn label_border(mut self, border: bool) -> Self {
        self.bottom_border = border;
        self
    }

    /// Add a child to the sidebar group, the child should implement [`SidebarItem`].
    pub fn child(mut self, child: E) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple children to the sidebar group.
    ///
    /// See also [`SidebarGroup::child`].
    pub fn children(mut self, children: impl IntoIterator<Item = E>) -> Self {
        self.children.extend(children);
        self
    }
}

impl<E: SidebarItem> Collapsible for SidebarGroup<E> {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl<E: SidebarItem> SidebarItem for SidebarGroup<E> {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();

        let bottom_border = self.bottom_border;
        let label: SharedString = if self.uppercase {
            self.label.to_uppercase().into()
        } else {
            self.label
        };

        v_flex()
            .relative()
            .when(!self.collapsed, |this| {
                this.child(
                    h_flex()
                        .flex_shrink_0()
                        .px_2()
                        .rounded(cx.theme().radius)
                        .text_xs()
                        .text_color(cx.theme().sidebar_foreground.opacity(0.5))
                        .h_8()
                        .child(label)
                        .when(bottom_border, |this| {
                            this.border_b_1()
                                .border_color(cx.theme().sidebar_border)
                                .rounded_none()
                                .mb(px(4.0))
                        }),
                )
            })
            .child(
                div()
                    .gap_2()
                    .flex_col()
                    .children(self.children.into_iter().enumerate().map(|(ix, child)| {
                        child
                            .collapsed(self.collapsed)
                            .render(format!("{}-{}", id, ix), window, cx)
                            .into_any_element()
                    })),
            )
    }
}
