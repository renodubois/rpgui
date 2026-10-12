//! Local creation form; the workspace resolves defaults and owns creation.
use crate::{connectors::git, workspace::Workspace};
use gpui_kit::{
    base::{
        Disableable,
        input::{InputBaseState, InputMode, InputState},
    },
    component::{
        ActiveTheme, WindowExt,
        button::{Button, ButtonVariants},
        input::Input,
        radio::Radio,
    },
    *,
};

pub(super) fn open(owner: Entity<Workspace>, project: u64, window: &mut Window, cx: &mut App) {
    if owner.read(cx).exit_requested {
        return;
    }
    let request = owner.update(cx, |owner, cx| {
        let request = owner.prepare_creation(project);
        cx.notify();
        request
    });
    let form = cx.new(|cx| CreationForm::new(owner, project, request, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("Create thread")
            .child(form.clone())
            .footer(div())
    });
}

struct CreationForm {
    owner: Entity<Workspace>,
    project: u64,
    request: u64,
    loaded: bool,
    shared: bool,
    name: Entity<InputBaseState<InputMode>>,
    commit: Entity<InputBaseState<InputMode>>,
    _observe: Subscription,
    _input_observations: [Subscription; 2],
}
impl CreationForm {
    fn new(
        owner: Entity<Workspace>,
        project: u64,
        request: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value("New thread"));
        let commit =
            cx.new(|cx| InputState::new(window, cx).placeholder("Full committed object ID"));
        let observe = cx.observe(&owner, |_, _, cx| cx.notify());
        let input_observations = [
            cx.observe(&name, |_, _, cx| cx.notify()),
            cx.observe(&commit, |_, _, cx| cx.notify()),
        ];
        Self {
            owner,
            project,
            request,
            loaded: false,
            shared: false,
            name,
            commit,
            _observe: observe,
            _input_observations: input_observations,
        }
    }
}
impl Render for CreationForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let result = self
            .owner
            .read(cx)
            .creation_base
            .as_ref()
            .filter(|(request, _)| *request == self.request)
            .map(|(_, result)| result.clone());
        if !self.loaded
            && let Some(result) = &result
        {
            if let Ok(commit) = result {
                self.commit
                    .update(cx, |input, cx| input.set_value(commit.clone(), window, cx));
            }
            self.loaded = true;
        }
        let valid = !self.name.read(cx).text().to_string().trim().is_empty()
            && (self.shared
                || (self.loaded
                    && git::is_full_object_id(self.commit.read(cx).text().to_string().trim())))
            && !self.owner.read(cx).exit_requested;
        let mut body = div()
            .id("create-thread-form")
            .flex()
            .flex_col()
            .gap_3()
            .child("Thread name")
            .child(
                Input::new(&self.name)
                    .id("create-thread-name")
                    .aria_label("Thread name"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        Radio::new("create-isolated-mode")
                            .label("Isolated worktree (recommended)")
                            .checked(!self.shared)
                            .disabled(self.owner.read(cx).exit_requested)
                            .on_change(cx.listener(|view, _, _, cx| {
                                view.shared = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Radio::new("create-directory-mode")
                            .label("Shared project directory")
                            .checked(self.shared)
                            .disabled(self.owner.read(cx).exit_requested)
                            .on_change(cx.listener(|view, _, _, cx| {
                                view.shared = true;
                                cx.notify();
                            })),
                    ),
            );
        if self.shared {
            body = body.child(div().text_sm().text_color(cx.theme().warning).child("Uses the project directory directly, including uncommitted changes. Other threads may edit the same files."));
        } else {
            body = body.child("Base commit (defaults to current HEAD)")
                .child(Input::new(&self.commit).id("create-thread-base").aria_label("Base commit").disabled(!self.loaded))
                .child("Creates a separate worktree from this commit. Uncommitted changes are not copied.");
            match result {
                None => body = body.child("Loading current commit…"),
                Some(Err(error)) => body = body.child(error),
                _ => {}
            }
            if self.loaded
                && !git::is_full_object_id(self.commit.read(cx).text().to_string().trim())
            {
                body = body.child("Enter a full 40- or 64-character commit hash.");
            }
        }
        body.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("create-thread-cancel")
                        .ghost()
                        .label("Cancel")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .child(
                    Button::new("create-thread-submit")
                        .primary()
                        .label("Create thread")
                        .disabled(!valid)
                        .on_click(cx.listener(|view, _, window, cx| {
                            let name = view.name.read(cx).text().to_string().trim().to_owned();
                            let commit = if view.shared {
                                None
                            } else {
                                Some(view.commit.read(cx).text().to_string().trim().to_owned())
                            };
                            if name.is_empty()
                                || commit
                                    .as_ref()
                                    .is_some_and(|base| !git::is_full_object_id(base))
                                || view.owner.read(cx).exit_requested
                            {
                                return;
                            }
                            view.owner.update(cx, |owner, cx| {
                                owner.create(view.project, name, commit);
                                cx.notify();
                            });
                            window.close_dialog(cx);
                        })),
                ),
        )
    }
}
