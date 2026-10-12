//! One framed composer, explicit delivery choices and a bounded visible queue.
use super::{WorkspaceView, can_send};
use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme, Icon, Selectable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
    },
    *,
};
use std::path::PathBuf;

impl WorkspaceView {
    pub(super) fn render_composer(&self, id: u64, cx: &Context<Self>) -> Div {
        let owner = self.owner.read(cx);
        let thread = &owner.threads[&id];
        let mut composer = div()
            .flex_shrink_0()
            .px_3()
            .pt_2()
            .pb_3()
            .bg(cx.theme().sidebar)
            .flex()
            .flex_col()
            .gap_2();
        if !thread.steering.is_empty() || !thread.follow_up.is_empty() {
            let mut queue = div().flex().flex_col().gap_1().child(
                Button::new("queue-toggle")
                    .ghost()
                    .small()
                    .selected(self.queues)
                    .toggled(self.queues)
                    .icon(if self.queues {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(format!(
                        "Queued · {} steering · {} follow-up",
                        thread.steering.len(),
                        thread.follow_up.len()
                    ))
                    .tooltip("Inspect queued messages; Stop clears the queue before aborting")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.queues = !view.queues;
                        cx.notify();
                    })),
            );
            if self.queues {
                let mut messages = div()
                    .id("queued-messages")
                    .test_support()
                    .max_h(px(100.))
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .border_l_2()
                    .border_color(cx.theme().ring)
                    .flex()
                    .flex_col()
                    .gap_2();
                for (kind, texts) in [
                    ("Steering", &thread.steering),
                    ("Follow-up", &thread.follow_up),
                ] {
                    for text in texts {
                        messages = messages.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(kind),
                                )
                                .child(div().text_sm().child(text.clone())),
                        );
                    }
                }
                queue = queue.child(messages);
            }
            composer = composer.child(queue);
        }
        if thread.image_pending || !thread.images.is_empty() {
            let image_owner = self.owner.clone();
            composer = composer.child(
                div()
                    .id("staged-image")
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(
                        Icon::new(IconName::FileImage)
                            .size(px(14.))
                            .text_color(cx.theme().ring),
                    )
                    .child(if thread.image_pending {
                        "Reading image…"
                    } else {
                        "Image staged"
                    })
                    .child(
                        Button::new("remove-image")
                            .ghost()
                            .small()
                            .icon(IconName::X)
                            .accessibility_label("Remove image")
                            .tooltip("Remove staged image or cancel its read")
                            .disabled(owner.exit_requested)
                            .on_click(move |_, _, cx| {
                                image_owner.update(cx, |owner, cx| {
                                    if let Some(t) = owner.threads.get_mut(&id) {
                                        t.remove_image();
                                    }
                                    cx.notify();
                                })
                            }),
                    ),
            );
        }
        if self.attachments {
            composer = composer.child(
                div()
                    .id("attachment-panel")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .bg(cx.theme().muted)
                    .rounded_sm()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .child("One PNG, JPEG, GIF or WebP · up to 1 MiB"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Input::new(&self.image)
                                    .id("image-path")
                                    .small()
                                    .flex_1()
                                    .aria_label("Image file path"),
                            )
                            .child(
                                Button::new("attach-image")
                                    .small()
                                    .label("Attach")
                                    .disabled(
                                        owner.exit_requested
                                            || thread.image_pending
                                            || !thread.images.is_empty(),
                                    )
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        let path =
                                            PathBuf::from(view.image.read(cx).text().to_string());
                                        view.owner.update(cx, |owner, cx| {
                                            owner.attach_image(id, path);
                                            cx.notify();
                                        });
                                    })),
                            ),
                    ),
            );
        }
        let model = thread.state["model"]["id"].as_str().unwrap_or("Model");
        let provider = thread.state["model"]["provider"]
            .as_str()
            .unwrap_or("not opened");
        let thinking = thread.state["thinkingLevel"].as_str().unwrap_or("—");
        let mut controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(
                Button::new("attachments-toggle")
                    .ghost()
                    .small()
                    .icon(IconName::Paperclip)
                    .selected(self.attachments)
                    .toggled(self.attachments)
                    .accessibility_label("Attach image")
                    .tooltip("Image attachment")
                    .disabled(owner.exit_requested)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.attachments = !view.attachments;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("model-settings")
                    .ghost()
                    .small()
                    .icon(IconName::Cpu)
                    .selected(self.settings)
                    .toggled(self.settings)
                    .max_w(px(240.))
                    .accessibility_label(format!("Model {provider}/{model}, thinking {thinking}"))
                    .tooltip(format!("{provider}/{model} · thinking: {thinking}"))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .child(format!("{model} · {thinking}")),
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.settings = !view.settings;
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_sm()
                    .child(
                        Button::new("delivery-steer")
                            .ghost()
                            .small()
                            .label("Steer")
                            .selected(!self.follow_up)
                            .toggled(!self.follow_up)
                            .tooltip("While busy, steer the current turn (default)")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.follow_up = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("delivery-mode")
                            .ghost()
                            .small()
                            .label("Follow-up")
                            .selected(self.follow_up)
                            .toggled(self.follow_up)
                            .tooltip("While busy, queue a message after the current turn")
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.follow_up = true;
                                cx.notify();
                            })),
                    ),
            );
        if thread.live() {
            let stop_owner = self.owner.clone();
            controls = controls.child(
                Button::new(SharedString::from(format!("stop-{id}")))
                    .ghost()
                    .small()
                    .icon(IconName::Square)
                    .label("Stop")
                    .text_color(cx.theme().danger)
                    .tooltip("Clear queued messages, then stop admitted work")
                    .disabled(
                        owner.exit_requested || thread.phase == crate::threads::Phase::Closing,
                    )
                    .on_click(move |_, _, cx| {
                        stop_owner.update(cx, |owner, cx| {
                            if let Some(t) = owner.threads.get_mut(&id) {
                                t.stop();
                            }
                            cx.notify();
                        })
                    }),
            );
        }
        controls = controls.child(
            Button::new("send-message")
                .primary()
                .small()
                .icon(IconName::ArrowUp)
                .label("Send")
                .disabled(!can_send(
                    thread,
                    owner.exit_requested,
                    &self.prompt.read(cx).text().to_string(),
                ))
                .tooltip(if !thread.live() {
                    "Open the runtime before sending"
                } else if thread.image_pending {
                    "Wait for the image read, or remove it"
                } else {
                    "Send · Ctrl+Enter · selected delivery mode"
                })
                .on_click(cx.listener(|view, _, _, cx| view.send(cx))),
        );
        composer.child(
            div()
                .id("composer")
                .test_support()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .bg(cx.theme().background)
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("conversation-input")
                        .test_support()
                        .key_context("PiComposer")
                        .on_action(cx.listener(|view, _: &super::SendMessage, _, cx| view.send(cx)))
                        .p_2()
                        .child(
                            Textarea::new(&self.prompt)
                                .appearance(false)
                                .bordered(false)
                                .disabled(owner.exit_requested)
                                .aria_label("Message Pi"),
                        ),
                )
                .child(div().p_2().child(controls)),
        )
    }
}
