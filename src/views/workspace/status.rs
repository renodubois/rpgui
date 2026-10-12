//! Consistent, non-color-only runtime status presentation.
use crate::threads::Phase;
use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme, Icon},
    *,
};

pub(super) fn label(phase: Phase) -> &'static str {
    match phase {
        Phase::Passive => "Saved",
        Phase::Starting => "Starting",
        Phase::Idle => "Ready",
        Phase::Running => "Running",
        Phase::Compacting => "Compacting",
        Phase::Retrying => "Retrying",
        Phase::Attention => "Needs input",
        Phase::Failed => "Failed",
        Phase::Uncertain => "Uncertain",
        Phase::Closing => "Closing",
    }
}
pub(super) fn color(phase: Phase, cx: &App) -> Hsla {
    match phase {
        Phase::Passive | Phase::Closing => cx.theme().muted_foreground,
        Phase::Idle => cx.theme().success,
        Phase::Attention | Phase::Uncertain => cx.theme().warning,
        Phase::Failed => cx.theme().danger,
        _ => cx.theme().ring,
    }
}
pub(super) fn icon(phase: Phase) -> IconName {
    match phase {
        Phase::Attention | Phase::Failed | Phase::Uncertain => IconName::CircleAlert,
        Phase::Passive => IconName::Circle,
        _ => IconName::CircleDot,
    }
}
pub(super) fn badge(phase: Phase, cx: &App) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(cx.theme().muted)
        .text_color(color(phase, cx))
        .text_size(px(11.))
        .child(Icon::new(icon(phase)).size(px(12.)))
        .child(label(phase))
}
pub(super) fn notice(title: &str, message: impl Into<SharedString>, color: Hsla, cx: &App) -> Div {
    div()
        .border_l_2()
        .border_color(color)
        .bg(cx.theme().muted)
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(title.to_owned()),
        )
        .child(div().text_sm().child(message.into()))
}
#[cfg(test)]
#[path = "tests/status.rs"]
mod tests;
