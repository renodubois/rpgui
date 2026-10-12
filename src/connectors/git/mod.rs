//! Scoped Git operations. No shell, copying dirty files, commits, stashes, or cleanup.
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn git(directory: &Path, args: &[&str]) -> io::Result<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "submodule.recurse=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Git output unavailable"))?;
    let reader = thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        let mut chunk = [0; 8192];
        let mut overflow = false;
        loop {
            let count = stdout.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            if bytes.len() + count <= 1024 * 1024 {
                bytes.extend_from_slice(&chunk[..count]);
            } else {
                overflow = true;
            }
        }
        if overflow {
            Err(io::Error::other("Git output exceeds limit"))
        } else {
            Ok(bytes)
        }
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other(
                "Git operation deadline; inspect state before retrying.",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = reader
        .join()
        .map_err(|_| io::Error::other("Git reader failed"))??;
    if !status.success() {
        return Err(io::Error::other(
            "Git operation failed; existing paths and changes preserved.",
        ));
    }
    String::from_utf8(output).map_err(|_| io::Error::other("Invalid Git output"))
}
#[derive(Clone, Debug)]
pub(crate) struct Status {
    pub branch: String,
    pub changes: String,
}
pub(crate) fn status(directory: &Path) -> io::Result<Status> {
    // Status may run clean/process filters while comparing working files with the index.
    let mut overrides = filter_overrides(directory)?;
    overrides.extend([
        "status".into(),
        "--porcelain=v1".into(),
        "--untracked-files=normal".into(),
    ]);
    let args: Vec<_> = overrides.iter().map(String::as_str).collect();
    Ok(Status {
        branch: git(directory, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .into(),
        changes: git(directory, &args)?,
    })
}
pub(crate) fn is_full_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn head_commit(directory: &Path) -> io::Result<String> {
    let commit = git(directory, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let commit = commit.trim().to_owned();
    if !is_full_object_id(&commit) {
        return Err(io::Error::other("Invalid current commit"));
    }
    Ok(commit)
}
pub(crate) fn create(
    directory: &Path,
    target: &Path,
    branch: &str,
    selected_commit: &str,
) -> io::Result<PathBuf> {
    // A literal immutable object ID is required: no moving HEAD, option injection or revision expressions.
    if !is_full_object_id(selected_commit) {
        return Err(io::Error::other(
            "Select an explicit full committed object ID.",
        ));
    }
    if target.exists() || !target.is_absolute() || !branch.starts_with("rpgui/") {
        return Err(io::Error::other(
            "Worktree destination must be new and absolute; branch must be rpgui-scoped.",
        ));
    }
    git(directory, &["check-ref-format", "--branch", branch])?;
    let kind = git(directory, &["cat-file", "-t", selected_commit])?;
    if kind.trim() != "commit" {
        return Err(io::Error::other("Selected object is not a commit."));
    }
    let target_text = target
        .to_str()
        .ok_or_else(|| io::Error::other("Non-UTF8 worktree path"))?;
    // Git checkout can execute configured smudge/process filters before Pi resource trust runs.
    // Disable those invocation-locally; retain committed blobs (including LFS pointers), never edit Git config.
    let mut overrides = filter_overrides(directory)?;
    overrides.extend([
        "worktree".to_owned(),
        "add".into(),
        "-b".into(),
        branch.into(),
        target_text.into(),
        selected_commit.into(),
    ]);
    let args: Vec<_> = overrides.iter().map(String::as_str).collect();
    git(directory, &args)?;
    std::fs::canonicalize(target)
}
fn filter_overrides(directory: &Path) -> io::Result<Vec<String>> {
    let keys = git(directory, &["config", "--name-only", "--list"])?;
    let mut overrides = Vec::new();
    let mut filters = std::collections::HashSet::new();
    for key in keys.lines() {
        if let Some(rest) = key.strip_prefix("filter.")
            && let Some((name, field)) = rest.rsplit_once('.')
            && ["smudge", "process", "clean", "required"].contains(&field)
        {
            filters.insert(name.to_owned());
        }
    }
    for filter in filters {
        for field in ["smudge", "process", "clean"] {
            overrides.push("-c".to_owned());
            overrides.push(format!("filter.{filter}.{field}="));
        }
        overrides.push("-c".to_owned());
        overrides.push(format!("filter.{filter}.required=false"));
    }
    Ok(overrides)
}
#[cfg(test)]
#[path = "tests/worktrees.rs"]
mod tests;
