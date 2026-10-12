use super::*;
use gpui_kit::{TestAppContext, component::Theme};
#[gpui_kit::test]
fn installs_dark_theme_without_native_window(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        init(cx);
        let theme = Theme::global(cx);
        assert!(theme.mode.is_dark());
        assert_eq!(theme.font_size, px(14.));
        assert_eq!(theme.radius, px(4.));
        assert_ne!(theme.background, theme.sidebar);
        assert_ne!(theme.sidebar, theme.muted);
        assert_eq!(theme.tokens.primary.color, theme.colors.primary);
        assert_eq!(theme.tokens.sidebar.color, theme.colors.sidebar);
        assert_eq!(theme.semantic_tokens().colors.primary, theme.colors.primary);
    });
}
