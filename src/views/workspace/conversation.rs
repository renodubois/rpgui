//! Conversation viewport, compact toolbar and on-demand thread details/activity.
use super::{WorkspaceView, status, thread_menu};
use crate::storage::history::History;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme, Icon, Selectable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
        text::TextView,
    },
    *,
};
use std::path::Path;

impl WorkspaceView {
    pub(super) fn render_thread(&self, id: u64, cx: &Context<Self>) -> Div {
        let owner = self.owner.read(cx);
        let Some(thread) = owner.threads.get(&id) else {
            return div();
        };
        let saved = owner.metadata.threads.iter().find(|t| t.id == id);
        let title = saved
            .map(|t| t.name.clone())
            .unwrap_or_else(|| format!("Thread {id}"));
        let mut toolbar = div()
            .h(px(48.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(status::badge(thread.phase, cx));
        if !thread.live() {
            let open_owner = self.owner.clone();
            toolbar = toolbar.child(
                Button::new(SharedString::from(format!("open-{id}")))
                    .small()
                    .icon(IconName::Play)
                    .label("Open runtime")
                    .disabled(
                        owner.exit_requested
                            || owner
                                .threads
                                .values()
                                .any(|t| t.phase == crate::threads::Phase::Uncertain),
                    )
                    .tooltip(
                        "Explicitly open this owned thread; saved history does not start an agent",
                    )
                    .on_click(move |_, _, cx| {
                        open_owner.update(cx, |owner, cx| {
                            owner.open(id);
                            cx.notify();
                        })
                    }),
            );
        }
        toolbar = toolbar
            .child(
                Button::new(SharedString::from(format!("git-{id}")))
                    .ghost()
                    .small()
                    .icon(IconName::GitBranch)
                    .accessibility_label("Refresh Git status")
                    .tooltip("Refresh Git status")
                    .disabled(owner.exit_requested)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.details = true;
                        view.owner.update(cx, |owner, cx| {
                            owner.status(id);
                            cx.notify();
                        });
                    })),
            )
            .child(
                Button::new("thread-details-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::Info)
                    .selected(self.details)
                    .toggled(self.details)
                    .accessibility_label("Thread details")
                    .tooltip("Directory, resource trust and runtime identity")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.details = !view.details;
                        cx.notify();
                    })),
            )
            .child(thread_menu::trigger(self.owner.clone(), id));
        let mut body = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(toolbar);
        if self.details {
            let mut details = div()
                .id("thread-details")
                .test_support()
                .max_h(px(150.))
                .overflow_y_scroll()
                .flex_shrink_0()
                .px_4()
                .py_3()
                .flex()
                .flex_col()
                .gap_1()
                .bg(cx.theme().sidebar)
                .text_sm()
                .border_b_1()
                .border_color(cx.theme().border);
            if let Some(saved) = saved {
                details = details
                    .child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(saved.directory.display().to_string()),
                    )
                    .child(div().text_color(cx.theme().muted_foreground).child(format!(
                        "{} · {}",
                        if saved.isolated {
                            "Isolated committed worktree"
                        } else {
                            "Explicit shared directory"
                        },
                        if saved.trusted {
                            "Trusted resources, subject to audit admission"
                        } else {
                            "Project resources not loaded"
                        }
                    )));
            }
            details = details.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Thread {} · attempt {}", thread.id, thread.attempt)),
            );
            if let Some(git) = owner.worktrees.status(id) {
                details = details.child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .child(git.to_owned()),
                );
            }
            body = body.child(details);
        }
        let mut notices = div()
            .id("thread-notices")
            .max_h(px(150.))
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        if let Some(error) = &thread.error {
            notices = notices.child(status::notice(
                "Thread diagnostic",
                error.clone(),
                cx.theme().warning,
                cx,
            ));
        }
        if thread.omitted {
            notices = notices.child(status::notice(
                "Bounded presentation",
                "Older output omitted. Pi saved history remains authoritative.",
                cx.theme().muted_foreground,
                cx,
            ));
        }
        for notification in &thread.notifications {
            notices = notices.child(status::notice(
                "Notification",
                notification.clone(),
                cx.theme().ring,
                cx,
            ));
        }
        if thread.unknown_admission() {
            let acknowledge_owner = self.owner.clone();
            notices = notices.child(status::notice("Prompt outcome unknown", "Do not resend an uncertain prompt. Acknowledgement never replays it or proves its outcome.", cx.theme().warning, cx)
                .child(Button::new("acknowledge-unknown-admission").small().label("Acknowledge uncertainty")
                    .on_click(move |_, _, cx| acknowledge_owner.update(cx, |owner, cx| {
                        if let Some(t) = owner.threads.get_mut(&id) { t.acknowledge_unknown_admission(); }
                        cx.notify();
                    }))));
        }
        body = body.child(notices);
        if self.settings {
            body = body.child(self.render_model_picker(id, cx));
        }
        body = body.child(
            div()
                .flex_shrink_0()
                .h(px(38.))
                .px_3()
                .flex()
                .items_center()
                .gap_1()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    Button::new("conversation-tab")
                        .ghost()
                        .small()
                        .icon(IconName::MessageSquare)
                        .label("Conversation")
                        .selected(!self.inspect)
                        .toggled(!self.inspect)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.inspect = false;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("inspect-toggle")
                        .ghost()
                        .small()
                        .icon(IconName::Terminal)
                        .label("Activity")
                        .selected(self.inspect)
                        .toggled(self.inspect)
                        .tooltip("Thinking, tools, child activity and discovered commands")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.inspect = true;
                            cx.notify();
                        })),
                )
                .child(div().flex_1())
                .child(
                    Button::new("follow-latest")
                        .ghost()
                        .small()
                        .icon(if self.follow_latest {
                            IconName::ChevronsDown
                        } else {
                            IconName::Pause
                        })
                        .label(if self.follow_latest { "Live" } else { "Paused" })
                        .selected(self.follow_latest)
                        .toggled(self.follow_latest)
                        .tooltip(if self.follow_latest {
                            "Pause auto-scroll to read history"
                        } else {
                            "Jump to latest and resume auto-scroll"
                        })
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.follow_latest = !view.follow_latest;
                            if view.follow_latest {
                                view.scroll.scroll_to_bottom();
                            }
                            cx.notify();
                        })),
                ),
        );
        let content = if self.inspect {
            format!(
                "{}\n\n### Structured children (bounded / possibly partial)\nObserved {:?}\n```json\n{}\n```\n\n### Commands\n```json\n{}\n```",
                thread.inspection,
                thread.child_observed,
                serde_json::to_string_pretty(&thread.children).unwrap_or_default(),
                serde_json::to_string_pretty(&thread.commands).unwrap_or_default()
            )
        } else {
            thread.text.clone()
        };
        let mut viewport = div()
            .id("conversation-scroll")
            .test_support()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_4()
            .on_scroll_wheel(cx.listener(|view, event: &ScrollWheelEvent, _, cx| {
                if event.delta.pixel_delta(px(20.)).y > px(0.) {
                    view.follow_latest = false;
                    cx.notify();
                }
            }));
        if content.is_empty() {
            viewport = viewport.child(
                div()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::MessageSquare).size(px(28.)))
                    .child("Start a conversation")
                    .child(div().text_sm().child(if thread.live() {
                        "Describe a task in the composer below."
                    } else {
                        "Open this thread's runtime when you're ready to work."
                    })),
            );
        } else {
            viewport = viewport.child(TextView::markdown("thread-content", content));
        }
        body = body.child(viewport);
        if self.inspect {
            body = body.child(div().px_3().py_2().flex_shrink_0().flex().items_center().gap_2().bg(cx.theme().sidebar)
                .child(Input::new(&self.child_id).id("child-run-id").small().flex_1().aria_label("Child run ID"))
                .child(Button::new("inspect-child").ghost().small().icon(IconName::Search).label("Inspect child")
                    .disabled(!thread.live() || owner.exit_requested)
                    .tooltip("Inspect the exact run ID, or leave it blank to refresh all observed children")
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let target = view.child_id.read(cx).text().to_string();
                        view.owner.update(cx, |owner, cx| {
                            if let Some(t) = owner.threads.get_mut(&id) { t.inspect_children(if target.is_empty() { None } else { Some(target) }); }
                            cx.notify();
                        });
                    }))));
        }
        body = body.child(
            div()
                .id("usage-status")
                .flex_shrink_0()
                .px_4()
                .py_1()
                .border_t_1()
                .border_color(cx.theme().border)
                .text_size(px(11.))
                .text_color(cx.theme().muted_foreground)
                .child(if thread.statistics.is_null() {
                    "Usage available after opening the runtime".to_owned()
                } else {
                    format!(
                        "{} tokens  ·  ${}  ·  Context ~{}%",
                        metric(&thread.statistics["tokens"]["total"]),
                        metric(&thread.statistics["cost"]),
                        metric(&thread.statistics["contextUsage"]["percent"])
                    )
                }),
        );
        body.child(self.render_composer(id, cx))
    }
    fn render_model_picker(&self, id: u64, cx: &Context<Self>) -> impl IntoElement + use<> {
        let owner = self.owner.read(cx);
        let thread = &owner.threads[&id];
        let disabled = owner.exit_requested
            || !thread.live()
            || !matches!(
                thread.phase,
                crate::threads::Phase::Idle | crate::threads::Phase::Failed
            );
        let mut models = div()
            .id("model-options")
            .max_h(px(120.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (index, model) in thread.models.iter().enumerate() {
            let model = model.clone();
            let model_owner = self.owner.clone();
            let name = format!(
                "{}/{}",
                model["provider"].as_str().unwrap_or(""),
                model["id"].as_str().unwrap_or("")
            );
            models = models.child(
                Button::new(SharedString::from(format!("model-{index}")))
                    .ghost()
                    .small()
                    .w_full()
                    .disabled(disabled)
                    .selected(
                        model["provider"] == thread.state["model"]["provider"]
                            && model["id"] == thread.state["model"]["id"],
                    )
                    .accessibility_label(name.clone())
                    .tooltip(name.clone())
                    .child(div().w_full().min_w_0().truncate().text_left().child(name))
                    .on_click(move |_, _, cx| {
                        model_owner.update(cx, |owner, cx| {
                            if let Some(t) = owner.threads.get_mut(&id) {
                                t.select_model(
                                    model["provider"].as_str().unwrap_or("").to_owned(),
                                    model["id"].as_str().unwrap_or("").to_owned(),
                                );
                            }
                            cx.notify();
                        })
                    }),
            );
        }
        let mut levels = div().flex().flex_wrap().gap_1();
        for level in &thread.levels {
            let level = level.clone();
            let level_owner = self.owner.clone();
            levels = levels.child(
                Button::new(SharedString::from(format!("thinking-{level}")))
                    .ghost()
                    .small()
                    .label(level.clone())
                    .disabled(disabled)
                    .selected(thread.state["thinkingLevel"].as_str() == Some(level.as_str()))
                    .on_click(move |_, _, cx| {
                        level_owner.update(cx, |owner, cx| {
                            if let Some(t) = owner.threads.get_mut(&id) {
                                t.select_thinking(level.clone());
                            }
                            cx.notify();
                        })
                    }),
            );
        }
        div()
            .id("model-picker")
            .test_support()
            .flex_shrink_0()
            .px_4()
            .py_3()
            .bg(cx.theme().sidebar)
            .border_b_1()
            .border_color(cx.theme().border)
            .flex()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Model"),
                    )
                    .child(models)
                    .when(thread.models.is_empty(), |this| {
                        this.child(div().text_sm().child("Open runtime to discover models."))
                    }),
            )
            .child(
                div()
                    .w(px(220.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Thinking"),
                    )
                    .child(levels)
                    .when(disabled, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Changes require an idle runtime."),
                        )
                    }),
            )
    }
    pub(super) fn render_external(
        &self,
        path: &Path,
        history: &History,
        cx: &Context<Self>,
    ) -> Div {
        div().flex_1().min_h_0().flex().flex_col()
            .child(div().h(px(48.)).flex_shrink_0().px_4().flex().items_center().gap_2().border_b_1().border_color(cx.theme().border)
                .child(Icon::new(IconName::FileClock).size(px(16.)))
                .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).child(path.display().to_string()))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Read-only snapshot")))
            .child(status::notice("Unknown writer ownership", format!("Saved directory: {}. No writable handoff, copying or replay is admitted. This is a snapshot, not a live attachment.", history.cwd.display()), cx.theme().warning, cx))
            .when(history.omitted, |this| this.child(status::notice("Bounded presentation", "Older saved history omitted.", cx.theme().muted_foreground, cx)))
            .child(Button::new("external-inspection").ghost().small().icon(IconName::Terminal)
                .label(if self.inspect { "Conversation" } else { "Activity" })
                .on_click(cx.listener(|view, _, _, cx| { view.inspect = !view.inspect; cx.notify(); })))
            .child(div().id("external-history").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).p_4()
                .child(TextView::markdown("external-content", if self.inspect { history.inspection.clone() } else { history.markdown.clone() })))
    }
}
fn metric(value: &serde_json::Value) -> String {
    if value.is_null() {
        "—".to_owned()
    } else {
        value.to_string()
    }
}
