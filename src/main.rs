mod connectors;
mod storage;
mod theme;
mod threads;
mod views;
mod workspace;
mod worktrees;

use gpui_kit::*;
use std::path::PathBuf;

fn main() {
    if connectors::process_scope::internal_mode() {
        return;
    }
    let profile = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            "Set an absolute XDG_CONFIG_HOME or HOME; writable startup refused.".to_owned()
        })
        .and_then(|path| {
            storage::workspace::Profile::open(&path.join(env!("CARGO_PKG_NAME")))
                .map_err(|error| error.to_string())
        });
    // Kernel-held profile ownership lasts until retained owners/storage workers release it.
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            theme::init(cx);
            views::init(cx);
            let bounds = Bounds::centered(None, size(px(1200.), px(840.)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some(env!("CARGO_PKG_NAME").into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                move |window, cx| views::app_shell::open(window, cx, profile),
            )
            .expect("open application window");
            cx.activate(true);
        });
}
