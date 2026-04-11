//! An autocomplete text input with a filtered suggestion popup.
//!
//! `SuggestInput` combines a single-line text input with a floating popup list
//! of suggestions. Typing in the input filters the list; pressing Up/Down
//! navigates the highlighted item; pressing Enter or clicking an item fills the
//! input with that value. A separate "Add" button (provided by the caller) is
//! typically used to commit the value.
//!
//! Unlike [`Select`](crate::select::Select), the popup has no dedicated search
//! input — the text input itself acts as the search field.

use gpui::{
    deferred, div, prelude::FluentBuilder, px, rems, AnyElement, App, AppContext, Bounds, Context,
    Edges, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, RenderOnce, SharedString, Styled,
    Subscription, Task, WeakEntity, Window,
};

use crate::{
    actions::{Cancel, SelectDown, SelectUp},
    anchored::anchored,
    dialog::ConfirmDialog,
    global_state::GlobalState,
    h_flex,
    input::{Input, InputEvent, InputState},
    list::{List, ListDelegate, ListState},
    v_flex, ActiveTheme, ElementExt as _, IndexPath, Selectable, Sizable, Size, StyleSized,
    StyledExt,
};

// ---------------------------------------------------------------------------
// Event
// ---------------------------------------------------------------------------

/// Emitted when the user confirms a suggestion from the popup list.
#[derive(Clone)]
pub enum SuggestInputEvent {
    /// The user confirmed a value (via Enter key or click).
    Confirm(SharedString),
}

// ---------------------------------------------------------------------------
// Delegate
// ---------------------------------------------------------------------------

/// List delegate that holds a flat list of string suggestions and filters them
/// by case-insensitive substring match.
pub(crate) struct SuggestDelegate {
    items: Vec<SharedString>,
    matched_items: Vec<SharedString>,
    selected_index: Option<IndexPath>,
    state: WeakEntity<SuggestInputState>,
}

impl SuggestDelegate {
    fn new(items: Vec<SharedString>, state: WeakEntity<SuggestInputState>) -> Self {
        Self {
            matched_items: items.clone(),
            items,
            selected_index: None,
            state,
        }
    }
}

impl ListDelegate for SuggestDelegate {
    type Item = SuggestListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.matched_items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let text = self.matched_items.get(ix.row)?;
        let selected = self.selected_index.map_or(false, |sel| sel.eq_row(ix));
        let size = self
            .state
            .upgrade()
            .map_or(Size::Small, |s| s.read(cx).size);
        Some(
            SuggestListItem::new(ix.row)
                .selected(selected)
                .with_size(size)
                .child(div().whitespace_nowrap().child(text.clone())),
        )
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let query_lower = query.to_lowercase();
        self.matched_items = if query_lower.is_empty() {
            self.items.clone()
        } else {
            self.items
                .iter()
                .filter(|item| item.to_lowercase().contains(&query_lower))
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let value = self
            .selected_index
            .and_then(|ix| self.matched_items.get(ix.row))
            .cloned();
        let state = self.state.clone();
        cx.defer_in(window, move |_, window, cx| {
            if let Some(value) = value {
                _ = state.update(cx, |this, cx| {
                    this.input.update(cx, |input, cx| {
                        input.set_value(value.clone(), window, cx);
                    });
                    cx.emit(SuggestInputEvent::Confirm(value));
                    this.set_open(false, cx);
                });
            }
        });
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let state = self.state.clone();
        cx.defer_in(window, move |_, _, cx| {
            _ = state.update(cx, |this, cx| {
                this.set_open(false, cx);
            });
        });
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .justify_center()
            .py_4()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("No matches")
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct SuggestInputState {
    /// The text input entity. Accessible for wiring external subscriptions.
    pub input: Entity<InputState>,
    list: Entity<ListState<SuggestDelegate>>,
    open: bool,
    /// When true, the popup hides entirely when the delegate has no matches
    /// instead of showing a "No matches" empty state.
    pub(crate) hide_when_empty: bool,
    pub(crate) size: Size,
    bounds: Bounds<Pixels>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<SuggestInputEvent> for SuggestInputState {}

impl SuggestInputState {
    pub fn new(
        items: impl IntoIterator<Item = impl Into<SharedString>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let items: Vec<SharedString> = items.into_iter().map(|s| s.into()).collect();
        let input: Entity<InputState> = cx.new(|cx| {
            InputState::new(window, cx).placeholder(SharedString::from("Search..."))
        });
        let weak = cx.entity().downgrade();

        let list: Entity<ListState<SuggestDelegate>> = cx.new(|cx| {
            ListState::new(SuggestDelegate::new(items, weak), window, cx).reset_on_cancel(false)
        });

        let mut subs = Vec::new();

        // Input events → drive list search, confirm, and open/close.
        let list_for_sub = list.clone();
        let input_for_sub = input.clone();
        subs.push(cx.subscribe_in(
            &input,
            window,
            move |this: &mut SuggestInputState,
                  _,
                  event: &InputEvent,
                  window: &mut Window,
                  cx: &mut Context<SuggestInputState>| {
                match event {
                    InputEvent::Change => {
                        // Only open the popup if the input is focused — ignore
                        // programmatic value changes (e.g. initial pre-fill).
                        if !input_for_sub.focus_handle(cx).is_focused(window) {
                            return;
                        }
                        let text = input_for_sub.read(cx).value().to_string();
                        list_for_sub.update(cx, |list: &mut ListState<SuggestDelegate>, cx| {
                            list.set_query(&text, window, cx);
                        });
                        // Suppress the popup when the input value already
                        // exactly matches the only filtered item — there's
                        // nothing useful to show. This also breaks the
                        // re-open loop after a confirm: confirm sets the
                        // value (emitting Change), the new value matches
                        // the now-singleton filter, and we keep the popup
                        // closed instead of re-opening it.
                        let matched = &list_for_sub.read(cx).delegate().matched_items;
                        let redundant =
                            matched.len() == 1 && matched[0] == *text.as_str();
                        if redundant {
                            if this.open {
                                this.set_open(false, cx);
                            }
                        } else if !this.open {
                            this.set_open(true, cx);
                        }
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        if this.open {
                            list_for_sub.update(
                                cx,
                                |list: &mut ListState<SuggestDelegate>, cx| {
                                    list.confirm_selection(window, cx);
                                },
                            );
                        }
                    }
                    InputEvent::Focus => {
                        if !this.open {
                            let text = input_for_sub.read(cx).value().to_string();
                            list_for_sub.update(
                                cx,
                                |list: &mut ListState<SuggestDelegate>, cx| {
                                    list.set_query(&text, window, cx);
                                },
                            );
                            // Don't open if the current value is already an
                            // exact match for the only filtered item.
                            let matched = &list_for_sub.read(cx).delegate().matched_items;
                            let redundant =
                                matched.len() == 1 && matched[0] == *text.as_str();
                            if !redundant {
                                this.set_open(true, cx);
                            }
                        }
                    }
                    InputEvent::Blur => {
                        // Close the popup when the input loses focus (e.g.
                        // the user pressed Tab or clicked outside). However,
                        // skip closing if focus moved to the popup list
                        // itself — that means the user clicked a list item,
                        // and the list's `confirm` flow will set the input
                        // value and close the popup. Closing here would
                        // hide the list before mouse-up arrives, killing
                        // the click handler.
                        if !this.open {
                            return;
                        }
                        let list_focused = this
                            .list
                            .read(cx)
                            .focus_handle
                            .contains_focused(window, cx);
                        if !list_focused {
                            this.set_open(false, cx);
                        }
                    }
                }
            },
        ));

        // List cancel event → close popup.
        subs.push(cx.subscribe_in(
            &list,
            window,
            |this: &mut SuggestInputState,
             _,
             event: &crate::list::ListEvent,
             _window: &mut Window,
             cx: &mut Context<SuggestInputState>| {
                if matches!(event, crate::list::ListEvent::Cancel) {
                    this.set_open(false, cx);
                }
            },
        ));

        Self {
            input,
            list,
            open: false,
            hide_when_empty: false,
            size: Size::Small,
            bounds: Bounds::default(),
            _subs: subs,
        }
    }

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        let fh = self.input.focus_handle(cx);
        if self.open {
            GlobalState::global_mut(cx).register_deferred_popover(&fh);
        } else {
            GlobalState::global_mut(cx).unregister_deferred_popover(&fh);
        }
        cx.notify();
    }

    /// When true, the popup hides entirely when there are no matching items
    /// instead of showing a "No matches" empty state. Default: false.
    pub fn hide_when_empty(mut self, hide: bool) -> Self {
        self.hide_when_empty = hide;
        self
    }

    /// Read the current input text.
    pub fn value(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    /// Set the input text programmatically.
    pub fn set_value(&self, value: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        self.input.update(cx, |input, cx| {
            input.set_value(value.into(), window, cx);
        });
    }
}

impl Focusable for SuggestInputState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for SuggestInputState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

// ---------------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct SuggestInput {
    state: Entity<SuggestInputState>,
    size: Size,
}

impl SuggestInput {
    pub fn new(state: &Entity<SuggestInputState>) -> Self {
        Self {
            state: state.clone(),
            size: Size::default(),
        }
    }

    pub fn small(mut self) -> Self {
        self.size = Size::Small;
        self
    }
}

impl Sizable for SuggestInput {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for SuggestInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let mut show_popup = state.open;
        let input = state.input.clone();
        let list = state.list.clone();
        let bounds = state.bounds;
        let hide_when_empty = state.hide_when_empty;
        let popup_radius = cx.theme().radius.min(px(8.));

        // Suppress popup when empty and configured to hide.
        if show_popup && hide_when_empty {
            let match_count = list.read(cx).delegate().matched_items.len();
            if match_count == 0 {
                show_popup = false;
            }
        }

        let state_entity = self.state.clone();

        div()
            .id(("suggest-input", self.state.entity_id()))
            .w_full()
            .relative()
            .on_prepaint({
                let state = self.state.clone();
                move |bounds, _, cx| {
                    state.update(cx, |s, _| s.bounds = bounds);
                }
            })
            .key_context("List")
            .on_action({
                let list = list.clone();
                move |_: &SelectUp, window: &mut Window, cx: &mut App| {
                    list.update(cx, |l: &mut ListState<SuggestDelegate>, cx| {
                        l.select_prev(window, cx);
                    });
                }
            })
            .on_action({
                let list = list.clone();
                move |_: &SelectDown, window: &mut Window, cx: &mut App| {
                    list.update(cx, |l: &mut ListState<SuggestDelegate>, cx| {
                        l.select_next(window, cx);
                    });
                }
            })
            .on_action({
                let state = state_entity.clone();
                move |_: &Cancel, _window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.set_open(false, cx));
                }
            })
            // Prevent Enter from bubbling up to a parent Dialog's
            // ConfirmDialog handler while the user is interacting with the
            // suggest input (typing or picking from the popup).
            .on_action(|_: &ConfirmDialog, _window, _cx| {})
            .child(Input::new(&input).with_size(self.size))
            .when(show_popup, |this: gpui::Stateful<gpui::Div>| {
                this.child(
                    deferred(
                        anchored()
                            .snap_to_window_with_margin(px(8.))
                            .child(
                                div()
                                    .occlude()
                                    .w(bounds.size.width)
                                    .child(
                                        v_flex()
                                            .occlude()
                                            .mt_1p5()
                                            .bg(cx.theme().background)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .rounded(popup_radius)
                                            .shadow_md()
                                            .child(
                                                List::new(&list)
                                                    .with_size(self.size)
                                                    .max_h(rems(15.))
                                                    .paddings(Edges::all(px(4.))),
                                            ),
                                    )
                                    .on_mouse_down_out({
                                        let state = state_entity.clone();
                                        move |_, _, cx| {
                                            state.update(cx, |s, cx| s.set_open(false, cx));
                                        }
                                    }),
                            ),
                    )
                    .with_priority(1),
                )
            })
    }
}

// ---------------------------------------------------------------------------
// List item (mirrors SelectListItem but simpler)
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub(crate) struct SuggestListItem {
    id: gpui::ElementId,
    size: Size,
    selected: bool,
    children: Vec<AnyElement>,
}

impl SuggestListItem {
    fn new(ix: usize) -> Self {
        Self {
            id: ("suggest-item", ix).into(),
            size: Size::default(),
            selected: false,
            children: Vec::new(),
        }
    }
}

impl gpui::ParentElement for SuggestListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Selectable for SuggestListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Sizable for SuggestListItem {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for SuggestListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .id(self.id)
            .gap_x_1()
            .py_1()
            .px_2()
            .rounded(cx.theme().radius)
            .text_color(cx.theme().foreground)
            .items_center()
            .list_size(self.size)
            .when(!self.selected, |this: gpui::Stateful<gpui::Div>| {
                this.hover(|style| style.bg(cx.theme().accent.alpha(0.7)))
            })
            .when(self.selected, |this: gpui::Stateful<gpui::Div>| {
                this.bg(cx.theme().accent)
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_x_1()
                    .child(div().w_full().children(self.children)),
            )
    }
}
