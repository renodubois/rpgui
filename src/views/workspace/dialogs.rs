//! Bounded attention/exit surfaces. Hidden requests retain thread + attempt routing.
use super::{WorkspaceView, status};
use crate::threads::Phase;
use gpui_kit::{
    base::input::TextareaState,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
        input::Textarea,
    },
    *,
};
use serde_json::json;

impl WorkspaceView {
    pub(super) fn prepare_replies(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let requests: Vec<_> = self
            .owner
            .read(cx)
            .threads
            .iter()
            .filter_map(|(id, t)| {
                t.dialog.as_ref().map(|dialog| {
                    (
                        format!("{id}-{}-{}", t.attempt, dialog.record["id"]),
                        dialog.record["prefill"].as_str().unwrap_or("").to_owned(),
                    )
                })
            })
            .collect();
        self.replies
            .retain(|key, _| requests.iter().any(|(id, _)| key == id));
        for (key, prefill) in requests {
            self.replies.entry(key).or_insert_with(|| {
                cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .auto_grow(1, 4)
                        .placeholder("Answer this request")
                        .default_value(prefill)
                })
            });
        }
    }
    pub(super) fn render_pending_dialogs(&self, cx: &Context<Self>) -> Stateful<Div> {
        let owner = self.owner.read(cx);
        let mut dialogs = div()
            .id("pending-dialogs")
            .max_h(px(220.))
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (&id, thread) in &owner.threads {
            if let Some(dialog) = &thread.dialog {
                let attempt = thread.attempt;
                let request_id = dialog.record["id"].as_str().unwrap_or("").to_owned();
                let title = dialog.record["title"].as_str().unwrap_or("Question");
                let name = owner
                    .metadata
                    .threads
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| format!("Thread {id}"));
                let mut card = div()
                    .m_2()
                    .p_3()
                    .border_l_2()
                    .border_color(cx.theme().warning)
                    .bg(cx.theme().muted)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().warning)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{name} · Needs input")),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title.to_owned()),
                    );
                if let Some(message) = dialog.record["message"].as_str() {
                    card = card.child(div().text_sm().child(message.to_owned()));
                }
                let choices: Vec<_> = if dialog.record["method"] == "confirm" {
                    vec![
                        ("Yes".to_owned(), json!({"confirmed":true})),
                        ("No".to_owned(), json!({"confirmed":false})),
                    ]
                } else {
                    dialog.record["options"]
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|v| {
                                    v.as_str().map(|s| (s.to_owned(), json!({"value":s})))
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let mut options = div().flex().flex_wrap().gap_2();
                for (index, (label, answer)) in choices.into_iter().enumerate() {
                    let request_id = request_id.clone();
                    let owner = self.owner.clone();
                    options = options.child(
                        Button::new(SharedString::from(format!("dialog-{id}-{index}")))
                            .accessibility_label(label.clone())
                            .child(div().max_w(px(560.)).text_sm().text_left().child(label))
                            .on_click(move |_, _, cx| {
                                owner.update(cx, |owner, cx| {
                                    if let Some(t) = owner.threads.get_mut(&id) {
                                        t.answer(
                                            attempt,
                                            &request_id,
                                            answer.clone(),
                                            std::time::Instant::now(),
                                        );
                                    }
                                    cx.notify();
                                })
                            }),
                    );
                }
                card = card.child(options);
                if matches!(dialog.record["method"].as_str(), Some("input" | "editor")) {
                    let request_id = request_id.clone();
                    let reply_key = format!("{id}-{attempt}-{}", dialog.record["id"]);
                    let input = self.replies.get(&reply_key).unwrap();
                    card = card
                        .child(
                            div()
                                .id(SharedString::from(format!("reply-{id}")))
                                .child(Textarea::new(input).aria_label("Answer this request")),
                        )
                        .child(
                            Button::new(SharedString::from(format!("reply-submit-{id}")))
                                .primary()
                                .label("Send answer")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    let text = view
                                        .replies
                                        .get(&reply_key)
                                        .map(|input| input.read(cx).text().to_string())
                                        .unwrap_or_default();
                                    view.owner.update(cx, |owner, cx| {
                                        if let Some(t) = owner.threads.get_mut(&id) {
                                            t.answer(
                                                attempt,
                                                &request_id,
                                                json!({"value":text}),
                                                std::time::Instant::now(),
                                            );
                                        }
                                        cx.notify();
                                    });
                                })),
                        );
                }
                let owner = self.owner.clone();
                card = card.child(
                    Button::new(SharedString::from(format!("dialog-cancel-{id}")))
                        .ghost()
                        .label("Cancel")
                        .on_click(move |_, _, cx| {
                            owner.update(cx, |owner, cx| {
                                if let Some(t) = owner.threads.get_mut(&id) {
                                    t.answer(
                                        attempt,
                                        &request_id,
                                        json!({"cancelled":true}),
                                        std::time::Instant::now(),
                                    );
                                }
                                cx.notify();
                            })
                        }),
                );
                dialogs = dialogs.child(card);
            }
        }
        dialogs
    }
    pub(super) fn render_exit(&self, cx: &Context<Self>) -> Stateful<Div> {
        let owner = self.owner.read(cx);
        let mut card = div().id("exit-review").max_h(px(220.)).overflow_y_scroll().p_3().flex().flex_col().gap_2().bg(cx.theme().muted).border_b_1().border_color(cx.theme().warning)
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Stop work and exit?"))
            .child(div().text_sm().child("Exit stops admitted work. History and worktrees are preserved; owner loss is uncertain."));
        if owner.storage_pending() {
            card = card
                .child("Waiting for accepted metadata saves; the profile lock remains retained.");
        }
        if owner.storage_failed {
            card = card.child(status::notice(
                "Save outcome unproved",
                "Review the metadata save error before acknowledging exit.",
                cx.theme().warning,
                cx,
            ));
        }
        let uncertain: Vec<_> = owner
            .threads
            .iter()
            .filter(|(_, t)| t.phase == Phase::Uncertain)
            .map(|(id, _)| *id)
            .collect();
        if !uncertain.is_empty() {
            card = card.child(format!("Uncertain/interrupted shutdown outcomes: {uncertain:?}. Review before acknowledging exit."));
        }
        let confirm_owner = self.owner.clone();
        let cancel_owner = self.owner.clone();
        card.child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new("cancel-exit")
                        .ghost()
                        .label("Keep working")
                        .on_click(move |_, _, cx| {
                            cancel_owner.update(cx, |owner, cx| {
                                if !owner.confirmed_exit {
                                    owner.exit_requested = false;
                                }
                                cx.notify();
                            })
                        }),
                )
                .child(
                    Button::new("confirm-exit")
                        .danger()
                        .label(if owner.confirmed_exit {
                            "Acknowledge outcomes and exit"
                        } else {
                            "Stop and exit"
                        })
                        .on_click(move |_, _, cx| {
                            confirm_owner.update(cx, |owner, cx| {
                                owner.confirm_exit();
                                cx.notify();
                            })
                        }),
                ),
        )
    }
}
