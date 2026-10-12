//! Native workspace presentation. Retained owners decide every workflow.
use crate::{
    threads::{Delivery, Phase, Thread},
    workspace::Workspace,
};
use gpui_kit::{
    assets::IconName,
    base::{
        Disableable,
        input::{InputBaseState, InputEvent, InputMode, InputState, TextareaState},
    },
    component::{
        ActiveTheme, Icon,
        button::{Button, ButtonVariants},
    },
    *,
};
use std::collections::{HashMap, HashSet};

actions!(conversation, [SendMessage]);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "ctrl-enter",
        SendMessage,
        Some("PiComposer > Input"),
    )]);
}

mod composer;
mod conversation;
mod create_thread;
mod dialogs;
mod navigation;
mod session_forms;
mod status;
mod thread_menu;

pub(super) struct WorkspaceView {
    owner: Entity<Workspace>,
    project_picker_pending: bool,
    project_picker_error: Option<String>,
    prompt: Entity<TextareaState>,
    replies: HashMap<String, Entity<TextareaState>>,
    image: Entity<InputBaseState<InputMode>>,
    child_id: Entity<InputBaseState<InputMode>>,
    _draft_subscription: Subscription,
    follow_up: bool,
    inspect: bool,
    settings: bool,
    attachments: bool,
    details: bool,
    queues: bool,
    collapsed_projects: HashSet<u64>,
    observed_selection: Option<u64>,
    follow_latest: bool,
    scroll: ScrollHandle,
    _observe: Subscription,
}
impl WorkspaceView {
    pub(super) fn new(
        owner: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = owner.read(cx).selected;
        let draft = selected
            .and_then(|id| owner.read(cx).threads.get(&id).map(|t| t.draft.clone()))
            .unwrap_or_default();
        let prompt = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 5)
                .placeholder("Message Pi…  / for discovered commands")
                .default_value(draft)
        });
        let image = cx.new(|cx| InputState::new(window, cx).placeholder("Image file path"));
        let child_id =
            cx.new(|cx| InputState::new(window, cx).placeholder("Child run ID (optional)"));
        let observe = cx.observe_in(&owner, window, |view: &mut Self, _, window, cx| {
            let selected = view.owner.read(cx).selected;
            if view.observed_selection != selected {
                view.observed_selection = selected;
                view.inspect = false;
                view.settings = false;
                view.attachments = false;
                view.details = false;
                view.queues = false;
                view.follow_latest = true;
                view.image
                    .update(cx, |input, cx| input.set_value("", window, cx));
                view.child_id
                    .update(cx, |input, cx| input.set_value("", window, cx));
            }
            let draft = selected
                .and_then(|id| {
                    view.owner
                        .read(cx)
                        .threads
                        .get(&id)
                        .map(|t| t.draft.clone())
                })
                .unwrap_or_default();
            if *view.prompt.read(cx).text() != draft {
                view.prompt
                    .update(cx, |input, cx| input.set_value(draft, window, cx));
            }
            if view.follow_latest {
                view.scroll.scroll_to_bottom();
            }
            cx.notify();
        });
        let draft_subscription = cx.subscribe(&prompt, |view: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                let draft = view.prompt.read(cx).text().to_string();
                view.owner.update(cx, |owner, cx| {
                    if let Some(id) = owner.selected
                        && let Some(t) = owner.threads.get_mut(&id)
                    {
                        t.draft = draft;
                    }
                    cx.notify();
                });
            }
        });
        Self {
            owner,
            project_picker_pending: false,
            project_picker_error: None,
            prompt,
            replies: Default::default(),
            image,
            child_id,
            _draft_subscription: draft_subscription,
            follow_up: false,
            inspect: false,
            settings: false,
            attachments: false,
            details: false,
            queues: false,
            collapsed_projects: Default::default(),
            observed_selection: selected,
            follow_latest: true,
            scroll: ScrollHandle::new(),
            _observe: observe,
        }
    }
    fn choose_project(&mut self, cx: &mut Context<Self>) {
        if self.project_picker_pending || self.owner.read(cx).exit_requested {
            return;
        }
        self.project_picker_pending = true;
        self.project_picker_error = None;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = selection.await;
            let _ = view.update(cx, |view, cx| {
                view.project_picker_pending = false;
                if !view.owner.read(cx).exit_requested {
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.into_iter().next() {
                                view.owner.update(cx, |owner, cx| { owner.add_project(path); cx.notify(); });
                            }
                        }
                        Ok(Ok(None)) => {}
                        _ => view.project_picker_error = Some("Cannot open the system folder picker. Check that a desktop file-chooser portal is available.".into()),
                    }
                }
                cx.notify();
            });
        }).detach();
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        let text = self.prompt.read(cx).text().to_string();
        let mode = if self.follow_up {
            Delivery::FollowUp
        } else {
            Delivery::Steer
        };
        self.owner.update(cx, |owner, cx| {
            if let Some(id) = owner.selected
                && let Some(t) = owner.threads.get_mut(&id)
                && can_send(t, owner.exit_requested, &text)
            {
                t.prompt(text, mode, t.images.clone());
            }
            cx.notify();
        });
    }
}

fn can_send(thread: &Thread, exiting: bool, draft: &str) -> bool {
    !exiting
        && thread.live()
        && !thread.image_pending
        && !thread.unknown_admission()
        && matches!(
            thread.phase,
            Phase::Idle
                | Phase::Running
                | Phase::Attention
                | Phase::Compacting
                | Phase::Retrying
                | Phase::Failed
        )
        && (!draft.trim().is_empty() || !thread.images.is_empty())
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_replies(window, cx);
        if self.follow_latest {
            self.scroll.scroll_to_bottom();
        }
        let owner = self.owner.read(cx);
        let mut pane = div()
            .id("conversation-pane")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col();
        if let Some(warning) = &owner.warning {
            pane = pane.child(
                div()
                    .id("workspace-warning")
                    .max_h(px(120.))
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(status::notice(
                        "Workspace",
                        warning.clone(),
                        cx.theme().warning,
                        cx,
                    )),
            );
        }
        if owner.exit_requested {
            pane = pane.child(self.render_exit(cx));
        }
        pane = pane.child(self.render_pending_dialogs(cx));
        if let Some(id) = owner.selected {
            pane = pane.child(self.render_thread(id, cx));
        } else if let Some((path, history)) = &owner.external_history {
            pane = pane.child(self.render_external(path, history, cx));
        } else {
            pane = pane.child(
                div().id("workspace-empty").flex_1().flex().flex_col().items_center().justify_center().gap_3().p_8()
                    .child(Icon::new(IconName::SquareTerminal).size(px(40.)).text_color(cx.theme().muted_foreground))
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("A workspace for your agents"))
                    .child(div().max_w(px(400.)).text_center().text_color(cx.theme().muted_foreground)
                        .child("Add a project, then create a thread. Each thread keeps its own conversation and working directory."))
                    .child(Button::new("empty-add-project").primary().icon(IconName::FolderPlus).label("Add project")
                        .disabled(self.project_picker_pending || owner.exit_requested)
                        .on_click(cx.listener(|view, _, _, cx| view.choose_project(cx))))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Saved threads reopen without starting work."))
            );
        }
        div()
            .size_full()
            .flex()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(pane)
    }
}
#[cfg(test)]
#[path = "tests/workspace.rs"]
mod tests;
