use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Edges, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, KeyBinding, Length,
    ParentElement, Render, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, actions, deferred, div, prelude::FluentBuilder, px, rems,
};
use rust_i18n::t;

use crate::ThemeStyled as _;
use crate::{
    ActiveTheme, Disableable, ElementExt as _, Icon, IconName, IndexPath, Sizable, Size,
    StyleSized, StyledExt,
    actions::Cancel,
    h_flex,
    input::{clear_button, input_style},
    list::List,
    searchable_list::{
        SearchableListChange, SearchableListDelegate, SearchableListItem, SearchableListState,
    },
    v_flex,
};
use gpui_base::{GlobalState, Select as BaseSelect};

// `tab` and `shift-tab` are claimed in the Select context so the Select can
// implement "commit current selection then advance focus" while the dropdown
// menu is open. When the menu is closed, the handler dispatches the regular
// `Tab`/`TabPrev` actions back through Root so the standard focus-trap aware
// navigation runs unchanged.
actions!(select, [SelectTab, SelectTabPrev]);

pub(crate) fn init(cx: &mut App) {
    // The rest of the Select context bindings live in `gpui_base::select`.
    cx.bind_keys([
        KeyBinding::new(
            "space",
            crate::actions::Confirm { secondary: false },
            Some(gpui_base::SELECT_CONTEXT),
        ),
        KeyBinding::new("tab", SelectTab, Some(gpui_base::SELECT_CONTEXT)),
        KeyBinding::new("shift-tab", SelectTabPrev, Some(gpui_base::SELECT_CONTEXT)),
    ])
}

// MARK: Public re-exports for back-compat

/// Re-exported for backward compatibility. New code should prefer [`SearchableGroup`].
pub use crate::searchable_list::SearchableGroup as SelectGroup;
/// Re-exported for backward compatibility. New code should prefer [`SearchableListDelegate`].
pub use crate::searchable_list::SearchableListDelegate as SelectDelegate;
/// Re-exported for backward compatibility. New code should prefer [`SearchableListItem`].
pub use crate::searchable_list::SearchableListItem as SelectItem;
/// Re-exported for backward compatibility. New code should prefer [`SearchableListItemElement`].
pub use crate::searchable_list::SearchableListItemElement as SelectListItem;
/// Re-exported for backward compatibility.
pub use crate::searchable_list::SearchableVec;

#[derive(IntoElement)]
pub struct Caret {
    size: Size,
    color: Option<Hsla>,
}

impl Caret {
    /// Create a select caret sized for its trigger.
    pub fn new(size: Size) -> Self {
        Self { size, color: None }
    }

    /// Set the caret color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Caret {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::new(IconName::ChevronDown)
            .with_size(match self.size {
                Size::XSmall => Size::XSmall,
                Size::Small => Size::Small,
                _ => Size::Medium,
            })
            .when_some(self.color, |this, color| this.text_color(color))
    }
}

/// Events emitted by [`SelectState`].
pub enum SelectEvent<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    Confirm(Option<<D::Item as SearchableListItem>::Value>),
}

// MARK: SelectOptions (builder only — applied to SearchableListState during render)

struct SelectOptions {
    style: StyleRefinement,
    size: Size,
    icon: Option<Icon>,
    cleanable: bool,
    placeholder: Option<SharedString>,
    accessibility_label: Option<SharedString>,
    title_prefix: Option<SharedString>,
    search_placeholder: Option<SharedString>,
    search_icon: Option<Icon>,
    menu_width: Length,
    menu_max_h: Length,
    disabled: bool,
    appearance: bool,
    focus_ring_enabled: bool,
    fit_items: bool,
}

impl Default for SelectOptions {
    fn default() -> Self {
        Self {
            style: StyleRefinement::default(),
            size: Size::default(),
            icon: None,
            cleanable: false,
            placeholder: None,
            accessibility_label: None,
            title_prefix: None,
            menu_width: Length::Auto,
            menu_max_h: rems(20.).into(),
            disabled: false,
            appearance: true,
            focus_ring_enabled: true,
            search_placeholder: None,
            search_icon: None,
            fit_items: false,
        }
    }
}

// MARK: SelectState

/// State of the [`Select`] component.
pub struct SelectState<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    pub(crate) state: SearchableListState<D>,

    // Select-specific fields
    searchable: bool,
    icon: Option<Icon>,
    title_prefix: Option<SharedString>,
    focus_ring_enabled: bool,
    search_icon: Option<Icon>,
    fit_items: bool,
}

/// A Select element.
#[derive(IntoElement)]
pub struct Select<D: SearchableListDelegate + 'static>
where
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    id: ElementId,
    state: Entity<SelectState<D>>,
    options: SelectOptions,
    empty: Option<Box<dyn Fn(&mut Window, &App) -> AnyElement + 'static>>,
}

impl<D> SelectState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    /// Create a new Select state.
    pub fn new(
        delegate: D,
        selected_index: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let weak = cx.entity().downgrade();
        let weak_confirm = weak.clone();
        let weak_cancel = weak.clone();
        let weak_empty = weak;

        let selected_indices = selected_index.into_iter().collect::<Vec<_>>();

        let state = SearchableListState::new(
            delegate,
            selected_indices,
            // on_confirm — commit the selection
            move |selected_index, _secondary, window, cx| {
                cx.defer_in(window, {
                    let weak_confirm = weak_confirm.clone();
                    move |list_state, window, cx| {
                        let mut selection = weak_confirm
                            .upgrade()
                            .map(|e| e.read(cx).state.selection.clone())
                            .unwrap_or_default();

                        let changes = {
                            let mut changes: Vec<SearchableListChange> = selection
                                .iter()
                                .map(|(ix, _)| SearchableListChange::Deselect { index: *ix })
                                .collect();

                            if let Some(ix) = selected_index {
                                changes.push(SearchableListChange::Select { index: ix });
                            }

                            changes
                        };

                        // on_will_change is called directly — entity-handle access would
                        // re-enter the ListState lock that defer_in holds for this callback.
                        list_state
                            .delegate_mut()
                            .delegate
                            .on_will_change(&mut selection, &changes);

                        let new_selection = weak_confirm.update(cx, |this, cx| {
                            this.state.selection = selection;

                            let final_value =
                                this.state.selection.first().map(|(_, i)| i.value().clone());

                            cx.emit(SelectEvent::Confirm(final_value));
                            cx.notify();
                            this.set_open(false, cx);
                            this.focus(window, cx);

                            this.state.selection.clone()
                        });

                        // Sync snapshot and fire on_confirm directly — same re-entrancy guard.
                        if let Ok(new_selection) = new_selection {
                            list_state
                                .delegate_mut()
                                .update_selection_snapshot(new_selection.clone());
                            list_state
                                .delegate_mut()
                                .delegate
                                .on_confirm(&new_selection);
                        }
                    }
                });
            },
            // on_cancel — restore cursor to committed index, close
            move |_final_selected_index, window, cx| {
                cx.defer_in(window, {
                    let weak_cancel = weak_cancel.clone();
                    move |list_state, window, cx| {
                        let committed_ix = weak_cancel
                            .upgrade()
                            .and_then(|e| e.read(cx).state.selection.first().map(|(ix, _)| *ix));

                        list_state.set_selected_index(committed_ix, window, cx);

                        _ = weak_cancel.update(cx, |this, cx| {
                            this.set_open(false, cx);
                            this.focus(window, cx);
                        });
                    }
                });
            },
            // on_render_empty
            move |window, cx| {
                if let Some(empty) = weak_empty
                    .upgrade()
                    .and_then(|e| e.read(cx).state.empty.as_ref().map(|f| f(window, cx)))
                {
                    empty
                } else {
                    h_flex()
                        .justify_center()
                        .py_6()
                        .text_color(cx.theme().muted_foreground.opacity(0.6))
                        .child(Icon::new(IconName::Inbox).size(px(28.)))
                        .into_any_element()
                }
            },
            Self::on_blur,
            window,
            cx,
        );

        Self {
            state,
            searchable: false,
            icon: None,
            title_prefix: None,
            focus_ring_enabled: true,
            search_icon: None,
            fit_items: false,
        }
    }

    /// Sets whether the dropdown menu is searchable, default is `false`.
    ///
    /// When `true`, a search input appears at the top of the dropdown menu.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// Set the selected index for the select.
    pub fn set_selected_index(
        &mut self,
        selected_index: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.list.update(cx, |list, cx| {
            list._set_selected_index(selected_index, window, cx);
        });

        let item = selected_index
            .and_then(|ix| self.state.list.read(cx).delegate().delegate.item(ix))
            .map(|i| i.clone());

        self.state.selection = match (selected_index, item) {
            (Some(ix), Some(item)) => vec![(ix, item)],
            _ => vec![],
        };
        self.state.sync_snapshot(cx);
    }

    /// Set selected value for the select.
    ///
    /// Looks up the position from the delegate and sets the selected index accordingly.
    /// Passes `None` when the value is not found.
    ///
    /// The delegate looks the value up in its matched items, so an active search query is
    /// cleared first to get an index into the full item list.
    pub fn set_selected_value(
        &mut self,
        selected_value: &<D::Item as SearchableListItem>::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.clear_query(window, cx);

        let selected_index = self
            .state
            .list
            .read(cx)
            .delegate()
            .delegate
            .position(selected_value);

        self.set_selected_index(selected_index, window, cx);
    }

    /// Replace the delegate (item data) for the select state.
    pub fn set_items(&mut self, items: D, _: &mut Window, cx: &mut Context<Self>)
    where
        D: SearchableListDelegate + 'static,
    {
        self.state.list.update(cx, |list, _| {
            list.delegate_mut().delegate = items;
        });
    }

    /// Get the current selected index.
    pub fn selected_index(&self, cx: &App) -> Option<IndexPath> {
        self.state.list.read(cx).selected_index()
    }

    /// Get the current selected value.
    pub fn selected_value(&self) -> Option<&<D::Item as SearchableListItem>::Value> {
        self.state.selection.first().map(|(_, i)| i.value())
    }

    /// Focus the select trigger input.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.state.focus_handle.focus(window, cx);
    }

    fn on_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.list.read(cx).is_focused(window, cx)
            || self.state.focus_handle.is_focused(window)
        {
            return;
        }

        let committed_ix = self.state.selection.first().map(|(ix, _)| *ix);
        if self.selected_index(cx) != committed_ix {
            self.state.list.update(cx, |list, cx| {
                list.set_selected_index(committed_ix, window, cx);
            });
        }

        self.set_open(false, cx);
        cx.notify();
    }

    fn toggle_menu(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();

        self.set_open(!self.state.open, cx);

        if self.state.open {
            self.state.list.focus_handle(cx).focus(window, cx);
        }

        cx.notify();
    }

    fn escape(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.open {
            cx.propagate();
            return;
        }

        cx.stop_propagation();
        self.set_open(false, cx);
        self.focus(window, cx);
        cx.notify();
    }

    /// Tab handler bound in the Select context. When the menu is open, commits
    /// the highlighted item, closes the menu, and advances focus to the next
    /// element. When the menu is closed, re-dispatches the standard `Tab`
    /// action so Root's focus-trap-aware navigation runs unchanged.
    fn select_tab(&mut self, _: &SelectTab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab_navigate(false, window, cx);
    }

    fn select_tab_prev(&mut self, _: &SelectTabPrev, window: &mut Window, cx: &mut Context<Self>) {
        self.tab_navigate(true, window, cx);
    }

    fn tab_navigate(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        let tab_action = move || -> Box<dyn gpui::Action> {
            if backwards {
                Box::new(crate::root::TabPrev)
            } else {
                Box::new(crate::root::Tab)
            }
        };

        if !self.state.open {
            // Closed: defer to Root's normal Tab handling so focus traps work.
            window.dispatch_action(tab_action(), cx);
            return;
        }

        // Open: confirm any current highlight (a no-op when nothing is
        // selected), then force-close the menu and put focus back on the
        // trigger so the state is consistent. `confirm_selection` schedules
        // its own deferred close, which would otherwise restore focus to the
        // trigger after we moved it; closing and re-focusing inline here makes
        // that deferred close idempotent.
        self.state.list.update(cx, |list, cx| {
            list.confirm_selection(window, cx);
        });
        self.set_open(false, cx);
        self.focus(window, cx);

        // Advance focus after the current effect cycle, so it runs after
        // `confirm_selection`'s deferred close-and-restore-focus (queued
        // first). Going through `crate::root::Tab` keeps the dialog focus trap
        // behavior intact.
        let window_handle = window.window_handle();
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                window.dispatch_action(tab_action(), cx);
            });
        });
    }

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.state.open = open;
        self.state.deferred_context = open.then(|| GlobalState::register_deferred_popover(cx));

        cx.notify();
    }

    fn clean(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.set_selected_index(None, window, cx);
        cx.emit(SelectEvent::Confirm(None));
    }

    /// Build the trigger element for a single item: its custom
    /// [`SearchableListItem::display_title`] when it provides one, otherwise
    /// its plain title with any `title_prefix` applied.
    fn item_title(&self, item: &D::Item, window: &mut Window, cx: &mut App) -> AnyElement {
        if let Some(el) = item.display_title(window, cx) {
            el
        } else if let Some(prefix) = self.title_prefix.as_ref() {
            format!("{}{}", prefix, item.title()).into_any_element()
        } else {
            item.title().into_any_element()
        }
    }

    /// Every item the delegate currently holds, cloned out so callers can pass
    /// `&mut App` to the implementor without holding a borrow on the list.
    fn all_items(&self, cx: &App) -> Vec<D::Item> {
        let list = self.state.list.read(cx);
        let delegate = &list.delegate().delegate;
        let mut items = Vec::new();
        for section in 0..delegate.sections_count(cx) {
            for row in 0..delegate.items_count(section) {
                if let Some(item) = delegate.item(IndexPath::default().section(section).row(row)) {
                    items.push(item.clone());
                }
            }
        }
        items
    }

    /// Zero-height, clipped copies of every item's title, stacked in a column.
    ///
    /// A flex column's intrinsic width is the widest of its children, so these
    /// pin the trigger to the width of its widest item while contributing no
    /// height and painting nothing (`h_0` + `overflow_hidden` clips them out).
    /// That gives native-`<select>` sizing without measuring any text or
    /// assuming anything about the trigger's own padding, gaps, or icons.
    /// See [`Select::fit_items`].
    fn fit_sizer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.all_items(cx);
        let mut column = v_flex().h_0().overflow_hidden();
        for item in &items {
            column = column.child(self.item_title(item, window, cx));
        }
        column
    }

    fn display_title(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let default_title = div().text_color(cx.theme().muted_foreground).child(
            self.state
                .placeholder
                .clone()
                .unwrap_or_else(|| t!("Select.placeholder").into()),
        );

        let Some(selected_index) = self.selected_index(cx) else {
            return default_title;
        };

        // Clone the item out before calling `display_title` so we don't hold a borrow on
        // `self.state.list` while passing `&mut App` (via `cx`) to the implementor.
        let item = self
            .state
            .list
            .read(cx)
            .delegate()
            .delegate
            .item(selected_index)
            .cloned();
        let Some(item) = item else {
            return default_title;
        };

        let title = self.item_title(&item, window, cx);

        div()
            .when(self.state.disabled, |this| {
                this.text_color(cx.theme().muted_foreground)
            })
            .child(title)
    }
}

impl<D> Render for SelectState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let searchable = self.searchable;
        let is_focused = self.state.focus_handle.is_focused(window);
        let show_clean = self.state.cleanable && self.selected_index(cx).is_some();
        let bounds = self.state.bounds;
        let allow_open = !(self.state.open || self.state.disabled);
        let outline_visible = self.state.open || (is_focused && !self.state.disabled);

        let (bg, fg) = input_style(self.state.disabled, cx);

        self.state.list.update(cx, |list, cx| {
            list.set_searchable(searchable, cx);
            list.delegate_mut().size = self.state.size;
        });

        div().relative().child(
            div()
                .relative()
                .on_prepaint({
                    let state = cx.entity();
                    move |bounds, _, cx| state.update(cx, |r, _| r.state.bounds = bounds)
                })
                .child(
                    div()
                        .id("input")
                        .relative()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_1()
                        .border_color(cx.theme().transparent)
                        .when(self.state.appearance, |this| {
                            this.bg(bg)
                                .text_color(fg)
                                .when(self.state.disabled, |this| this.opacity(0.5))
                                .border_color(cx.theme().input)
                                .rounded(cx.theme().radius)
                        })
                        .input_size(self.state.size)
                        .input_text_size(self.state.size)
                        .refine_style(&self.state.style)
                        .when(outline_visible && self.state.appearance, |this| {
                            this.border_1().border_color(cx.theme().ring)
                        })
                        .when(
                            outline_visible && self.state.appearance && self.focus_ring_enabled,
                            |this| this.focus_ring_style(window, cx),
                        )
                        .when(allow_open, |this| {
                            this.on_click(cx.listener(Self::toggle_menu))
                        })
                        .child(
                            h_flex()
                                .id("inner")
                                .w_full()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .items_center()
                                .justify_between()
                                .gap_1()
                                .child({
                                    // With `fit_items`, the hidden sizer goes
                                    // in first so the slot (a column) is as
                                    // wide as the widest item; the visible
                                    // title sits under it and takes the whole
                                    // height.
                                    let sizer = self.fit_items.then(|| {
                                        self.fit_sizer(window, cx).into_any_element()
                                    });
                                    div()
                                        .id("title")
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .truncate()
                                        .when(sizer.is_some(), |this| this.flex().flex_col())
                                        .children(sizer)
                                        .child(self.display_title(window, cx))
                                })
                                .when(show_clean, |this| {
                                    this.child(clear_button(None, cx).map(|this| {
                                        if self.state.disabled {
                                            this.disabled(true)
                                        } else {
                                            this.on_click(cx.listener(Self::clean))
                                        }
                                    }))
                                })
                                .when(!show_clean, |this| {
                                    let icon = match self.icon.clone() {
                                        Some(icon) => icon
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground)
                                            .into_any_element(),
                                        None => Caret::new(self.state.size)
                                            .text_color(cx.theme().muted_foreground)
                                            .into_any_element(),
                                    };

                                    this.child(icon)
                                }),
                        ),
                )
                .when(self.state.open, |this| {
                    this.child(
                        deferred(crate::popover::dropdown_popup(
                            ("select-popup", cx.entity_id()),
                            bounds,
                            v_flex()
                                .occlude()
                                .map(|this| match self.state.menu_width {
                                    Length::Auto => this.w(bounds.size.width + px(2.)),
                                    Length::Definite(w) => this.w(w),
                                })
                                .popover_style(cx)
                                .child(
                                    List::new(&self.state.list)
                                        .when_some(
                                            self.state.search_placeholder.clone(),
                                            |this, placeholder| {
                                                this.search_placeholder(placeholder)
                                            },
                                        )
                                        .when_some(self.search_icon.clone(), |this, icon| {
                                            this.search_icon(icon)
                                        })
                                        .with_size(self.state.size)
                                        .max_h(self.state.menu_max_h)
                                        .paddings(Edges::all(px(4.))),
                                )
                                .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                                    this.escape(&Cancel, window, cx);
                                })),
                            cx,
                        ))
                        .with_priority(gpui_base::POPUP_PRIORITY),
                    )
                }),
        )
    }
}

impl<D> Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    pub fn new(state: &Entity<SelectState<D>>) -> Self {
        Self {
            id: ("select", state.entity_id()).into(),
            state: state.clone(),
            options: SelectOptions::default(),
            empty: None,
        }
    }

    /// Size the trigger to its widest item rather than to the current
    /// selection, the way a native `<select>` sizes to its widest `<option>`.
    /// Default: `false`.
    ///
    /// Without this, a `Select` given no explicit width sizes to whatever it
    /// happens to be showing: the trigger resizes every time the selection
    /// changes, and because an `Length::Auto` menu copies the trigger's width,
    /// selecting a short item leaves the open menu too narrow for its own
    /// longer rows. Passing an explicit `w()` avoids both, but only if the
    /// caller hardcodes a width large enough for the longest item, which has
    /// to be re-guessed whenever the font, font size, or item text changes.
    ///
    /// This measures nothing. It lays out a zero-height, clipped copy of every
    /// item's title inside the trigger, so the trigger's intrinsic width is the
    /// widest item's actual laid-out width, including whatever custom element
    /// [`SearchableListItem::display_title`] returns. Combine with `min_w` /
    /// `max_w` to bound it; an explicit `w()` still wins outright.
    ///
    /// Costs one extra layout pass per item per frame, so it suits small,
    /// fixed option sets rather than long or searchable lists. It sizes to the
    /// delegate's current items, so a filtered searchable list sizes to the
    /// filtered set.
    pub fn fit_items(mut self, fit: bool) -> Self {
        self.options.fit_items = fit;
        self
    }

    /// Set the width of the dropdown menu, default: `Length::Auto`.
    pub fn menu_width(mut self, width: impl Into<Length>) -> Self {
        self.options.menu_width = width.into();
        self
    }

    /// Set the max height of the dropdown menu, default: 20rem.
    pub fn menu_max_h(mut self, max_h: impl Into<Length>) -> Self {
        self.options.menu_max_h = max_h.into();
        self
    }

    /// Set the placeholder shown when no value is selected.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.options.placeholder = Some(placeholder.into());
        self
    }

    /// Set the name a screen reader announces for the select.
    ///
    /// The placeholder and selected value are not used as the accessible name,
    /// because they describe the current value rather than the control itself.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.options.accessibility_label = Some(label.into());
        self
    }

    /// Override the trailing icon, replacing the default chevron.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.options.icon = Some(icon.into());
        self
    }

    /// Set a label prefix shown before the selected title in the trigger.
    ///
    /// e.g. `title_prefix("Country: ")` → "Country: United States"
    pub fn title_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.options.title_prefix = Some(prefix.into());
        self
    }

    /// Show a clear button when a value is selected.
    pub fn cleanable(mut self, cleanable: bool) -> Self {
        self.options.cleanable = cleanable;
        self
    }

    /// Set the placeholder text for the search input.
    pub fn search_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.options.search_placeholder = Some(placeholder.into());
        self
    }

    /// Set the icon for the search input, default is `IconName::Search`.
    pub fn search_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.options.search_icon = Some(icon.into());
        self
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.options.disabled = disabled;
        self
    }

    /// Set a custom closure that renders the empty-state element.
    pub fn empty<E: IntoElement + 'static>(
        mut self,
        builder: impl Fn(&mut Window, &App) -> E + 'static,
    ) -> Self {
        self.empty = Some(Box::new(move |window, cx| {
            builder(window, cx).into_any_element()
        }));
        self
    }

    /// Control whether the trigger shows a border and background (`true` by default).
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.options.appearance = appearance;
        self
    }
}

impl<D> Sizable for Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.options.size = size.into();
        self
    }
}

impl<D> crate::FocusableExt for Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn focus_ring(mut self, enabled: bool) -> Self {
        self.options.focus_ring_enabled = enabled;
        self
    }

    fn is_focus_ring_enabled(&self) -> bool {
        self.options.focus_ring_enabled
    }
}

impl<D> EventEmitter<SelectEvent<D>> for SelectState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
}

impl<D> EventEmitter<DismissEvent> for SelectState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
}

impl<D> Focusable for SelectState<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.state.open {
            self.state.list.focus_handle(cx)
        } else {
            self.state.focus_handle.clone()
        }
    }
}

impl<D> Styled for Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.options.style
    }
}

impl<D> RenderOnce for Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.options.disabled;
        let accessibility_label = self.options.accessibility_label.clone();
        // Always track the trigger's focus handle, never the list/query handle.
        // The `Focusable` impl returns the list handle while the menu is open,
        // but tracking that one is unsafe: when an ancestor view re-uses its
        // cached prepaint, the cached div replays a `track_focus(list_handle)`
        // pointing at an element no longer in the dispatch tree, leaving
        // keyboard focus stranded once the menu closes.
        let focus_handle = self.state.read(cx).state.focus_handle.clone();
        let empty = self.empty;
        let opts = self.options;

        self.state.update(cx, |this, _| {
            this.state.style = opts.style;
            this.state.size = opts.size;
            this.state.cleanable = opts.cleanable;
            this.state.placeholder = opts.placeholder;
            this.state.search_placeholder = opts.search_placeholder;
            this.state.menu_width = opts.menu_width;
            this.state.menu_max_h = opts.menu_max_h;
            this.state.disabled = opts.disabled;
            this.state.appearance = opts.appearance;
            this.focus_ring_enabled = opts.focus_ring_enabled;
            this.fit_items = opts.fit_items;
            this.icon = opts.icon;
            this.title_prefix = opts.title_prefix;
            this.search_icon = opts.search_icon;

            if let Some(empty) = empty {
                this.state.empty = Some(empty);
            }
        });

        let is_open = self.state.read(cx).state.open;
        let content_focus_handle = self.state.read(cx).state.list.focus_handle(cx);
        let open_state = self.state.clone();

        // The Tab handlers sit on a wrapper: `BaseSelect` owns the key context
        // and the focus handle, and the actions bubble out to here.
        div()
            .on_action(window.listener_for(&self.state, SelectState::select_tab))
            .on_action(window.listener_for(&self.state, SelectState::select_tab_prev))
            .child(
                BaseSelect::new(self.id)
                    .open(is_open)
                    .disabled(disabled)
                    .when_some(accessibility_label, |this, label| {
                        this.accessibility_label(label)
                    })
                    .focus_handle(&focus_handle)
                    .content_focus_handle(&content_focus_handle)
                    .on_open_change(move |open, _, cx| {
                        open_state.update(cx, |state, cx| state.set_open(open, cx));
                    })
                    .child(self.state),
            )
    }
}

// MARK: Tests

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};

    use crate::{
        IndexPath,
        searchable_list::{SearchableListDelegate as _, SearchableVec},
        select::{Select, SelectGroup, SelectState},
    };

    #[gpui::test]
    fn an_explicit_accessibility_label_does_not_replace_the_placeholder(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| SelectState::new(items, None, window, cx));

            let plain = Select::new(&state).placeholder("Choose a language");
            assert_eq!(plain.options.accessibility_label, None);
            assert_eq!(
                plain.options.placeholder.as_deref(),
                Some("Choose a language")
            );

            let named = Select::new(&state)
                .placeholder("Choose a language")
                .accessibility_label("Programming language");
            assert_eq!(
                named.options.accessibility_label.as_deref(),
                Some("Programming language")
            );
            assert_eq!(
                named.options.placeholder.as_deref(),
                Some("Choose a language"),
                "an accessible name must not change what is drawn"
            );
        });
    }

    #[gpui::test]
    fn test_select_initial_selection_seeds_cursor(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| SelectState::new(items, Some(IndexPath::new(1)), window, cx));

            assert_eq!(
                state.read(cx).selected_index(cx),
                Some(IndexPath::new(1)),
                "initial cursor should be seeded on ListState so display_title can read it",
            );
            assert_eq!(state.read(cx).selected_value(), Some(&"Go"));
        });
    }

    #[gpui::test]
    fn test_select_initial_grouped_selection_seeds_cursor(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let mut groups: SearchableVec<SelectGroup<&'static str>> = SearchableVec::new(vec![]);
            groups.push(SelectGroup::new("A").items(["Apple", "Avocado"]));
            groups.push(SelectGroup::new("B").items(["Banana", "Blueberry", "Blackberry"]));

            let initial = IndexPath::new(1).section(1);
            let state = cx.new(|cx| SelectState::new(groups, Some(initial), window, cx));

            assert_eq!(state.read(cx).selected_index(cx), Some(initial));
            assert_eq!(state.read(cx).selected_value(), Some(&"Blueberry"));
        });
    }

    #[gpui::test]
    fn test_select_set_selected_value_clears_search_query(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let items = SearchableVec::new(vec!["Rust", "Go", "C++"]);
            let state = cx.new(|cx| SelectState::new(items, None, window, cx).searchable(true));
            let list = state.read(cx).state.list.clone();

            list.update(cx, |list, cx| list.set_query("Rust", window, cx));
            assert_eq!(list.read(cx).delegate().delegate.items_count(0), 1);

            state.update(cx, |state, cx| {
                state.set_selected_value(&"Go", window, cx);
            });

            assert_eq!(state.read(cx).selected_value(), Some(&"Go"));
            assert_eq!(state.read(cx).selected_index(cx), Some(IndexPath::new(1)));
            assert_eq!(list.read(cx).query_input.read(cx).value(), "");
        });
    }

    #[gpui::test]
    fn test_select_set_selected_value_clears_grouped_search_query(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let mut groups: SearchableVec<SelectGroup<&'static str>> = SearchableVec::new(vec![]);
            groups.push(SelectGroup::new("A").items(["Apple", "Avocado"]));
            groups.push(SelectGroup::new("B").items(["Banana", "Blueberry"]));

            let state = cx.new(|cx| SelectState::new(groups, None, window, cx).searchable(true));
            let list = state.read(cx).state.list.clone();

            list.update(cx, |list, cx| list.set_query("Blue", window, cx));
            state.update(cx, |state, cx| {
                state.set_selected_value(&"Banana", window, cx);
            });

            assert_eq!(state.read(cx).selected_value(), Some(&"Banana"));
            assert_eq!(
                state.read(cx).selected_index(cx),
                Some(IndexPath::new(0).section(1)),
            );
        });
    }
}
