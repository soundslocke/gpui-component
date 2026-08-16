use crate::{ActiveTheme, Sizable, Size};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, Hsla, IntoElement, Radians, Render, RenderOnce,
    SharedString, StyleRefinement, Styled, Svg, Transformation, Window,
    prelude::FluentBuilder as _, svg,
};
use gpui_component_macros::icon_named;

/// Types implementing this trait can automatically be converted to [`Icon`].
///
/// This allows you to implement a custom version of [`IconName`] that functions as a drop-in
/// replacement for other UI components.
pub trait IconNamed {
    /// Returns the embedded path of the icon.
    fn path(self) -> SharedString;
}

impl<T: IconNamed> From<T> for Icon {
    fn from(value: T) -> Self {
        Icon::build(value)
    }
}

// Generate `IconName` from the icons that `gpui-kit-assets` ships.
// The `$VAR` form resolves to the absolute path published by the assets
// crate's `build.rs` (via cargo's `links` mechanism) and re-exported by
// our own `build.rs`. See `gpui_component_macros::icon_named!`'s doc
// comment for the full mechanism.
icon_named!(IconName, "$GPUI_KIT_DEFAULT_ICONS_DIR");

impl IconName {
    /// Return the icon as a Entity<Icon>
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        Icon::build(self).view(cx)
    }
}

impl From<IconName> for AnyElement {
    fn from(val: IconName) -> Self {
        Icon::build(val).into_any_element()
    }
}

impl RenderOnce for IconName {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::build(self)
    }
}

/// An icon.
///
/// This is a description of an icon, not a built element: the `svg()` it
/// renders to is created in `render`. Keep it that way. Types all over the
/// crate store an `Option<Icon>` inline, so every byte here is paid many times
/// over, and a stored `gpui::Svg` alone costs 1344 of them.
#[derive(IntoElement, Default)]
pub struct Icon {
    style: Option<Box<StyleRefinement>>,
    path: SharedString,
    text_color: Option<Hsla>,
    size: Option<Size>,
    transformation: Option<Transformation>,
}

// `Icon` is stored inline by roughly twenty types in this crate, several of
// which hold a `Vec` of themselves. Growing it is not a local decision.
const _: () = assert!(std::mem::size_of::<Icon>() <= 96);

impl Clone for Icon {
    fn clone(&self) -> Self {
        Self {
            style: self.style.clone(),
            path: self.path.clone(),
            text_color: self.text_color,
            size: self.size,
            transformation: self.transformation,
        }
    }
}

impl Icon {
    pub fn new(icon: impl Into<Icon>) -> Self {
        icon.into()
    }

    fn build(name: impl IconNamed) -> Self {
        Self::default().path(name.path())
    }

    /// Set the icon path of the Assets bundle
    ///
    /// For example: `icons/foo.svg`
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self
    }

    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub(crate) fn path_ref(&self) -> &SharedString {
        &self.path
    }

    /// Create a new view for the icon
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        cx.new(|_| self)
    }

    pub fn transform(mut self, transformation: Transformation) -> Self {
        self.transformation = Some(transformation);
        self
    }

    pub fn empty() -> Self {
        Self::default()
    }

    /// Rotate the icon by the given angle
    pub fn rotate(self, radians: impl Into<Radians>) -> Self {
        self.transform(Transformation::rotate(radians))
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        self.style.get_or_insert_default()
    }

    fn text_color(mut self, color: impl Into<Hsla>) -> Self {
        self.text_color = Some(color.into());
        self
    }
}

impl Sizable for Icon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = Some(size.into());
        self
    }
}

impl Icon {
    /// Build the `svg` element this icon describes.
    fn build_svg(self, text_color: Hsla, text_size: gpui::Pixels) -> Svg {
        let style = self.style.map(|style| *style).unwrap_or_default();
        let has_base_size = style.size.width.is_some() || style.size.height.is_some();

        let mut base = svg();
        *base.style() = style;

        base.flex_shrink_0()
            .text_color(text_color)
            .when(!has_base_size, |this| this.size(text_size))
            .when_some(self.size, |this, size| match size {
                Size::Size(px) => this.size(px),
                Size::XSmall => this.size_3(),
                Size::Small => this.size_3p5(),
                Size::Medium => this.size_4(),
                Size::Large => this.size_6(),
            })
            .when_some(self.transformation, |this, transformation| {
                this.with_transformation(transformation)
            })
            .path(self.path)
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());

        self.build_svg(text_color, text_size)
    }
}

impl From<Icon> for AnyElement {
    fn from(val: Icon) -> Self {
        val.into_any_element()
    }
}

impl Render for Icon {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_color = self.text_color.unwrap_or_else(|| cx.theme().foreground);
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());

        self.clone().build_svg(text_color, text_size)
    }
}
