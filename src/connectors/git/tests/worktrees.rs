use super::*;
#[test]
fn current_commit_defaults_are_full_and_require_a_commit() {
    let temp = tempfile::tempdir().unwrap();
    assert!(head_commit(temp.path()).is_err());
    git(temp.path(), &["init", "-b", "main"]).unwrap();
    assert!(head_commit(temp.path()).is_err());
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "fixture",
        ],
    )
    .unwrap();
    let commit = head_commit(temp.path()).unwrap();
    assert!(is_full_object_id(&commit));
    assert_eq!(
        commit,
        git(temp.path(), &["rev-parse", "HEAD"]).unwrap().trim()
    );
    for invalid in ["", "HEAD", "main", "abc123", &"z".repeat(40)] {
        assert!(!is_full_object_id(invalid));
    }
}
#[test]
fn committed_worktree_excludes_dirty_files_and_is_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("repo");
    std::fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]).unwrap();
    std::fs::write(source.join("tracked"), "committed").unwrap();
    git(&source, &["add", "tracked"]).unwrap();
    git(
        &source,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            "fixture",
        ],
    )
    .unwrap();
    let base = git(&source, &["rev-parse", "HEAD"]).unwrap();
    std::fs::write(source.join("tracked"), "dirty").unwrap();
    std::fs::write(source.join("untracked"), "private").unwrap();
    let target = temp.path().join("worktree");
    create(&source, &target, "rpgui/1", base.trim()).unwrap();
    assert_eq!(
        std::fs::read_to_string(target.join("tracked")).unwrap(),
        "committed"
    );
    assert!(!target.join("untracked").exists());
    assert_eq!(status(&target).unwrap().branch, "rpgui/1");
    assert!(status(&source).unwrap().changes.contains("tracked"));
    assert!(create(&source, &target, "rpgui/2", base.trim()).is_err());
    assert!(create(&source, &temp.path().join("bad"), "rpgui/3", "HEAD").is_err());
    assert_eq!(
        std::fs::read_to_string(source.join("tracked")).unwrap(),
        "dirty"
    );
}
#[test]
fn non_git_and_invalid_base_fail_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    assert!(status(temp.path()).is_err());
    assert!(
        create(
            temp.path(),
            &temp.path().join("new"),
            "rpgui/1",
            &"0".repeat(40)
        )
        .is_err()
    );
    assert!(!temp.path().join("new").exists());
}
#[test]
fn checkout_filters_and_status_monitors_do_not_execute_before_trust() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("repo");
    std::fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]).unwrap();
    std::fs::write(source.join(".gitattributes"), "filtered filter=untrusted\n").unwrap();
    std::fs::write(source.join("filtered"), "committed blob").unwrap();
    git(&source, &["add", "."]).unwrap();
    git(
        &source,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    )
    .unwrap();
    let marker = temp.path().join("unexpected-startup");
    let command = format!("touch {}; cat", marker.display());
    git(&source, &["config", "filter.untrusted.smudge", &command]).unwrap();
    git(&source, &["config", "core.fsmonitor", &command]).unwrap();
    let commit = git(&source, &["rev-parse", "HEAD"]).unwrap();
    let target = temp.path().join("checkout");
    create(&source, &target, "rpgui/guarded", commit.trim()).unwrap();
    // Force status to compare content rather than trusting the cached index stat.
    git(&source, &["config", "filter.untrusted.clean", &command]).unwrap();
    git(&source, &["config", "filter.untrusted.required", "true"]).unwrap();
    std::fs::write(source.join("filtered"), "changed working content").unwrap();
    assert!(status(&source).unwrap().changes.contains("filtered"));
    assert!(!marker.exists());
    // A process filter is another status execution seam, not just a checkout seam.
    git(&source, &["config", "filter.untrusted.process", &command]).unwrap();
    assert!(status(&source).unwrap().changes.contains("filtered"));
    assert!(!marker.exists());
    assert_eq!(
        std::fs::read_to_string(target.join("filtered")).unwrap(),
        "committed blob"
    );
}
