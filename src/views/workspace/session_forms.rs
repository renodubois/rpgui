//! On-demand session forms; edits stay bound to the thread that opened them.
use crate::workspace::Workspace;
use gpui_kit::{
    base::{
        Disableable,
        input::{InputBaseState, InputMode, InputState},
    },
    component::{
        ActiveTheme, WindowExt,
        button::{Button, ButtonVariants},
        input::Input,
    },
    *,
};
use std::path::PathBuf;

pub(super) fn open_rename(owner: Entity<Workspace>, id: u64, window: &mut Window, cx: &mut App) {
    if owner.read(cx).exit_requested {
        return;
    }
    let Some(name) = owner
        .read(cx)
        .metadata
        .threads
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.name.clone())
    else {
        return;
    };
    let form = cx.new(|cx| {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let observe = cx.observe(&owner, |_, _, cx| cx.notify());
        let input_observe = cx.observe(&name, |_, _, cx| cx.notify());
        RenameForm {
            owner,
            id,
            name,
            _observe: observe,
            _input_observe: input_observe,
        }
    });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("Rename thread")
            .child(form.clone())
            .footer(div())
    });
}
struct RenameForm {
    owner: Entity<Workspace>,
    id: u64,
    name: Entity<InputBaseState<InputMode>>,
    _observe: Subscription,
    _input_observe: Subscription,
}
impl Render for RenameForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                Input::new(&self.name)
                    .id("rename-thread-name")
                    .aria_label("Thread name"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("rename-thread-cancel")
                            .ghost()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("rename-thread-submit")
                            .primary()
                            .label("Save name")
                            .disabled(
                                self.name.read(cx).text().to_string().trim().is_empty()
                                    || self.owner.read(cx).exit_requested,
                            )
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.owner.read(cx).exit_requested {
                                    return;
                                }
                                let name = view.name.read(cx).text().to_string().trim().to_owned();
                                if name.is_empty() {
                                    return;
                                }
                                view.owner.update(cx, |owner, cx| {
                                    owner.rename(view.id, name);
                                    cx.notify();
                                });
                                window.close_dialog(cx);
                            })),
                    ),
            )
    }
}

pub(super) fn open_saved(owner: Entity<Workspace>, window: &mut Window, cx: &mut App) {
    if owner.read(cx).exit_requested {
        return;
    }
    let form = cx.new(|cx| {
        let path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Absolute path to a Pi session file")
        });
        let observe = cx.observe(&owner, |_, _, cx| cx.notify());
        let input_observe = cx.observe(&path, |_, _, cx| cx.notify());
        SavedForm {
            owner,
            path,
            _observe: observe,
            _input_observe: input_observe,
        }
    });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("Inspect saved session")
            .child(form.clone())
            .footer(div())
    });
}
struct SavedForm {
    owner: Entity<Workspace>,
    path: Entity<InputBaseState<InputMode>>,
    _observe: Subscription,
    _input_observe: Subscription,
}
impl Render for SavedForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap_3()
            .child(div().text_sm().text_color(cx.theme().muted_foreground)
                .child("Opens a read-only snapshot. This does not attach to a running terminal session or start an agent."))
            .child(Input::new(&self.path).id("saved-session-path").aria_label("Saved session file path"))
            .child(div().flex().justify_end().gap_2()
                .child(Button::new("saved-session-cancel").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
                .child(Button::new("saved-session-submit").primary().label("Inspect session")
                    .disabled(self.path.read(cx).text().to_string().trim().is_empty() || self.owner.read(cx).exit_requested)
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.owner.read(cx).exit_requested { return; }
                        let path = view.path.read(cx).text().to_string().trim().to_owned();
                        if path.is_empty() { return; }
                        view.owner.update(cx, |owner, cx| { owner.open_saved(PathBuf::from(path)); cx.notify(); });
                        window.close_dialog(cx);
                    }))))
    }
}
