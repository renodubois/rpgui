//! Infrequent thread actions, bound to an exact thread rather than current selection.
use super::session_forms;
use crate::{threads::Phase, workspace::Workspace};
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenuItem},
    },
    *,
};

pub(super) fn trigger(owner: Entity<Workspace>, id: u64) -> impl IntoElement {
    Button::new(SharedString::from(format!("thread-menu-{id}")))
        .ghost()
        .small()
        .icon(IconName::Ellipsis)
        .accessibility_label("Thread actions")
        .tooltip("Thread actions")
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, cx| {
            let state = owner.read(cx);
            let live = state.threads.get(&id).is_some_and(|t| t.live());
            let idle = state
                .threads
                .get(&id)
                .is_some_and(|t| matches!(t.phase, Phase::Idle | Phase::Failed));
            let exiting = state.exit_requested;
            let trusted = state
                .metadata
                .threads
                .iter()
                .find(|t| t.id == id)
                .is_some_and(|t| t.trusted);
            let rename_owner = owner.clone();
            let compact_owner = owner.clone();
            let trust_owner = owner.clone();
            let close_owner = owner.clone();
            let archive_owner = owner.clone();
            menu.min_w(px(230.))
                .label("Thread")
                .item(
                    PopupMenuItem::new("Rename…")
                        .icon(IconName::Pencil)
                        .disabled(exiting)
                        .on_click(move |_, window, cx| {
                            session_forms::open_rename(rename_owner.clone(), id, window, cx)
                        }),
                )
                .item(
                    PopupMenuItem::new("Compact context")
                        .icon(IconName::Minimize2)
                        .disabled(exiting || !live || !idle)
                        .on_click(move |_, _, cx| {
                            compact_owner.update(cx, |owner, cx| {
                                if !owner.exit_requested
                                    && let Some(t) = owner.threads.get_mut(&id)
                                {
                                    t.compact();
                                }
                                cx.notify();
                            })
                        }),
                )
                .item(
                    PopupMenuItem::new(if trusted {
                        "Disable trusted project resources"
                    } else {
                        "Enable trusted project resources"
                    })
                    .icon(IconName::ShieldCheck)
                    .disabled(exiting || live)
                    .on_click(move |_, _, cx| {
                        trust_owner.update(cx, |owner, cx| {
                            if !owner.exit_requested {
                                owner.toggle_trust(id);
                            }
                            cx.notify();
                        })
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Close runtime")
                        .icon(IconName::Square)
                        .disabled(exiting || !live)
                        .on_click(move |_, _, cx| {
                            close_owner.update(cx, |owner, cx| {
                                if !owner.exit_requested
                                    && let Some(t) = owner.threads.get_mut(&id)
                                {
                                    t.close();
                                }
                                cx.notify();
                            })
                        }),
                )
                .item(
                    PopupMenuItem::new("Archive thread")
                        .icon(IconName::Archive)
                        .disabled(exiting || live)
                        .on_click(move |_, _, cx| {
                            archive_owner.update(cx, |owner, cx| {
                                if !owner.exit_requested {
                                    owner.archive(id);
                                }
                                cx.notify();
                            })
                        }),
                )
                .separator()
                .label("History and worktrees are preserved")
        })
}
