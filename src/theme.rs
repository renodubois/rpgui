//! Product visual policy lives here, not in workflow owners.
use gpui_kit::{
    App,
    component::{Theme, ThemeMode},
    px, rgb,
};
pub(crate) fn init(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_size = px(14.);
        theme.radius = px(4.);
        theme.radius_lg = px(8.);
        let colors = &mut theme.colors;
        colors.background = rgb(0x161b22).into();
        colors.foreground = rgb(0xdce3ec).into();
        colors.sidebar = rgb(0x10151c).into();
        colors.sidebar_foreground = colors.foreground;
        colors.muted = rgb(0x202834).into();
        colors.muted_foreground = rgb(0x97a6ba).into();
        colors.border = rgb(0x303b49).into();
        colors.sidebar_border = colors.border;
        colors.input = colors.border;
        colors.popover = rgb(0x1d2631).into();
        colors.popover_foreground = colors.foreground;
        colors.accent = rgb(0x26374a).into();
        colors.accent_foreground = colors.foreground;
        colors.secondary = colors.muted;
        colors.secondary_hover = colors.muted;
        colors.secondary_active = colors.accent;
        colors.secondary_foreground = colors.foreground;
        colors.button_secondary = colors.secondary;
        colors.button_secondary_hover = colors.secondary_hover;
        colors.button_secondary_active = colors.secondary_active;
        colors.button_secondary_foreground = colors.secondary_foreground;
        colors.primary = rgb(0x2879bc).into();
        colors.primary_hover = rgb(0x3088ce).into();
        colors.primary_active = rgb(0x216aa6).into();
        colors.primary_foreground = rgb(0xffffff).into();
        colors.button_primary = colors.primary;
        colors.button_primary_hover = colors.primary_hover;
        colors.button_primary_active = colors.primary_active;
        colors.button_primary_foreground = colors.primary_foreground;
        colors.button = colors.muted;
        colors.button_foreground = colors.foreground;
        colors.button_hover = colors.accent;
        colors.button_active = colors.accent;
        colors.ring = rgb(0x62b0ee).into();
        colors.selection = colors.accent;
        colors.list = colors.sidebar;
        colors.list_hover = colors.muted;
        colors.list_active = colors.accent;
        colors.list_active_border = colors.ring;
        colors.sidebar_accent = colors.accent;
        colors.sidebar_accent_foreground = colors.foreground;
    });
}
#[cfg(test)]
#[path = "theme/tests/config.rs"]
mod tests;
