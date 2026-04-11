use crate::{
    ActiveTheme as _, Collapsible, Icon, IconName, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    h_flex,
    menu::{ContextMenuExt, PopupMenu},
    sidebar::SidebarItem,
    v_flex,
};
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, percentage,
    prelude::FluentBuilder,
};
use std::rc::Rc;

type SidebarKeyHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Menu for the [`super::Sidebar`]
#[derive(Clone)]
pub struct SidebarMenu {
    style: StyleRefinement,
    collapsed: bool,
    items: Vec<SidebarMenuItem>,
    focus_handle: Option<FocusHandle>,
    on_select_prev: Option<SidebarKeyHandler>,
    on_select_next: Option<SidebarKeyHandler>,
    on_select_first: Option<SidebarKeyHandler>,
    on_select_last: Option<SidebarKeyHandler>,
    on_confirm: Option<SidebarKeyHandler>,
}

impl SidebarMenu {
    /// Create a new SidebarMenu
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            items: Vec::new(),
            collapsed: false,
            focus_handle: None,
            on_select_prev: None,
            on_select_next: None,
            on_select_first: None,
            on_select_last: None,
            on_confirm: None,
        }
    }

    /// Add a [`SidebarMenuItem`] child menu item to the sidebar menu.
    ///
    /// See also [`SidebarMenu::children`].
    pub fn child(mut self, child: impl Into<SidebarMenuItem>) -> Self {
        self.items.push(child.into());
        self
    }

    /// Add multiple [`SidebarMenuItem`] child menu items to the sidebar menu.
    pub fn children(
        mut self,
        children: impl IntoIterator<Item = impl Into<SidebarMenuItem>>,
    ) -> Self {
        self.items = children.into_iter().map(Into::into).collect();
        self
    }

    /// Make this menu focusable using the given focus handle.
    ///
    /// When attached, the menu's root element calls `track_focus` so the
    /// caller can place it in the tab order (configure the handle with
    /// `tab_stop(true)` to enable Tab navigation). While focused, the menu
    /// dispatches keyboard events to the `on_select_*` / `on_confirm`
    /// callbacks below so the parent can drive selection through its own
    /// data model.
    pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.focus_handle = Some(focus_handle.clone());
        self
    }

    /// Callback fired when the Up arrow key is pressed while the menu is focused.
    pub fn on_select_prev(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_prev = Some(Rc::new(handler));
        self
    }

    /// Callback fired when the Down arrow key is pressed while the menu is focused.
    pub fn on_select_next(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_next = Some(Rc::new(handler));
        self
    }

    /// Callback fired when the Home key is pressed while the menu is focused.
    pub fn on_select_first(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_first = Some(Rc::new(handler));
        self
    }

    /// Callback fired when the End key is pressed while the menu is focused.
    pub fn on_select_last(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select_last = Some(Rc::new(handler));
        self
    }

    /// Callback fired when Enter or Space is pressed while the menu is focused.
    pub fn on_confirm(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
}

impl Collapsible for SidebarMenu {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for SidebarMenu {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();
        let collapsed = self.collapsed;
        let focus_handle = self.focus_handle.clone();
        let parent_focusable = focus_handle.is_some();
        let parent_focused = focus_handle
            .as_ref()
            .map_or(false, |fh| fh.is_focused(window));
        let on_select_prev = self.on_select_prev.clone();
        let on_select_next = self.on_select_next.clone();
        let on_select_first = self.on_select_first.clone();
        let on_select_last = self.on_select_last.clone();
        let on_confirm = self.on_confirm.clone();
        let has_key_handlers = on_select_prev.is_some()
            || on_select_next.is_some()
            || on_select_first.is_some()
            || on_select_last.is_some()
            || on_confirm.is_some();

        v_flex()
            .gap_2()
            .refine_style(&self.style)
            .when_some(focus_handle.as_ref(), |this, fh| this.track_focus(fh))
            .when(has_key_handlers, |this| {
                this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "up" => {
                            if let Some(handler) = &on_select_prev {
                                handler(window, cx);
                                cx.stop_propagation();
                            }
                        }
                        "down" => {
                            if let Some(handler) = &on_select_next {
                                handler(window, cx);
                                cx.stop_propagation();
                            }
                        }
                        "home" => {
                            if let Some(handler) = &on_select_first {
                                handler(window, cx);
                                cx.stop_propagation();
                            }
                        }
                        "end" => {
                            if let Some(handler) = &on_select_last {
                                handler(window, cx);
                                cx.stop_propagation();
                            }
                        }
                        "enter" | "space" => {
                            if let Some(handler) = &on_confirm {
                                handler(window, cx);
                                cx.stop_propagation();
                            }
                        }
                        _ => {}
                    }
                })
            })
            .children(self.items.into_iter().enumerate().map(|(ix, item)| {
                let id = SharedString::from(format!("{}-{}", id, ix));
                item.collapsed(collapsed)
                    .with_parent_focused(parent_focused)
                    .with_parent_focusable(parent_focusable)
                    .render(id, window, cx)
                    .into_any_element()
            }))
    }
}

impl Styled for SidebarMenu {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

/// Menu item for the [`SidebarMenu`]
#[derive(Clone)]
pub struct SidebarMenuItem {
    icon: Option<Icon>,
    label: SharedString,
    subtitle: Option<SharedString>,
    handler: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
    active: bool,
    default_open: bool,
    click_to_open: bool,
    collapsed: bool,
    children: Vec<Self>,
    suffix: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    disabled: bool,
    context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu + 'static>>,
    /// Set internally by [`SidebarMenu`] when its focus handle is focused,
    /// so the active item can render a focus ring as the keyboard cursor.
    parent_focused: bool,
    /// Set internally by [`SidebarMenu`] when it has a focus handle attached,
    /// so the item can suppress GPUI's mouse-down auto-focus on the parent
    /// (which would otherwise cause the focus ring to flash on the previously
    /// selected item between mouse-down and the click handler running).
    parent_focusable: bool,
}

impl SidebarMenuItem {
    /// Create a new [`SidebarMenuItem`] with a label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            icon: None,
            label: label.into(),
            subtitle: None,
            handler: Rc::new(|_, _, _| {}),
            active: false,
            collapsed: false,
            default_open: false,
            click_to_open: false,
            children: Vec::new(),
            suffix: None,
            disabled: false,
            context_menu: None,
            parent_focused: false,
            parent_focusable: false,
        }
    }

    /// Set a subtitle line below the label, rendered in smaller muted text.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set the icon for the menu item
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the active state of the menu item
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Add a click handler to the menu item
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Rc::new(handler);
        self
    }

    /// Set the collapsed state of the menu item
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Set the default open state of the Submenu, default is `false`.
    ///
    /// This only used on initial render, the internal state will be used afterwards.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Set whether clicking the menu item open the submenu.
    ///
    /// Default is `false`.
    ///
    /// If `false` we only handle open/close via the caret button.
    pub fn click_to_open(mut self, click_to_open: bool) -> Self {
        self.click_to_open = click_to_open;
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Self>>) -> Self {
        self.children = children.into_iter().map(Into::into).collect();
        self
    }

    /// Set the suffix for the menu item.
    pub fn suffix<F, E>(mut self, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.suffix = Some(Rc::new(move |window, cx| {
            builder(window, cx).into_any_element()
        }));
        self
    }

    /// Set disabled flat for menu item.
    pub fn disable(mut self, disable: bool) -> Self {
        self.disabled = disable;
        self
    }

    fn is_submenu(&self) -> bool {
        self.children.len() > 0
    }

    /// Set the context menu for the menu item.
    pub fn context_menu(
        mut self,
        f: impl Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu + 'static,
    ) -> Self {
        self.context_menu = Some(Rc::new(f));
        self
    }

    /// Internal: marks this item as belonging to a [`SidebarMenu`] whose
    /// focus handle is currently focused. Used by [`SidebarMenu`] to draw a
    /// focus ring on the active item.
    pub(super) fn with_parent_focused(mut self, parent_focused: bool) -> Self {
        self.parent_focused = parent_focused;
        self
    }

    /// Internal: marks this item as belonging to a [`SidebarMenu`] whose
    /// focus handle is attached. Lets the item suppress GPUI's mouse-down
    /// auto-focus on the parent menu so clicking an item doesn't briefly
    /// focus the parent (which causes a focus-ring flash).
    pub(super) fn with_parent_focusable(mut self, parent_focusable: bool) -> Self {
        self.parent_focusable = parent_focusable;
        self
    }
}

impl FluentBuilder for SidebarMenuItem {}

impl Collapsible for SidebarMenuItem {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for SidebarMenuItem {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let click_to_open = self.click_to_open;
        let default_open = self.default_open;
        let parent_focusable = self.parent_focusable;
        let id = id.into();
        let is_submenu = self.is_submenu();
        let open_state = if is_submenu {
            Some(window.use_keyed_state(id.clone(), cx, |_, _| default_open))
        } else {
            None
        };
        let handler = self.handler.clone();
        let is_collapsed = self.collapsed;
        let is_active = self.active;
        let is_hoverable = !is_active && !self.disabled;
        let is_disabled = self.disabled;
        let is_keyboard_focused = self.parent_focused && is_active;
        let is_open = open_state
            .as_ref()
            .map_or(false, |s| !is_collapsed && *s.read(cx));

        div()
            .id(id.clone())
            .w_full()
            .child(
                h_flex()
                    .size_full()
                    .id("item")
                    .relative()
                    .overflow_x_hidden()
                    .flex_shrink_0()
                    .p_2()
                    .gap_x_2()
                    .rounded(cx.theme().radius)
                    .text_sm()
                    // Suppress GPUI's bubble-phase auto-focus on the parent
                    // [`SidebarMenu`] when the user clicks an item. The parent
                    // would otherwise capture focus on mouse-down — before the
                    // click handler runs to update selection — and the active
                    // item's focus ring would flash on the previously selected
                    // entry. Tab-key navigation still works because tab stops
                    // are sourced from the dispatch tree, not mouse events.
                    .when(parent_focusable && !is_disabled, |this| {
                        this.on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.prevent_default();
                        })
                    })
                    .when(is_hoverable, |this| {
                        this.hover(|this| {
                            this.bg(cx.theme().sidebar_accent.opacity(0.8))
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                    })
                    .when(is_active, |this| {
                        this.font_medium()
                            .bg(cx.theme().sidebar_accent)
                            .text_color(cx.theme().sidebar_accent_foreground)
                    })
                    // Focus ring overlay drawn *inside* the item's bounds. We
                    // can't use [`crate::FocusableExt::focus_ring`] here because
                    // it positions the ring at negative offsets just outside
                    // the parent — those pixels get clipped by the
                    // [`gpui::list`] content mask in `Sidebar::render`. Drawing
                    // it inset by a pixel keeps the ring fully visible while
                    // still tracing the rounded item shape.
                    .when(is_keyboard_focused, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top_px()
                                .bottom_px()
                                .left_px()
                                .right_px()
                                .border_1()
                                .border_color(cx.theme().ring)
                                .rounded(cx.theme().radius),
                        )
                    })
                    .when_some(self.icon.clone(), |this, icon| this.child(icon))
                    .when(is_collapsed, |this| {
                        this.justify_center().when(is_active, |this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                    })
                    .when(!is_collapsed, |this| {
                        let has_subtitle = self.subtitle.is_some();

                        this.when(!has_subtitle, |this| this.h_7())
                            .when(has_subtitle, |this| this.py_1p5())
                            .child(
                                h_flex()
                                    .flex_1()
                                    .gap_x_2()
                                    .justify_between()
                                    .overflow_x_hidden()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .overflow_x_hidden()
                                            .child(self.label.clone())
                                            .when_some(self.subtitle.clone(), |this, subtitle| {
                                                this.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(
                                                            cx.theme()
                                                                .sidebar_foreground
                                                                .opacity(0.5),
                                                        )
                                                        .child(subtitle),
                                                )
                                            }),
                                    )
                                    .when_some(self.suffix.clone(), |this, suffix| {
                                        this.child(suffix(window, cx).into_any_element())
                                    }),
                            )
                            .when_some(open_state.clone(), |this, open_state| {
                                this.child(
                                    Button::new("caret")
                                        .xsmall()
                                        .ghost()
                                        .icon(
                                            Icon::new(IconName::ChevronRight)
                                                .size_4()
                                                .when(is_open, |this| {
                                                    this.rotate(percentage(90. / 360.))
                                                }),
                                        )
                                        .on_click({
                                            move |_, _, cx| {
                                                // Avoid trigger item click, just expand/collapse submenu
                                                cx.stop_propagation();
                                                open_state.update(cx, |is_open, cx| {
                                                    *is_open = !*is_open;
                                                    cx.notify();
                                                })
                                            }
                                        }),
                                )
                            })
                    })
                    .when(is_disabled, |this| {
                        this.text_color(cx.theme().muted_foreground)
                    })
                    .when(!is_disabled, |this| {
                        this.on_click({
                            let open_state = open_state.clone();
                            move |ev, window, cx| {
                                if click_to_open {
                                    if let Some(ref s) = open_state {
                                        s.update(cx, |is_open: &mut bool, cx| {
                                            *is_open = true;
                                            cx.notify();
                                        });
                                    }
                                }

                                handler(ev, window, cx)
                            }
                        })
                    })
                    .map(|this| {
                        if let Some(context_menu) = self.context_menu {
                            this.context_menu(move |menu, window, cx| {
                                context_menu(menu, window, cx)
                            })
                            .into_any_element()
                        } else {
                            this.into_any_element()
                        }
                    }),
            )
            .when(is_open, |this| {
                this.child(
                    v_flex()
                        .id("submenu")
                        .border_l_1()
                        .border_color(cx.theme().sidebar_border)
                        .gap_1()
                        .ml_3p5()
                        .pl_2p5()
                        .py_0p5()
                        .children(self.children.into_iter().enumerate().map(|(ix, item)| {
                            let id = format!("{}-{}", id, ix);
                            item.render(id, window, cx).into_any_element()
                        })),
                )
            })
    }
}
