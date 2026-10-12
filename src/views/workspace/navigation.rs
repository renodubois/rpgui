//! Compact project/thread navigation, separate from conversation actions.
use super::{WorkspaceView, create_thread, session_forms, status};
use crate::threads::Phase;
use gpui_kit::{
    assets::IconName,
    base::Disableable,
    component::{
        ActiveTheme, Icon, Selectable, Sizable,
        button::{Button, ButtonVariants},
    },
    *,
};

impl WorkspaceView {
    pub(super) fn render_sidebar(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let owner = self.owner.read(cx);
        let mut projects = div()
            .id("project-tree")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col()
            .gap_3();
        for project in &owner.metadata.projects {
            let id = project.id;
            let collapsed = self.collapsed_projects.contains(&id);
            let threads: Vec<_> = owner
                .metadata
                .threads
                .iter()
                .filter(|t| t.project == id && !t.archived)
                .collect();
            let attention = threads
                .iter()
                .filter(|t| {
                    owner
                        .threads
                        .get(&t.id)
                        .is_some_and(|t| t.phase == Phase::Attention)
                })
                .count();
            let mut heading = div().flex().items_center().gap_1().child(
                Button::new(SharedString::from(format!("project-{id}")))
                    .ghost()
                    .small()
                    .flex_1()
                    .min_w_0()
                    .accessibility_label(format!(
                        "{} project, {}",
                        project.name,
                        if collapsed { "collapsed" } else { "expanded" }
                    ))
                    .tooltip(project.directory.display().to_string())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .min_w_0()
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .size(px(12.)),
                            )
                            .child(
                                Icon::new(IconName::Folder)
                                    .size(px(14.))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(project.name.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.collapsed_projects.remove(&id) {
                            view.collapsed_projects.insert(id);
                        }
                        cx.notify();
                    })),
            );
            if attention > 0 {
                heading = heading.child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().warning)
                        .child(format!("{attention} !")),
                );
            }
            heading = heading.child(
                Button::new(SharedString::from(format!("create-{id}")))
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .accessibility_label(format!("Create thread in {}", project.name))
                    .tooltip("New thread")
                    .disabled(owner.exit_requested)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        create_thread::open(view.owner.clone(), id, window, cx)
                    })),
            );
            let mut group = div().flex().flex_col().gap_1().child(heading);
            if !collapsed {
                for saved in &threads {
                    let id = saved.id;
                    let phase = owner
                        .threads
                        .get(&id)
                        .map(|t| t.phase)
                        .unwrap_or(Phase::Passive);
                    group = group.child(
                        Button::new(SharedString::from(format!("thread-{id}")))
                            .ghost()
                            .small()
                            .w_full()
                            .h(px(32.))
                            .selected(owner.selected == Some(id))
                            .border_l_2()
                            .border_color(if owner.selected == Some(id) {
                                cx.theme().ring
                            } else {
                                cx.theme().transparent
                            })
                            .accessibility_label(format!(
                                "{}, {}",
                                saved.name,
                                status::label(phase)
                            ))
                            .tooltip(format!("{} · {}", saved.name, status::label(phase)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .w_full()
                                    .min_w_0()
                                    .child(
                                        Icon::new(status::icon(phase))
                                            .size(px(12.))
                                            .text_color(status::color(phase, cx)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(13.))
                                            .child(saved.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(11.))
                                            .text_color(status::color(phase, cx))
                                            .child(status::label(phase)),
                                    ),
                            )
                            .on_click({
                                let owner = self.owner.clone();
                                move |_, _, cx| {
                                    owner.update(cx, |owner, cx| {
                                        owner.select(id);
                                        cx.notify();
                                    });
                                }
                            }),
                    );
                }
                if threads.is_empty() {
                    group = group.child(
                        div()
                            .pl_8()
                            .py_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("No threads yet"),
                    );
                }
            }
            projects = projects.child(group);
        }
        if owner.metadata.projects.is_empty() {
            projects = projects.child(
                div()
                    .px_2()
                    .py_3()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Your projects and threads will appear here."),
            );
        }
        if let Some(error) = &self.project_picker_error {
            projects = projects.child(status::notice(
                "Folder picker unavailable",
                error.clone(),
                cx.theme().warning,
                cx,
            ));
        }
        let saved_owner = self.owner.clone();
        let mute_owner = self.owner.clone();
        div()
            .id("workspace-sidebar")
            .test_support()
            .w(px(260.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(
                                Icon::new(IconName::SquareTerminal)
                                    .size(px(18.))
                                    .text_color(cx.theme().ring),
                            )
                            .child("Workspace"),
                    )
                    .child(
                        Button::new("add-project")
                            .ghost()
                            .small()
                            .icon(IconName::FolderPlus)
                            .accessibility_label("Add project")
                            .tooltip("Add project…")
                            .disabled(self.project_picker_pending || owner.exit_requested)
                            .on_click(cx.listener(|view, _, _, cx| view.choose_project(cx))),
                    ),
            )
            .child(projects)
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("open-saved-session")
                            .ghost()
                            .small()
                            .flex_1()
                            .icon(IconName::FileClock)
                            .label("Saved session…")
                            .tooltip("Inspect a saved session (read-only)")
                            .disabled(owner.exit_requested)
                            .on_click(move |_, window, cx| {
                                session_forms::open_saved(saved_owner.clone(), window, cx)
                            }),
                    )
                    .child(
                        Button::new("notification-mute")
                            .ghost()
                            .small()
                            .icon(if owner.metadata.muted {
                                IconName::VolumeX
                            } else {
                                IconName::Volume2
                            })
                            .accessibility_label(if owner.metadata.muted {
                                "Unmute attention sounds"
                            } else {
                                "Mute attention sounds"
                            })
                            .tooltip(if owner.metadata.muted {
                                "Attention sounds muted"
                            } else {
                                "Attention sounds enabled"
                            })
                            .on_click(move |_, _, cx| {
                                mute_owner.update(cx, |owner, cx| {
                                    owner.mute();
                                    cx.notify();
                                })
                            }),
                    ),
            )
    }
}
