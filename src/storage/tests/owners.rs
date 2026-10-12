use super::*;
#[test]
fn nonregular_and_symlink_owner_evidence_is_refused_without_blocking() {
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::symlink},
    };
    let temp = tempfile::tempdir().unwrap();
    let fifo = temp.path().join("owner.json");
    let name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a valid NUL-terminated path in this disposable directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(read(&fifo).is_err());
    assert!(read(temp.path()).is_err());
    let regular = temp.path().join("regular.json");
    fs::write(&regular, r#"{"version":1}"#).unwrap();
    assert!(read(&regular).is_ok());
    let link = temp.path().join("link.json");
    symlink(&regular, &link).unwrap();
    assert!(read(&link).is_err());
}

#[test]
fn owner_loss_and_unproved_cleanup_refuse_all_admission() {
    let temp = tempfile::tempdir().unwrap();
    let scope = temp.path().join("1/attempt");
    fs::create_dir_all(&scope).unwrap();
    let current = identity(std::process::id()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_err());
    let owner = json!({"version":1,"scope":scope,"frontend":current,"owner":current,"pi":current,"phase":"running"});
    fs::write(scope.join("guardian-owner.json"), owner.to_string()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_ok());
    assert!(admission(temp.path(), &current, &|_| false).is_err());
    let mut stale = owner.clone();
    stale["owner"]["startTicks"] = json!("wrong-attempt");
    fs::write(scope.join("guardian-owner.json"), stale.to_string()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_err());
    stale["phase"] = json!("runner-observation-complete");
    fs::write(scope.join("guardian-owner.json"), stale.to_string()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_err());
    fs::write(
        scope.join("guardian-proof.json"),
        json!({"uncertain":true,"parent":{"signal":null}}).to_string(),
    )
    .unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_err());
}
#[test]
fn explicit_pre_start_refusal_is_not_an_uncertain_dispatch() {
    let temp = tempfile::tempdir().unwrap();
    let scope = temp.path().join("1/attempt");
    fs::create_dir_all(&scope).unwrap();
    fs::write(
        scope.join("owner-refusal.json"),
        json!({"version":1,"admitted":false}).to_string(),
    )
    .unwrap();
    assert!(
        admission(temp.path(), &identity(std::process::id()).unwrap(), &|_| {
            false
        })
        .is_ok()
    );
}

#[test]
fn legacy_positive_proof_does_not_upgrade_to_descendant_shutdown_proof() {
    let temp = tempfile::tempdir().unwrap();
    let scope = temp.path().join("1/attempt");
    fs::create_dir_all(&scope).unwrap();
    let current = identity(std::process::id()).unwrap();
    let mut old = current.clone();
    old["startTicks"] = json!("not-this-attempt");
    fs::write(scope.join("guardian-owner.json"),json!({"version":1,"scope":scope,"frontend":current,"owner":old,"pi":old,"phase":"runner-observation-complete"}).to_string()).unwrap();
    let mut proof =
        json!({"type":"rpgui_owner_done","uncertain":false,"parent":{"code":0,"signal":null}});
    fs::write(scope.join("guardian-proof.json"), proof.to_string()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_err());
    proof["descendants"] = json!({"proof":"retained-linux-subreaper-pidfd","uncertain":false});
    fs::write(scope.join("guardian-proof.json"), proof.to_string()).unwrap();
    assert!(admission(temp.path(), &current, &|_| true).is_ok());
}
