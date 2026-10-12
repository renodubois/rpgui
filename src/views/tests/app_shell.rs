use super::open;
use gpui_kit::{TestAppContext, component::Root, test::TestWindowExt};
#[gpui_kit::test]
fn startup_refusal_is_a_native_error_view_without_a_workspace(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut shell = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = open(
            window,
            cx,
            Err("Profile already owned; refusing writable startup.".into()),
        );
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let shell = shell.as_ref().unwrap().read(cx);
        assert!(shell.owner.is_none());
        assert!(
            shell
                .startup_error
                .as_ref()
                .unwrap()
                .contains("already owned")
        );
        assert!(window.try_find("close-startup-error").is_some());
    });
}
