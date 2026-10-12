//! Stable window host retains workflow owners and consumes their opaque updates independently of views.
use super::workspace::WorkspaceView;
use crate::{storage::workspace::Profile, workspace::Workspace};
use gpui_kit::component::button::Button;
use gpui_kit::*;
use std::time::{Duration, Instant};

pub(crate) fn open(
    window: &mut Window,
    cx: &mut App,
    profile: Result<Profile, String>,
) -> Entity<AppShell> {
    cx.new(|cx| AppShell::new(window, cx, profile))
}
pub(crate) struct AppShell {
    owner: Option<Entity<Workspace>>,
    screen: Option<Entity<WorkspaceView>>,
    _delivery: Option<Task<()>>,
    startup_error: Option<String>,
    audio: Option<std::sync::mpsc::Receiver<Result<(), &'static str>>>,
}
impl AppShell {
    fn new(window: &mut Window, cx: &mut Context<Self>, profile: Result<Profile, String>) -> Self {
        let workspace = match profile.and_then(Workspace::new) {
            Ok(workspace) => workspace,
            Err(error) => {
                return Self {
                    owner: None,
                    screen: None,
                    _delivery: None,
                    audio: None,
                    startup_error: Some(error),
                };
            }
        };
        let owner = cx.new(|_| workspace);
        let close_owner = owner.clone();
        window.on_window_should_close(cx, move |_, cx| {
            close_owner.update(cx, |owner, cx| {
                if owner.ready_to_exit() {
                    true
                } else {
                    owner.request_exit();
                    cx.notify();
                    false
                }
            })
        });
        let screen = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        let executor = cx.background_executor().clone();
        let delivery = cx.spawn_in(window, async move |weak, cx| {
            let mut sounded = 0;
            loop {
                executor.timer(Duration::from_millis(40)).await;
                if weak
                    .update_in(cx, |shell, window, cx| {
                        let audio_result = shell
                            .audio
                            .as_ref()
                            .and_then(|receiver| receiver.try_recv().ok());
                        if audio_result.is_some() {
                            shell.audio = None;
                        }
                        shell.owner.as_ref().unwrap().update(cx, |owner, cx| {
                            if let Some(Err(error)) = audio_result {
                                owner.warning = Some(error.into());
                                cx.notify();
                            }
                            if owner.poll(Instant::now()) {
                                cx.notify();
                            }
                            if owner.attention_transitions > sounded {
                                sounded = owner.attention_transitions;
                                if !owner.metadata.muted {
                                    shell.audio =
                                        Some(crate::connectors::notification::attention());
                                }
                            }
                            if owner.ready_to_exit() {
                                window.remove_window();
                            }
                        });
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            owner: Some(owner),
            screen: Some(screen),
            _delivery: Some(delivery),
            startup_error: None,
            audio: None,
        }
    }
}
impl Render for AppShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if let Some(screen) = &self.screen {
            div().size_full().child(screen.clone())
        } else {
            div()
                .size_full()
                .p_8()
                .flex()
                .flex_col()
                .gap_4()
                .child("Writable startup refused; no workspace changes were made.")
                .child(self.startup_error.clone().unwrap_or_default())
                .child(
                    Button::new("close-startup-error")
                        .label("Close")
                        .on_click(|_, window, _| window.remove_window()),
                )
        }
    }
}
#[cfg(test)]
#[path = "tests/app_shell.rs"]
mod tests;
