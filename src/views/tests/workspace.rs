use super::WorkspaceView;
use crate::{
    storage::workspace::{Profile, Project, Thread as SavedThread},
    threads::Thread,
};
use crate::{threads::Phase, workspace::Workspace};
use gpui_kit::{
    AppContext, ScrollDelta, TestAppContext,
    component::{Root, WindowExt},
    point, px, size,
    test::TestWindowExt,
};
use serde_json::json;

#[gpui_kit::test]
fn create_button_opens_prefilled_dialog_and_cancel_does_not_create(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let mut workspace =
        Workspace::new(Profile::open(&temp.path().join("profile")).unwrap()).unwrap();
    workspace.metadata.projects.push(Project {
        id: 1,
        name: "Fixture".into(),
        directory: temp.path().into(),
    });
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("create-1", cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx), "dialog must open");
        assert!(window.try_find("create-thread-submit").is_some());
        assert!(owner.read(cx).metadata.threads.is_empty());
        window.click("create-thread-submit", cx); // Loading/default unavailable: no admission.
        assert!(owner.read(cx).metadata.threads.is_empty());
        window.click("create-thread-cancel", cx);
        assert!(owner.read(cx).metadata.threads.is_empty());
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("create-1", cx);
        owner.update(cx, |owner, cx| {
            owner.creation_base = Some((2, Err("No current commit available.".into())));
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        window.click("create-thread-submit", cx);
        assert!(owner.read(cx).metadata.threads.is_empty());
        window.click("create-directory-mode", cx);
        window.click("create-thread-submit", cx);
        assert_eq!(owner.read(cx).metadata.threads.len(), 1);
        assert!(!owner.read(cx).metadata.threads[0].isolated);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("create-1", cx);
        owner.update(cx, |owner, cx| {
            owner.creation_base = Some((3, Ok("a".repeat(40))));
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        window.click("create-thread-submit", cx);
        assert_eq!(owner.read(cx).metadata.threads.len(), 2);
        assert_eq!(owner.read(cx).metadata.threads[1].name, "New thread");
        assert!(owner.read(cx).metadata.threads[1].isolated);
    });
}

#[gpui_kit::test]
fn project_folder_picker_adds_selection_and_cancel_or_exit_does_not_mutate(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("Project with spaces – café");
    std::fs::create_dir(&project).unwrap();
    let profile = Profile::open(&temp.path().join("profile")).unwrap();
    let workspace = Workspace::new(profile).unwrap();
    cx.update(gpui_kit::init);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let mut screen = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("add-project", cx);
    });
    assert!(visual.did_prompt_for_paths());
    visual.simulate_path_prompt_response(|options| {
        assert!(!options.files);
        assert!(options.directories);
        assert!(!options.multiple);
        None
    });
    visual.run_until_parked();
    assert!(visual.update(|_, cx| owner.read(cx).metadata.projects.is_empty()));
    assert!(!visual.update(|_, cx| screen.as_ref().unwrap().read(cx).project_picker_pending));

    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("add-project", cx);
    });
    visual.simulate_path_prompt_response(|_| Some(vec![project.clone()]));
    visual.run_until_parked();
    visual.update(|_, cx| {
        let state = owner.read(cx);
        assert_eq!(state.metadata.projects.len(), 1);
        assert_eq!(
            state.metadata.projects[0].directory,
            std::fs::canonicalize(&project).unwrap()
        );
        assert!(state.threads.is_empty()); // Selecting a project never launches execution.
    });

    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("add-project", cx);
    });
    visual.simulate_path_prompt_response(|_| Some(vec![project.clone()]));
    visual.run_until_parked();
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).metadata.projects.len()),
        1
    );
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).warning.clone()),
        Some("Project already exists.".into())
    );

    // Revalidate selections in the feature owner even if a platform returns a file.
    let file = temp.path().join("not-a-directory");
    std::fs::write(&file, "fixture").unwrap();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("add-project", cx);
    });
    visual.simulate_path_prompt_response(|_| Some(vec![file]));
    visual.run_until_parked();
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).metadata.projects.len()),
        1
    );
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).warning.clone()),
        Some("Choose an existing project directory.".into())
    );

    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("add-project", cx);
        owner.update(cx, |owner, _| owner.request_exit());
    });
    visual.simulate_path_prompt_response(|_| Some(vec![temp.path().into()]));
    visual.run_until_parked();
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).metadata.projects.len()),
        1
    );
}

#[gpui_kit::test]
fn native_controls_route_hidden_dialog_and_recreated_view_keeps_thread_owner(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let mut owner = Workspace::new(Profile::open(temp.path()).unwrap()).unwrap();
    owner.metadata.muted = true;
    owner.metadata.projects.push(Project {
        id: 1,
        name: "Disposable".into(),
        directory: temp.path().into(),
    });
    for id in [2, 3] {
        owner.metadata.threads.push(SavedThread {
            id,
            project: 1,
            name: format!("Thread {id}"),
            directory: temp.path().into(),
            session: None,
            archived: false,
            interrupted: false,
            isolated: false,
            trusted: false,
        });
        owner.threads.insert(id, Thread::passive(id));
    }
    owner.selected = Some(2);
    owner.threads.get_mut(&2).unwrap().phase = Phase::Running;
    owner.threads.get_mut(&3).unwrap().apply(0,crate::connectors::pi::Update::Record(json!({"type":"extension_ui_request","id":"hidden","method":"select","title":"Hidden question","options":["Reject"]})),std::time::Instant::now());
    cx.update(gpui_kit::init);
    let owner = cx.update(|cx| cx.new(|_| owner));
    let mut screen = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("dialog-3-0").is_some());
        window.click("thread-3", cx);
    });
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).threads[&2].phase),
        Phase::Running
    );
    visual.update(|window, cx| {
        screen.as_ref().unwrap().update(cx, |view, cx| {
            *view = WorkspaceView::new(owner.clone(), window, cx);
            cx.notify();
        })
    });
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("dialog-cancel-3", cx);
    });
    assert!(visual.update(|_, cx| owner.read(cx).threads[&3].dialog.is_none()));
    assert_eq!(
        visual.update(|_, cx| owner.read(cx).threads[&2].phase),
        Phase::Running
    );
}

fn refresh_fixture(root: &std::path::Path) -> Workspace {
    let mut owner = Workspace::new(Profile::open(&root.join("profile")).unwrap()).unwrap();
    owner.metadata.projects.push(Project {
        id: 1,
        name: "Fixture project".into(),
        directory: root.into(),
    });
    for id in [2, 3] {
        owner.metadata.threads.push(SavedThread {
            id,
            project: 1,
            name: format!("Thread {id}"),
            directory: root.into(),
            session: None,
            archived: false,
            interrupted: false,
            isolated: false,
            trusted: false,
        });
        let mut thread = Thread::passive(id);
        thread.draft = format!("Draft {id}");
        thread.text =
            "## Fixture conversation\n\nA bounded paragraph of saved history.\n\n".repeat(30);
        owner.threads.insert(id, thread);
    }
    owner.selected = Some(2);
    owner
}
fn init_refresh_ui(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::init(cx);
        crate::views::init(cx);
        cx.set_reduce_motion(true);
    });
}

#[gpui_kit::test]
fn compact_workspace_keeps_primary_controls_in_bounds_at_small_sizes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = refresh_fixture(temp.path());
    init_refresh_ui(cx);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    for (width, height) in [(1200., 840.), (900., 700.), (680., 520.)] {
        visual.simulate_resize(size(px(width), px(height)));
        visual.update(|window, cx| {
            window.render_frame(cx);
            window.render_frame(cx);
            assert_eq!(
                window.find("workspace-sidebar").bounds().size.width,
                px(260.)
            );
            let send = window.find("send-message");
            assert!(send.visible());
            window.click("send-message", cx); // A passive composer is inert, not an implicit runtime open.
            assert!(owner.read(cx).threads[&2].error.is_none());
            assert!(send.bounds().origin.x >= px(260.));
            assert!(send.bounds().bottom_right().x <= px(width));
            assert!(send.bounds().bottom_right().y <= px(height));
            assert!(window.find("conversation-scroll").bounds().size.height > px(150.));
            assert!(window.try_find("thread-details").is_none());
            assert!(window.try_find("saved-session-path").is_none());
            assert!(window.try_find("rename-thread-name").is_none());
            assert_eq!(owner.read(cx).selected, Some(2));
            assert_eq!(window.find("add-project").label(), Some("Add project"));
        });
    }
    assert!(!visual.update(|_, cx| owner.read(cx).threads[&2].live()));
}

#[gpui_kit::test]
fn panels_and_delivery_controls_are_local_and_selection_restores_the_right_draft(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().unwrap();
    let mut workspace = refresh_fixture(temp.path());
    workspace
        .threads
        .get_mut(&2)
        .unwrap()
        .steering
        .push("Pending steering".into());
    workspace
        .threads
        .get_mut(&2)
        .unwrap()
        .follow_up
        .push("Pending follow-up".into());
    init_refresh_ui(cx);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let mut screen = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("queued-messages").is_none());
        window.click("queue-toggle", cx);
        window.click("delivery-mode", cx);
        window.click("attachments-toggle", cx);
        window.click("model-settings", cx);
        window.click("thread-details-toggle", cx);
        window.click("inspect-toggle", cx);
        window.click("follow-latest", cx);
        window.render_frame(cx);
        assert!(window.try_find("queued-messages").is_some());
        assert!(window.try_find("thread-details").is_some());
        assert!(window.try_find("model-picker").is_some());
        window.click("inspect-child", cx);
        assert!(owner.read(cx).threads[&2].error.is_none());
        assert_eq!(window.find("delivery-mode").checked(), Some(true));
        assert_eq!(window.find("delivery-steer").checked(), Some(false));
        assert_eq!(owner.read(cx).selected, Some(2));
        assert_eq!(owner.read(cx).threads[&2].steering, ["Pending steering"]);
        window.click("thread-3", cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let view = screen.as_ref().unwrap().read(cx);
        assert!(
            !view.inspect && !view.details && !view.settings && !view.attachments && !view.queues
        );
        assert!(view.follow_latest);
        assert_eq!(view.prompt.read(cx).text().to_string(), "Draft 3");
        assert_eq!(owner.read(cx).threads[&2].draft, "Draft 2");
        assert!(window.try_find("image-path").is_none());
        assert!(window.try_find("inspect-child").is_none());
        assert_eq!(owner.read(cx).selected, Some(3));
        window.click("project-1", cx);
        window.render_frame(cx);
        assert!(window.try_find("thread-3").is_none());
        assert_eq!(owner.read(cx).selected, Some(3)); // Collapsing navigation never changes the conversation.
    });
}

#[gpui_kit::test]
fn menu_rename_stays_bound_to_original_thread_when_selection_changes(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = refresh_fixture(temp.path());
    init_refresh_ui(cx);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("thread-menu-2", cx);
        window.render_frame(cx);
        window.within("popup-menu").click(1_usize, cx); // Rename, after the non-interactive Thread heading.
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.has_active_dialog(cx));
        assert_eq!(window.find("rename-thread-name").value(), Some("Thread 2"));
        owner.update(cx, |owner, cx| {
            owner.select(3);
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("rename-thread-name", cx);
        window.press("ctrl-a", cx);
        window.input("Renamed original", cx);
        window.render_frame(cx);
        window.click("rename-thread-submit", cx);
        assert_eq!(owner.read(cx).selected, Some(3));
        assert_eq!(owner.read(cx).metadata.threads[0].name, "Renamed original");
        assert_eq!(owner.read(cx).metadata.threads[1].name, "Thread 3");
        assert!(!owner.read(cx).threads[&2].live());
    });
}

#[gpui_kit::test]
fn saved_session_form_is_on_demand_and_cancel_or_exit_does_not_read(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = refresh_fixture(temp.path());
    init_refresh_ui(cx);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("saved-session-path").is_none());
        window.click("open-saved-session", cx);
        window.render_frame(cx);
        window.click("saved-session-submit", cx);
        assert!(window.has_active_dialog(cx));
        window.click("saved-session-cancel", cx);
        assert_eq!(owner.read(cx).selected, Some(2));
        assert!(owner.read(cx).external_history.is_none());
        window.click("open-saved-session", cx);
        window.render_frame(cx);
        window.click("saved-session-path", cx);
        window.input(temp.path().join("nonexistent.jsonl").to_str().unwrap(), cx);
        window.render_frame(cx);
        assert_eq!(
            window.find("saved-session-path").value(),
            temp.path().join("nonexistent.jsonl").to_str()
        );
        owner.update(cx, |owner, cx| {
            owner.request_exit();
            cx.notify();
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("saved-session-submit", cx);
        assert!(owner.read(cx).external_history.is_none());
        assert!(owner.read(cx).warning.is_none());
        window.click("saved-session-cancel", cx);
    });
}

#[gpui_kit::test]
fn ctrl_enter_preserves_passive_draft_and_scrolling_pauses_following(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = refresh_fixture(temp.path());
    init_refresh_ui(cx);
    let owner = cx.update(|cx| cx.new(|_| workspace));
    let mut screen = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| WorkspaceView::new(owner.clone(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        window.click("conversation-input", cx);
        window.press("ctrl-enter", cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(
            screen
                .as_ref()
                .unwrap()
                .read(cx)
                .prompt
                .read(cx)
                .text()
                .to_string(),
            "Draft 2"
        );
        assert_eq!(owner.read(cx).threads[&2].phase, Phase::Passive);
        assert!(owner.read(cx).threads[&2].error.is_none());
        window.press("enter", cx);
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(owner.read(cx).threads[&2].draft.contains('\n')); // Ordinary Enter remains multiline editing.
        window.scroll(
            "conversation-scroll",
            ScrollDelta::Pixels(point(px(0.), px(120.))),
            cx,
        );
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(!screen.as_ref().unwrap().read(cx).follow_latest);
        window.click("follow-latest", cx);
        window.render_frame(cx);
        assert!(screen.as_ref().unwrap().read(cx).follow_latest);
    });
}
