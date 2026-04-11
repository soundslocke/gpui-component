use std::ops::Range;

use crate::{
    ActiveTheme, AxisExt, ElementExt, FocusableExt as _, StyledExt, h_flex,
    actions::{
        SelectDown, SelectFirst, SelectLast, SelectLeft, SelectPageDown, SelectPageUp, SelectRight,
        SelectUp,
    },
};
use gpui::{
    Along, App, AppContext as _, Axis, Background, Bounds, Context, Corners, DefiniteLength,
    DragMoveEvent, Empty, Entity, EntityId, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, IsZero, KeyBinding, MouseButton, MouseDownEvent,
    ParentElement as _, Pixels, Point, Render, RenderOnce, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

const CONTEXT: &str = "Slider";

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("home", SelectFirst, Some(CONTEXT)),
        KeyBinding::new("end", SelectLast, Some(CONTEXT)),
        KeyBinding::new("pageup", SelectPageUp, Some(CONTEXT)),
        KeyBinding::new("pagedown", SelectPageDown, Some(CONTEXT)),
    ]);
}

#[derive(Clone)]
struct DragThumb((EntityId, bool));

impl Render for DragThumb {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[derive(Clone)]
struct DragSlider(EntityId);

impl Render for DragSlider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Events emitted by the [`SliderState`].
pub enum SliderEvent {
    Change(SliderValue),
}

/// The value of the slider, can be a single value or a range of values.
///
/// - Can from a f32 value, which will be treated as a single value.
/// - Or from a (f32, f32) tuple, which will be treated as a range of values.
///
/// The default value is `SliderValue::Single(0.0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
    Single(f32),
    Range(f32, f32),
}

impl std::fmt::Display for SliderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SliderValue::Single(value) => write!(f, "{}", value),
            SliderValue::Range(start, end) => write!(f, "{}..{}", start, end),
        }
    }
}

impl From<f32> for SliderValue {
    fn from(value: f32) -> Self {
        SliderValue::Single(value)
    }
}

impl From<(f32, f32)> for SliderValue {
    fn from(value: (f32, f32)) -> Self {
        SliderValue::Range(value.0, value.1)
    }
}

impl From<Range<f32>> for SliderValue {
    fn from(value: Range<f32>) -> Self {
        SliderValue::Range(value.start, value.end)
    }
}

impl Default for SliderValue {
    fn default() -> Self {
        SliderValue::Single(0.)
    }
}

impl SliderValue {
    /// Clamp the value to the given range.
    pub fn clamp(self, min: f32, max: f32) -> Self {
        match self {
            SliderValue::Single(value) => SliderValue::Single(value.clamp(min, max)),
            SliderValue::Range(start, end) => {
                SliderValue::Range(start.clamp(min, max), end.clamp(min, max))
            }
        }
    }

    /// Check if the value is a single value.
    #[inline]
    pub fn is_single(&self) -> bool {
        matches!(self, SliderValue::Single(_))
    }

    /// Check if the value is a range of values.
    #[inline]
    pub fn is_range(&self) -> bool {
        matches!(self, SliderValue::Range(_, _))
    }

    /// Get the start value.
    pub fn start(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(start, _) => *start,
        }
    }

    /// Get the end value.
    pub fn end(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(_, end) => *end,
        }
    }

    fn set_start(&mut self, value: f32) {
        if let SliderValue::Range(_, end) = self {
            *self = SliderValue::Range(value.min(*end), *end);
        } else {
            *self = SliderValue::Single(value);
        }
    }

    fn set_end(&mut self, value: f32) {
        if let SliderValue::Range(start, _) = self {
            *self = SliderValue::Range(*start, value.max(*start));
        } else {
            *self = SliderValue::Single(value);
        }
    }
}

/// The scale mode of the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
    /// Linear scale where values change uniformly across the slider range.
    /// This is the default mode.
    #[default]
    Linear,
    /// Logarithmic scale where the distance between values increases exponentially.
    ///
    /// This is useful for parameters that have a large range of values where smaller
    /// changes are more significant at lower values. Common examples include:
    ///
    /// - Volume controls (human hearing perception is logarithmic)
    /// - Frequency controls (musical notes follow a logarithmic scale)
    /// - Zoom levels
    /// - Any parameter where you want finer control at lower values
    ///
    /// # For example
    ///
    /// ```ignore
    /// use gpui_component::slider::{SliderState, SliderScale};
    ///
    /// let slider = SliderState::new(cx)
    ///     .min(1.0)    // Must be > 0 for logarithmic scale
    ///     .max(1000.0)
    ///     .scale(SliderScale::Logarithmic);
    /// ```
    ///
    /// - Moving the slider 1/3 of the way will yield ~10
    /// - Moving it 2/3 of the way will yield ~100
    /// - The full range covers 3 orders of magnitude evenly
    Logarithmic,
}

impl SliderScale {
    #[inline]
    pub fn is_linear(&self) -> bool {
        matches!(self, SliderScale::Linear)
    }

    #[inline]
    pub fn is_logarithmic(&self) -> bool {
        matches!(self, SliderScale::Logarithmic)
    }
}

/// State of the [`Slider`].
pub struct SliderState {
    min: f32,
    max: f32,
    step: f32,
    value: SliderValue,
    /// When is single value mode, only `end` is used, the start is always 0.0.
    percentage: Range<f32>,
    /// The bounds of the slider after rendered.
    bounds: Bounds<Pixels>,
    scale: SliderScale,
    focus_handle: FocusHandle,
}

impl SliderState {
    /// Create a new [`SliderState`].
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: SliderValue::default(),
            percentage: (0.0..0.0),
            bounds: Bounds::default(),
            scale: SliderScale::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the minimum value of the slider, default: 0.0
    pub fn min(mut self, min: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                min > 0.0,
                "`min` must be greater than 0 for SliderScale::Logarithmic"
            );
            assert!(
                min < self.max,
                "`min` must be less than `max` for Logarithmic scale"
            );
        }
        self.min = min;
        self.update_thumb_pos();
        self
    }

    /// Set the maximum value of the slider, default: 100.0
    pub fn max(mut self, max: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.max = max;
        self.update_thumb_pos();
        self
    }

    /// Set the step value of the slider, default: 1.0
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Set the scale of the slider, default: [`SliderScale::Linear`].
    pub fn scale(mut self, scale: SliderScale) -> Self {
        if scale.is_logarithmic() {
            assert!(
                self.min > 0.0,
                "`min` must be greater than 0 for Logarithmic scale"
            );
            assert!(
                self.max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.scale = scale;
        self.update_thumb_pos();
        self
    }

    /// Set the default value of the slider, default: 0.0
    pub fn default_value(mut self, value: impl Into<SliderValue>) -> Self {
        self.value = value.into();
        self.update_thumb_pos();
        self
    }

    /// Set the value of the slider.
    pub fn set_value(
        &mut self,
        value: impl Into<SliderValue>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.value = value.into();
        self.update_thumb_pos();
        cx.notify();
    }

    /// Get the value of the slider.
    pub fn value(&self) -> SliderValue {
        self.value
    }

    /// Converts a value between 0.0 and 1.0 to a value between the minimum and maximum value,
    /// depending on the chosen scale.
    fn percentage_to_value(&self, percentage: f32) -> f32 {
        match self.scale {
            SliderScale::Linear => self.min + (self.max - self.min) * percentage,
            SliderScale::Logarithmic => {
                // when percentage is 0, this simplifies to (max/min)^0 * min = 1 * min = min
                // when percentage is 1, this simplifies to (max/min)^1 * min = (max*min)/min = max
                // we clamp just to make sure we don't have issue with floating point precision
                let base = self.max / self.min;
                (base.powf(percentage) * self.min).clamp(self.min, self.max)
            }
        }
    }

    /// Converts a value between the minimum and maximum value to a value between 0.0 and 1.0,
    /// depending on the chosen scale.
    fn value_to_percentage(&self, value: f32) -> f32 {
        match self.scale {
            SliderScale::Linear => {
                let range = self.max - self.min;
                if range <= 0.0 {
                    0.0
                } else {
                    (value - self.min) / range
                }
            }
            SliderScale::Logarithmic => {
                let base = self.max / self.min;
                (value / self.min).log(base).clamp(0.0, 1.0)
            }
        }
    }

    fn update_thumb_pos(&mut self) {
        match self.value {
            SliderValue::Single(value) => {
                let percentage = self.value_to_percentage(value.clamp(self.min, self.max));
                self.percentage = 0.0..percentage;
            }
            SliderValue::Range(start, end) => {
                let clamped_start = start.clamp(self.min, self.max);
                let clamped_end = end.clamp(self.min, self.max);
                self.percentage =
                    self.value_to_percentage(clamped_start)..self.value_to_percentage(clamped_end);
            }
        }
    }

    /// Update value by mouse position
    fn update_value_by_position(
        &mut self,
        axis: Axis,
        position: Point<Pixels>,
        is_start: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Match the inset thumb positioning in `Slider::render`: the
        // thumb's center moves between `THUMB_RADIUS` and
        // `bar_size - THUMB_RADIUS`, so a click at the bar's left edge
        // (or anywhere left of `THUMB_RADIUS`) maps to percentage 0,
        // and a click at the right edge maps to percentage 1.
        const THUMB_RADIUS: f32 = 8.0;
        const THUMB_DIAMETER: f32 = 16.0;
        let bounds = self.bounds;
        let step = self.step;

        let pos_along: f32 = if axis.is_horizontal() {
            (position.x - bounds.left()).into()
        } else {
            (bounds.bottom() - position.y).into()
        };
        let total_size: f32 = bounds.size.along(axis).into();
        let inner_size = total_size - THUMB_DIAMETER;
        let percentage = if inner_size > 0.0 {
            ((pos_along - THUMB_RADIUS) / inner_size).clamp(0.0, 1.0)
        } else if total_size > 0.0 {
            (pos_along / total_size).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let percentage = if is_start {
            percentage.clamp(0.0, self.percentage.end)
        } else {
            percentage.clamp(self.percentage.start, 1.0)
        };

        let value = self.percentage_to_value(percentage);
        let value = (value / step).round() * step;

        if is_start {
            self.percentage.start = percentage;
            self.value.set_start(value);
        } else {
            self.percentage.end = percentage;
            self.value.set_end(value);
        }
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
    }
}

impl EventEmitter<SliderEvent> for SliderState {}

impl Focusable for SliderState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SliderState {
    /// Adjust the active thumb's value by `delta`, clamped to the slider's range.
    /// For range sliders, this currently adjusts the end thumb only.
    fn adjust_by(&mut self, delta: f32, _: &mut Window, cx: &mut Context<Self>) {
        let new_value = match self.value {
            SliderValue::Single(v) => {
                SliderValue::Single((v + delta).clamp(self.min, self.max))
            }
            SliderValue::Range(start, end) => {
                // TODO: support keyboard navigation between range thumbs.
                // For now keyboard input always adjusts the end thumb.
                SliderValue::Range(start, (end + delta).clamp(start, self.max))
            }
        };
        self.value = new_value;
        self.update_thumb_pos();
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
    }

    /// Set the active thumb's value to a specific value, clamped to the slider's range.
    fn set_to(&mut self, value: f32, _: &mut Window, cx: &mut Context<Self>) {
        let new_value = match self.value {
            SliderValue::Single(_) => SliderValue::Single(value.clamp(self.min, self.max)),
            SliderValue::Range(start, _) => {
                SliderValue::Range(start, value.clamp(start, self.max))
            }
        };
        self.value = new_value;
        self.update_thumb_pos();
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
    }

    fn on_select_left(
        &mut self,
        _: &SelectLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.adjust_by(-self.step, window, cx);
    }

    fn on_select_right(
        &mut self,
        _: &SelectRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.adjust_by(self.step, window, cx);
    }

    fn on_select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        self.adjust_by(self.step, window, cx);
    }

    fn on_select_down(
        &mut self,
        _: &SelectDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.adjust_by(-self.step, window, cx);
    }

    fn on_select_first(
        &mut self,
        _: &SelectFirst,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_to(self.min, window, cx);
    }

    fn on_select_last(
        &mut self,
        _: &SelectLast,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_to(self.max, window, cx);
    }

    fn on_page_up(
        &mut self,
        _: &SelectPageUp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.adjust_by(self.step * 10.0, window, cx);
    }

    fn on_page_down(
        &mut self,
        _: &SelectPageDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.adjust_by(-self.step * 10.0, window, cx);
    }
}

/// A Slider element.
#[derive(IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    axis: Axis,
    style: StyleRefinement,
    disabled: bool,
    show_fill: bool,
    reverse_fill: bool,
}

impl Slider {
    /// Create a new [`Slider`] element bind to the [`SliderState`].
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            axis: Axis::Horizontal,
            state: state.clone(),
            style: StyleRefinement::default(),
            disabled: false,
            show_fill: true,
            reverse_fill: false,
        }
    }

    /// As a horizontal slider.
    pub fn horizontal(mut self) -> Self {
        self.axis = Axis::Horizontal;
        self
    }

    /// As a vertical slider.
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Vertical;
        self
    }

    /// Set the disabled state of the slider, default: false
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set whether the filled bar from min to the current value is visible.
    /// Default: `true`. Set to `false` for point-selection sliders where
    /// only the thumb position matters.
    pub fn show_fill(mut self, show: bool) -> Self {
        self.show_fill = show;
        self
    }

    /// Reverse the fill direction for single-value mode: fill from the thumb
    /// to the maximum instead of from the minimum to the thumb. Useful for
    /// "at least" / "greater than" semantics where the selected region is
    /// everything above the threshold.
    pub fn reverse_fill(mut self, reverse: bool) -> Self {
        self.reverse_fill = reverse;
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn render_thumb(
        &self,
        start: DefiniteLength,
        is_start: bool,
        bar_color: Background,
        thumb_color: Hsla,
        radius: Corners<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl gpui::IntoElement {
        let entity_id = self.state.entity_id();
        let axis = self.axis;
        let id = ("slider-thumb", is_start as u32);

        if self.disabled {
            return div().id(id);
        }

        div()
            .id(id)
            .absolute()
            .when(axis.is_horizontal(), |this| {
                this.top(px(-5.)).left(start).ml(-px(8.))
            })
            .when(axis.is_vertical(), |this| {
                this.bottom(start).left(px(-5.)).mb(-px(8.))
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .corner_radii(radius)
            .bg(bar_color.opacity(0.5))
            .when(cx.theme().shadow, |this| this.shadow_md())
            .size_4()
            .p(px(1.))
            .child(
                div()
                    .flex_shrink_0()
                    .size_full()
                    .corner_radii(radius)
                    .bg(thumb_color),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_drag(DragThumb((entity_id, is_start)), |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            })
            .on_drag_move(window.listener_for(
                &self.state,
                move |view, e: &DragMoveEvent<DragThumb>, window, cx| {
                    match e.drag(cx) {
                        DragThumb((id, is_start)) => {
                            if *id != entity_id {
                                return;
                            }

                            // set value by mouse position
                            view.update_value_by_position(
                                axis,
                                e.event.position,
                                *is_start,
                                window,
                                cx,
                            )
                        }
                    }
                },
            ))
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let axis = self.axis;
        let entity_id = self.state.entity_id();
        let state = self.state.read(cx);
        let is_range = state.value().is_range();
        let percentage = state.percentage.clone();
        let focus_handle = state.focus_handle.clone();
        let is_focused = focus_handle.is_focused(window) && !self.disabled;

        // Inset thumb positioning so the thumb stays inside the bar bounds
        // at the extremes (instead of spilling 50% past). This matches how
        // browsers render `<input type="range">`.
        //
        // The thumb's center moves linearly between `THUMB_RADIUS` and
        // `bar_size - THUMB_RADIUS`, rather than `0..bar_size`. The fill
        // bar is updated to terminate at the thumb center so it stays
        // visually aligned.
        //
        // On the very first render `state.bounds` is the default (zero)
        // because it's only filled in during prepaint. In that case we
        // fall back to the original (non-inset) percentage positioning.
        // The `on_prepaint` handler below calls `cx.notify()` whenever
        // bounds change, so the next frame uses the correct values.
        const THUMB_RADIUS: f32 = 8.0;
        const THUMB_DIAMETER: f32 = 16.0;
        let bar_size_px: f32 = state.bounds.size.along(axis).into();
        let inner_size = (bar_size_px - THUMB_DIAMETER).max(0.0);
        let use_inset = inner_size > 0.0;
        let thumb_center_at = |p: f32| -> DefiniteLength {
            if use_inset {
                px(THUMB_RADIUS + p * inner_size).into()
            } else {
                relative(p)
            }
        };

        let (bar_start, bar_end): (DefiniteLength, DefiniteLength) =
            if self.reverse_fill && !is_range {
                // Fill from thumb to max edge.
                if use_inset {
                    let center = THUMB_RADIUS + percentage.end * inner_size;
                    (px(center).into(), px(0.).into())
                } else {
                    (relative(percentage.end), relative(0.0))
                }
            } else {
                // Fill from min edge (or start thumb in range mode) to end thumb center.
                if use_inset {
                    let start_x = if is_range {
                        THUMB_RADIUS + percentage.start * inner_size
                    } else {
                        0.0
                    };
                    let end_center = THUMB_RADIUS + percentage.end * inner_size;
                    (
                        px(start_x).into(),
                        px((bar_size_px - end_center).max(0.0)).into(),
                    )
                } else {
                    (relative(percentage.start), relative(1. - percentage.end))
                }
            };
        let rem_size = window.rem_size();

        let bar_color = self
            .style
            .background
            .clone()
            .and_then(|bg| bg.color())
            .unwrap_or(cx.theme().slider_bar.into());
        let thumb_color = self
            .style
            .text
            .color
            .unwrap_or_else(|| cx.theme().slider_thumb);
        let corner_radii = self.style.corner_radii.clone();
        let default_radius = px(999.);
        let mut radius = Corners {
            top_left: corner_radii
                .top_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            top_right: corner_radii
                .top_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_left: corner_radii
                .bottom_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_right: corner_radii
                .bottom_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
        };
        if cx.theme().radius.is_zero() {
            radius.top_left = px(0.);
            radius.top_right = px(0.);
            radius.bottom_left = px(0.);
            radius.bottom_right = px(0.);
        }

        div()
            .id(("slider", self.state.entity_id()))
            .key_context(CONTEXT)
            .when(!self.disabled, |this| {
                this.track_focus(&focus_handle.tab_stop(true))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_left))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_right))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_up))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_down))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_first))
                    .on_action(window.listener_for(&self.state, SliderState::on_select_last))
                    .on_action(window.listener_for(&self.state, SliderState::on_page_up))
                    .on_action(window.listener_for(&self.state, SliderState::on_page_down))
            })
            // `relative()` gives the focus ring (an absolutely-positioned
            // child added by `focus_ring`) a positioning context.
            .relative()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .when(axis.is_vertical(), |this| this.h(px(120.)))
            .when(axis.is_horizontal(), |this| this.w_full())
            .refine_style(&self.style)
            .bg(cx.theme().transparent)
            .text_color(cx.theme().foreground)
            // Rounded corners pass through to the focus ring (which reads
            // the parent's corner_radii) so the ring is a rounded rect
            // instead of a hard square.
            .rounded(cx.theme().radius)
            .focus_ring(is_focused, px(2.), window, cx)
            .child(
                h_flex()
                    .id("slider-bar-container")
                    .when(!self.disabled, |this| {
                        this.on_mouse_down(
                            MouseButton::Left,
                            window.listener_for(
                                &self.state,
                                move |state, e: &MouseDownEvent, window, cx| {
                                    let mut is_start = false;
                                    if is_range {
                                        let bar_size = state.bounds.size.along(axis);
                                        let inner_pos = if axis.is_horizontal() {
                                            e.position.x - state.bounds.left()
                                        } else {
                                            state.bounds.bottom() - e.position.y
                                        };
                                        let center = ((percentage.end - percentage.start) / 2.0
                                            + percentage.start)
                                            * bar_size;
                                        is_start = inner_pos < center;
                                    }

                                    state.update_value_by_position(
                                        axis, e.position, is_start, window, cx,
                                    )
                                },
                            ),
                        )
                    })
                    .when(!self.disabled && !is_range, |this| {
                        this.on_drag(DragSlider(entity_id), |drag, _, _, cx| {
                            cx.stop_propagation();
                            cx.new(|_| drag.clone())
                        })
                        .on_drag_move(window.listener_for(
                            &self.state,
                            move |view, e: &DragMoveEvent<DragSlider>, window, cx| match e.drag(cx)
                            {
                                DragSlider(id) => {
                                    if *id != entity_id {
                                        return;
                                    }

                                    view.update_value_by_position(
                                        axis,
                                        e.event.position,
                                        false,
                                        window,
                                        cx,
                                    )
                                }
                            },
                        ))
                    })
                    .when(axis.is_horizontal(), |this| {
                        this.items_center().h_6().w_full()
                    })
                    .when(axis.is_vertical(), |this| {
                        this.justify_center().w_6().h_full()
                    })
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("slider-bar")
                            .relative()
                            .when(axis.is_horizontal(), |this| this.w_full().h_1p5())
                            .when(axis.is_vertical(), |this| this.h_full().w_1p5())
                            .bg(bar_color.opacity(0.2))
                            .active(|this| this.bg(bar_color.opacity(0.4)))
                            .corner_radii(radius)
                            .when(self.show_fill, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .when(axis.is_horizontal(), |this| {
                                            this.h_full().left(bar_start).right(bar_end)
                                        })
                                        .when(axis.is_vertical(), |this| {
                                            this.w_full().bottom(bar_start).top(bar_end)
                                        })
                                        .bg(bar_color)
                                        .when(!cx.theme().radius.is_zero(), |this| {
                                            this.rounded_full()
                                        }),
                                )
                            })
                            .when(is_range, |this| {
                                this.child(self.render_thumb(
                                    thumb_center_at(percentage.start),
                                    true,
                                    bar_color,
                                    thumb_color,
                                    radius,
                                    window,
                                    cx,
                                ))
                            })
                            .child(self.render_thumb(
                                thumb_center_at(percentage.end),
                                false,
                                bar_color,
                                thumb_color,
                                radius,
                                window,
                                cx,
                            ))
                            .on_prepaint({
                                let state = self.state.clone();
                                move |bounds, _, cx| {
                                    // Update the cached bar bounds. If they
                                    // changed, notify so the next render can
                                    // pick up the correct (inset) thumb
                                    // positioning. We compare against the
                                    // current value to avoid spurious
                                    // re-renders on every frame.
                                    state.update(cx, |r, cx| {
                                        if r.bounds != bounds {
                                            r.bounds = bounds;
                                            cx.notify();
                                        }
                                    })
                                }
                            }),
                    ),
            )
    }
}
