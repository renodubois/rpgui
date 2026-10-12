use super::*;
#[test]
fn creation_defaults_ignore_stale_results_and_validation_has_no_side_effects() {
    let temp = tempfile::tempdir().unwrap();
    let mut owner = Workspace::new(Profile::open(&temp.path().join("profile")).unwrap()).unwrap();
    owner.add_project(temp.path().into());
    let project = owner.metadata.projects[0].id;
    let first = owner.prepare_creation(project);
    let second = owner.prepare_creation(project);
    owner.poll(Instant::now());
    assert!(
        owner
            .creation_base
            .as_ref()
            .is_none_or(|(request, _)| *request == second)
    );
    poll_until(&mut owner, |w| w.creation_base.is_some());
    assert!(second > first);
    assert!(owner.creation_base.as_ref().unwrap().1.is_err());
    let next_id = owner.metadata.next_id;
    owner.create(project, " ".into(), None);
    owner.create(project, "test".into(), Some("HEAD".into()));
    assert!(owner.threads.is_empty());
    assert_eq!(owner.metadata.next_id, next_id);
    owner.create(project, "shared".into(), None);
    poll_until(&mut owner, |w| w.opening.is_empty());
    assert_eq!(owner.metadata.threads.len(), 1);
    assert!(!owner.metadata.threads[0].isolated);
}
#[test]
fn deterministic_launch_path_failures_do_not_poison_owner_admission() {
    let temp = tempfile::tempdir().unwrap();
    let profile = Profile::open(&temp.path().join("profile")).unwrap();
    let existing = temp.path().to_path_buf();
    let missing = temp.path().join("missing");
    let saved = SavedThread {
        id: 1,
        project: 1,
        name: "test".into(),
        directory: existing.clone(),
        session: None,
        archived: false,
        interrupted: false,
        isolated: false,
        trusted: false,
    };
    let metadata = Metadata {
        version: 1,
        ..Default::default()
    };
    for (pi, agent) in [
        (missing.clone(), existing.clone()),
        (existing.clone(), missing),
    ] {
        assert!(launch_at(&profile, &metadata, &saved, &[], pi, agent).is_err());
        assert!(
            crate::storage::owners::admission(
                &profile.root().join("owners"),
                &crate::storage::owners::identity(std::process::id()).unwrap(),
                &|_| false,
            )
            .is_ok()
        );
    }
}
#[test]
fn runtime_hook_refusal_leaves_no_incomplete_attempt() {
    const CHILD: &str = "RPGUI_TEST_RUNTIME_HOOK_REFUSAL";
    if std::env::var_os(CHILD).is_none() {
        // Isolate process environment changes from the parallel test suite.
        for hook in ["NODE_OPTIONS", "NODE_PATH"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "workspace::tests::runtime_hook_refusal_leaves_no_incomplete_attempt",
                ])
                .env(CHILD, "1")
                .env_remove("NODE_OPTIONS")
                .env_remove("NODE_PATH")
                .env(hook, "unreviewed-hook")
                .status()
                .unwrap();
            assert!(status.success());
        }
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let profile = Profile::open(&temp.path().join("profile")).unwrap();
    let saved = SavedThread {
        id: 1,
        project: 1,
        name: "test".into(),
        directory: temp.path().into(),
        session: None,
        archived: false,
        interrupted: false,
        isolated: false,
        trusted: false,
    };
    let metadata = Metadata {
        version: 1,
        ..Default::default()
    };
    let error = launch_at(
        &profile,
        &metadata,
        &saved,
        &[],
        temp.path().into(),
        temp.path().into(),
    )
    .err()
    .unwrap();
    assert!(error.contains("unsupported runtime hooks"));
    assert_eq!(
        std::fs::read_dir(profile.root().join("owners/1"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        crate::storage::owners::admission(
            &profile.root().join("owners"),
            &crate::storage::owners::identity(std::process::id()).unwrap(),
            &|_| false,
        )
        .is_ok()
    );
}
#[test]
fn disconnected_close_and_provider_error_cannot_bypass_exit_uncertainty() {
    let temp = tempfile::tempdir().unwrap();
    let mut owner = Workspace::new(Profile::open(temp.path()).unwrap()).unwrap();
    let mut thread = Thread::passive(1);
    thread.phase = Phase::Failed;
    thread.apply(
        0,
        crate::connectors::pi::Update::Failed("retained owner lost"),
        Instant::now(),
    );
    thread.close();
    owner.threads.insert(1, thread);
    owner.request_exit();
    owner.confirm_exit();
    assert!(!owner.ready_to_exit());
    assert_eq!(owner.threads[&1].phase, Phase::Uncertain);
    owner.confirm_exit();
    assert!(owner.ready_to_exit());
}

fn poll_until(owner: &mut Workspace, condition: impl Fn(&Workspace) -> bool) {
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    while !condition(owner) {
        owner.poll(Instant::now());
        assert!(Instant::now() < deadline, "worker deadline");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn project_threads_archive_preserves_directory_and_passive_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let profile = temp.path().join("profile");
    let mut owner = Workspace::new(Profile::open(&profile).unwrap()).unwrap();
    owner.add_project(project.clone());
    let project_id = owner.metadata.projects[0].id;
    owner.create(project_id, "first".into(), None);
    poll_until(&mut owner, |w| w.threads.len() == 1 && w.opening.is_empty());
    let id = owner.metadata.threads[0].id;
    owner.rename(id, "renamed".into());
    owner.select(id);
    owner.archive(id);
    assert!(owner.metadata.threads[0].archived);
    assert!(project.exists());
    assert!(owner.threads.values().all(|t| !t.live()));
    drop(owner);
    // Ordered worker may still hold profile lock until it drains queued saves.
    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    let profile = loop {
        if let Ok(p) = Profile::open(&profile) {
            break p;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let reopened = Workspace::new(profile).unwrap();
    assert_eq!(reopened.metadata.threads[0].name, "renamed");
    assert!(reopened.metadata.threads[0].archived);
    assert!(
        reopened
            .threads
            .values()
            .all(|t| t.phase == Phase::Passive && !t.live())
    );
}
#[test]
fn hidden_attention_is_transition_based_and_selection_does_not_end_work() {
    let temp = tempfile::tempdir().unwrap();
    let mut owner = Workspace::new(Profile::open(temp.path()).unwrap()).unwrap();
    owner.threads.insert(1, Thread::passive(1));
    owner.threads.insert(2, Thread::passive(2));
    owner.threads.get_mut(&1).unwrap().phase = Phase::Running;
    owner.select(2);
    assert_eq!(owner.threads[&1].phase, Phase::Running);
    owner.select(1);
    owner.select(2);
    assert_eq!(owner.threads[&1].phase, Phase::Running);
    owner.request_exit();
    assert!(!owner.ready_to_exit());
    owner.confirm_exit();
    assert!(owner.ready_to_exit());
}
#[test]
fn exit_waits_for_metadata_failure_then_requires_explicit_acknowledgement() {
    let temp = tempfile::tempdir().unwrap();
    let mut owner = Workspace::new(Profile::open(temp.path()).unwrap()).unwrap();
    std::fs::create_dir(temp.path().join("workspace.json.next")).unwrap();
    owner.mute();
    owner.request_exit();
    owner.confirm_exit();
    assert!(!owner.ready_to_exit());
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while owner.storage_pending() {
        owner.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(owner.storage_failed);
    assert!(!owner.ready_to_exit());
    owner.confirm_exit();
    assert!(owner.ready_to_exit());
}
#[test]
fn unknown_external_history_is_read_only_without_adoption_or_replay() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().join("profile");
    let path = temp.path().join("external.jsonl");
    let original = "{\"type\":\"session\",\"cwd\":\"/saved-cwd\"}\n{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":\"External saved content\"}}\n";
    std::fs::write(&path, original).unwrap();
    let mut owner = Workspace::new(Profile::open(&profile).unwrap()).unwrap();
    owner.open_saved(path.clone());
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while owner.external_history.is_none() {
        owner.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(owner.threads.is_empty());
    assert!(owner.metadata.threads.is_empty());
    assert!(
        owner
            .external_history
            .as_ref()
            .unwrap()
            .1
            .markdown
            .contains("External saved content")
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}
#[test]
fn removed_pending_image_cannot_reappear_from_a_late_read() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.png");
    std::fs::write(&path, b"\x89PNG\r\n\x1a\nsynthetic").unwrap();
    let mut owner = Workspace::new(Profile::open(temp.path()).unwrap()).unwrap();
    owner.threads.insert(2, Thread::passive(2));
    owner.attach_image(2, path);
    assert!(owner.threads[&2].image_pending);
    owner.threads.get_mut(&2).unwrap().remove_image();
    let deadline = Instant::now() + std::time::Duration::from_secs(1);
    while Instant::now() < deadline {
        owner.poll(Instant::now());
        std::thread::yield_now();
    }
    assert!(owner.threads[&2].images.is_empty());
    assert!(!owner.threads[&2].image_pending);
}
