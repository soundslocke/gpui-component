use std::{rc::Rc, sync::Arc};

use crate::ThemeStyled as _;
use crate::{
    ActiveTheme, AxisExt, Sizable, Size, StyledExt, checkbox::checkbox_check_icon, h_flex,
    text::Text, tooltip::ComponentTooltip, v_flex,
};
use gpui::{
    AnyElement, App, Axis, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, relative, rems,
};
use gpui_base::{Radio as BaseRadio, RadioGroup as BaseRadioGroup};

/// A Radio element.
///
/// This is not included the Radio group implementation, you can manage the group by yourself.
#[derive(IntoElement)]
pub struct Radio {
    base: BaseRadio,
    style: StyleRefinement,
    id: ElementId,
    label: Option<Text>,
    /// The announced name, when the visible label is not it.
    accessibility_label: Option<SharedString>,
    children: Vec<AnyElement>,
    checked: bool,
    disabled: bool,
    tab_stop: bool,
    tab_index: isize,
    size: Size,
    on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    tooltip: ComponentTooltip,
    position_in_set: Option<usize>,
    size_of_set: Option<usize>,
    focus_ring_enabled: bool,
    /// Supplied by [`RadioGroup`], which owns its options' handles so that it
    /// can move focus between them. A standalone Radio mints its own.
    focus_handle: Option<FocusHandle>,
}

impl Radio {
    /// Create a new Radio element with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: BaseRadio::new(id.clone()),
            id,
            style: StyleRefinement::default(),
            label: None,
            accessibility_label: None,
            children: Vec::new(),
            checked: false,
            disabled: false,
            tab_index: 0,
            tab_stop: true,
            size: Size::default(),
            on_click: None,
            tooltip: ComponentTooltip::default(),
            position_in_set: None,
            size_of_set: None,
            focus_handle: None,
            focus_ring_enabled: true,
        }
    }

    /// Set tooltip text for the radio.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip.text = Some((tooltip.into(), None));
        self
    }

    /// Set the label of the Radio element.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the name a screen reader announces, when the visible label is not
    /// it.
    ///
    /// A radio's name comes from its [`label`](Self::label) by default. Setting
    /// this replaces the announced name without changing what is displayed.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Set the checked state of the Radio element, default is `false`.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Set the disabled state of the Radio element, default is `false`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the tab index for the Radio element, default is `0`.
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// Set the tab stop for the Radio element, default is `true`.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Add on_click handler when the Radio is clicked.
    ///
    /// The `&bool` parameter is the **new checked state**.
    pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Sizable for Radio {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl crate::FocusableExt for Radio {
    fn focus_ring(mut self, enabled: bool) -> Self {
        self.focus_ring_enabled = enabled;
        self
    }

    fn is_focus_ring_enabled(&self) -> bool {
        self.focus_ring_enabled
    }
}

impl Styled for Radio {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Radio {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for Radio {}

impl ParentElement for Radio {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Radio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.checked;
        let focus_handle = self.focus_handle.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        let is_focused = focus_handle.is_focused(window);
        let disabled = self.disabled;
        let accessibility_label = self
            .accessibility_label
            .clone()
            .or_else(|| self.label.as_ref().map(|label| label.get_text(cx)));

        let (border_color, bg) = if checked {
            (cx.theme().primary, cx.theme().primary)
        } else {
            (cx.theme().input, cx.theme().input.opacity(0.5))
        };
        let (border_color, bg) = if disabled {
            (border_color.opacity(0.5), bg.opacity(0.5))
        } else {
            (border_color, bg)
        };

        self.base
            .id(self.id.clone())
            .checked(self.checked)
            .disabled(self.disabled)
            .track_focus(&focus_handle)
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index)
            .when_some(accessibility_label, |this, label| {
                this.accessibility_label(label)
            })
            .when_some(
                self.position_in_set.zip(self.size_of_set),
                |this, (position, size)| this.set_position(position, size),
            )
            .h_flex()
            .gap_x_2()
            .text_color(cx.theme().foreground)
            .items_start()
            .line_height(relative(1.))
            .rounded(cx.theme().radius * 0.5)
            .when(is_focused && self.focus_ring_enabled, |this| {
                this.focus_ring_style(window, cx)
            })
            .map(|this| match self.size {
                Size::XSmall => this.text_xs(),
                Size::Small => this.text_sm(),
                Size::Medium => this.text_base(),
                Size::Large => this.text_lg(),
                _ => this,
            })
            .refine_style(&self.style)
            .child(
                div()
                    .relative()
                    .map(|this| match self.size {
                        Size::XSmall => this.size_3(),
                        Size::Small => this.size_3p5(),
                        Size::Medium => this.size_4(),
                        Size::Large => this.size(rems(1.125)),
                        _ => this.size_4(),
                    })
                    .flex_shrink_0()
                    .rounded_full_style(cx)
                    .border_1()
                    .border_color(border_color)
                    .map(|this| match self.checked {
                        false => this.bg(cx.theme().input_background()),
                        true if disabled => this.bg(bg),
                        true => this.bg(cx.theme().tokens.primary),
                    })
                    .child(checkbox_check_icon(
                        self.id, self.size, checked, disabled, window, cx,
                    )),
            )
            .when(!self.children.is_empty() || self.label.is_some(), |this| {
                this.child(
                    v_flex()
                        .w_full()
                        .line_height(relative(1.2))
                        .gap_1()
                        .when_some(self.label, |this, label| {
                            this.child(
                                div()
                                    .size_full()
                                    .line_height(relative(1.))
                                    .when(self.disabled, |this| {
                                        this.text_color(cx.theme().muted_foreground)
                                    })
                                    .child(label),
                            )
                        })
                        .children(self.children),
                )
            })
            // Focus deliberately follows the click (GPUI's `div` does it for
            // any tracked handle): the ring stays hidden while the pointer is
            // driving, so the only effect is that a later Tab or arrow key
            // continues from the option the user actually picked.
            .when_some(self.on_click.clone(), |this, on_click| {
                this.on_change(move |next, _, window, cx| on_click(&next, window, cx))
            })
            .map(|this| self.tooltip.apply(this))
    }
}

/// A Radio group element.
#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    style: StyleRefinement,
    radios: Vec<Radio>,
    layout: Axis,
    selected_index: Option<usize>,
    disabled: bool,
    on_click: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

impl RadioGroup {
    fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default().flex_1(),
            on_click: None,
            layout: Axis::Vertical,
            selected_index: None,
            disabled: false,
            radios: vec![],
        }
    }

    /// Create a new Radio group with default Vertical layout.
    pub fn vertical(id: impl Into<ElementId>) -> Self {
        Self::new(id)
    }

    /// Create a new Radio group with Horizontal layout.
    pub fn horizontal(id: impl Into<ElementId>) -> Self {
        Self::new(id).layout(Axis::Horizontal)
    }

    /// Set the layout of the Radio group. Default is `Axis::Vertical`.
    pub fn layout(mut self, layout: Axis) -> Self {
        self.layout = layout;
        self
    }

    // Add on_click handler when selected index changes.
    //
    // The `&usize` parameter is the selected index.
    pub fn on_click(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Set the selected index.
    pub fn selected_index(mut self, index: Option<usize>) -> Self {
        self.selected_index = index;
        self
    }

    /// Set the disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Add a child Radio element.
    pub fn child(mut self, child: impl Into<Radio>) -> Self {
        self.radios.push(child.into());
        self
    }

    /// Add multiple child Radio elements.
    pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Radio>>) -> Self {
        self.radios.extend(children.into_iter().map(Into::into));
        self
    }
}

impl Styled for RadioGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl From<&'static str> for Radio {
    fn from(label: &'static str) -> Self {
        Self::new(label).label(label)
    }
}

impl From<SharedString> for Radio {
    fn from(label: SharedString) -> Self {
        Self::new(label.clone()).label(label)
    }
}

impl From<String> for Radio {
    fn from(label: String) -> Self {
        Self::new(SharedString::from(label.clone())).label(SharedString::from(label))
    }
}

/// The direction an unmodified arrow key moves selection within a group.
///
/// Both axes are accepted whatever the group's layout, matching how browsers
/// treat a radio group.
fn arrow_step(event: &KeyDownEvent) -> Option<isize> {
    if event.keystroke.modifiers.modified() {
        return None;
    }

    match event.keystroke.key.as_str() {
        "down" | "right" => Some(1),
        "up" | "left" => Some(-1),
        _ => None,
    }
}

/// Walk from `from` in the `step` direction, wrapping around the ends, to the
/// next option that can take the selection. `None` when there is no other one.
fn next_selectable(from: usize, step: isize, selectable: &[bool]) -> Option<usize> {
    let total = selectable.len();
    let mut ix = from;
    for _ in 0..total {
        ix = (ix as isize + step).rem_euclid(total as isize) as usize;
        if selectable[ix] {
            return (ix != from).then_some(ix);
        }
    }

    None
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click;
        let disabled = self.disabled;
        let selected_ix = self.selected_index;
        let total = self.radios.len();

        // The group owns its options' focus handles so that arrow keys can
        // move focus along with the selection.
        let focus_handles: Vec<FocusHandle> = (0..total)
            .map(|ix| {
                let key = ElementId::NamedChild(
                    Arc::new(self.id.clone()),
                    SharedString::from(ix.to_string()),
                );
                window
                    .use_keyed_state(key, cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
            .collect();

        let selectable: Vec<bool> = self
            .radios
            .iter()
            .map(|radio| !disabled && !radio.disabled)
            .collect();

        // Roving tab stop: the group is a single stop in the tab order, and
        // tabbing into it lands on the selected option rather than the first.
        let tab_stop_ix = selected_ix
            .filter(|ix| selectable.get(*ix).copied().unwrap_or(false))
            .or_else(|| selectable.iter().position(|can_select| *can_select));

        let base = if self.layout.is_vertical() {
            v_flex()
        } else {
            h_flex().w_full().flex_wrap()
        };

        let radio_handles = focus_handles.clone();
        BaseRadioGroup::new(self.id)
            .axis(self.layout)
            .refine_style(&self.style)
            .when_some(on_click.clone(), |this, on_click| {
                let focus_handles = focus_handles.clone();
                let selectable = selectable.clone();
                // Arrow keys move the selection, taking focus with it, so focus
                // never sits on an option the user did not pick.
                this.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let Some(step) = arrow_step(event) else {
                        return;
                    };
                    let Some(current) = focus_handles.iter().position(|h| h.is_focused(window))
                    else {
                        return;
                    };
                    let Some(next) = next_selectable(current, step, &selectable) else {
                        return;
                    };

                    cx.stop_propagation();
                    focus_handles[next].focus(window, cx);
                    on_click(&next, window, cx);
                })
            })
            .child(
                base.gap_3()
                    .children(self.radios.into_iter().enumerate().map(|(ix, mut radio)| {
                        let checked = selected_ix == Some(ix);

                        radio.id = ix.into();
                        radio.position_in_set = Some(ix + 1);
                        radio.size_of_set = Some(total);
                        radio.focus_handle = radio_handles.get(ix).cloned();
                        radio
                            .tab_stop(tab_stop_ix == Some(ix))
                            .disabled(disabled)
                            .checked(checked)
                            .when_some(on_click.clone(), |this, on_click| {
                                this.on_click(move |_, window, cx| on_click(&ix, window, cx))
                            })
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_accessibility_label_replaces_the_visible_one() {
        let plain = Radio::new("automatic").label("Automatic");
        assert_eq!(plain.accessibility_label, None);
        assert!(matches!(
            &plain.label,
            Some(Text::String(label)) if label.as_ref() == "Automatic"
        ));

        let named = Radio::new("automatic")
            .label("Automatic")
            .accessibility_label("Choose automatic mode");
        assert_eq!(
            named.accessibility_label.as_deref(),
            Some("Choose automatic mode"),
            "an explicit name must win over the visible label"
        );
        assert!(
            matches!(
                &named.label,
                Some(Text::String(label)) if label.as_ref() == "Automatic"
            ),
            "and must not change what is drawn"
        );
    }
}
