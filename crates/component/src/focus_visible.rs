use gpui::{
    App, Bounds, Element, ElementId, Global, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, MouseDownEvent, Pixels, Style, Window,
};

/// Whether the user is currently driving the UI from the keyboard, which is
/// what decides whether focus rings are drawn.
///
/// This mirrors the web platform's `:focus-visible`. Focus is always tracked,
/// but the ring only appears once a key is pressed and hides again on the next
/// pointer press. Focus that lands on a control by click, or that a component
/// moves programmatically (a dialog pulling focus into its content, for
/// example), therefore stays silent, while keyboard users still see exactly
/// where they are.
struct FocusVisible {
    visible: bool,
}

impl Global for FocusVisible {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(FocusVisible { visible: false });

    // A keystroke observer sees every key press, including ones a key binding
    // consumed (Tab and Escape above all). A capture-phase key listener would
    // miss exactly those: GPUI dispatches bindings first and returns before any
    // key listener runs once an action handles the event.
    cx.observe_keystrokes(|_, window, cx| set_focus_visible(true, window, cx))
        .detach();
}

/// Whether focus rings should be drawn right now.
///
/// See [`FocusableExt::focus_ring`](crate::styled::FocusableExt::focus_ring),
/// the only intended consumer.
pub(crate) fn focus_visible(cx: &App) -> bool {
    cx.global::<FocusVisible>().visible
}

/// Record the input modality behind the event being handled. Redraws on a
/// change so rings appear or vanish on the same frame as the event.
pub(crate) fn set_focus_visible(visible: bool, window: &mut Window, cx: &mut App) {
    if focus_visible(cx) == visible {
        return;
    }

    cx.global_mut::<FocusVisible>().visible = visible;
    window.refresh();
}

/// A zero-size element that hides focus rings again as soon as the pointer
/// takes over. The keyboard half lives in [`init`].
///
/// Must be a child of Root's container div. The listener runs in the capture
/// phase, so the modality is settled before any component handles the press.
///
/// It is registered with [`Window::on_mouse_event`] rather than on a hitbox: an
/// occluding overlay (every open Dialog has one) cuts Root's own hitbox out of
/// the hit test, so presses inside a dialog would never reach it.
pub(crate) struct FocusVisibleController;

impl IntoElement for FocusVisibleController {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for FocusVisibleController {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        window.on_mouse_event(move |_: &MouseDownEvent, phase, window, cx| {
            if phase.capture() {
                set_focus_visible(false, window, cx);
            }
        });
    }
}
