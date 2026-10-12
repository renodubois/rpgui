use super::*;
#[test]
fn canonical_lock_conflict_and_release() {
    let temp = tempfile::tempdir().unwrap();
    let owner = Profile::open(temp.path()).unwrap();
    assert!(Profile::open(&temp.path().join(".")).is_err());
    drop(owner);
    assert!(Profile::open(temp.path()).is_ok());
    assert!(temp.path().join("profile.lock").exists());
}
#[test]
fn metadata_roundtrip_and_unknown_sessions_refused() {
    let temp = tempfile::tempdir().unwrap();
    let profile = Profile::open(temp.path()).unwrap();
    let mut state = profile.load().unwrap();
    state.muted = true;
    profile.save(&state).unwrap();
    assert!(profile.load().unwrap().muted);
    let unknown = temp.path().join("terminal.jsonl");
    fs::write(&unknown, "history").unwrap();
    assert!(profile.owned_session(1, &unknown).is_err());
    let session = profile.session_directory(1).unwrap().join("pi.jsonl");
    fs::write(&session, "history").unwrap();
    assert_eq!(profile.owned_session(1, &session).unwrap(), session);
}
#[test]
fn invalid_version_and_storage_failure_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let profile = Profile::open(temp.path()).unwrap();
    fs::write(
        temp.path().join("workspace.json"),
        r#"{"version":2,"projects":[],"threads":[],"muted":false,"next_id":0}"#,
    )
    .unwrap();
    assert!(profile.load().is_err());
    fs::create_dir(temp.path().join("workspace.json.next")).unwrap();
    assert!(profile.save(&Metadata::default()).is_err());
}
#[test]
fn private_namespace_symlinks_and_oversized_metadata_are_refused() {
    let temp = tempfile::tempdir().unwrap();
    let profile = Profile::open(temp.path()).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("sessions")).unwrap();
    assert!(profile.session_directory(7).is_err());
    assert!(!outside.path().join("7").exists());
    let mut metadata = Metadata {
        version: 1,
        ..Default::default()
    };
    metadata.projects.push(Project {
        id: 1,
        name: "x".repeat(1024 * 1024),
        directory: temp.path().into(),
    });
    assert!(profile.save(&metadata).is_err());
    assert!(!temp.path().join("workspace.json").exists());
}
