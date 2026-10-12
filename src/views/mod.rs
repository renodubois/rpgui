pub(crate) mod app_shell;
mod workspace;

pub(crate) fn init(cx: &mut gpui_kit::App) {
    workspace::init(cx);
}
