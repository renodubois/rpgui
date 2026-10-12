//! Isolation/status/preservation policy. Git transport stays in the Git connector.
use crate::connectors::git;
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
#[derive(Default)]
pub(crate) struct Worktrees {
    statuses: BTreeMap<u64, String>,
}
impl Worktrees {
    pub(crate) fn apply_status(&mut self, id: u64, result: Result<git::Status, String>) {
        self.statuses.insert(
            id,
            result
                .map(|status| {
                    format!(
                        "{}\n{}",
                        status.branch,
                        if status.changes.is_empty() {
                            "Clean"
                        } else {
                            &status.changes
                        }
                    )
                })
                .unwrap_or_else(|error| error),
        );
    }
    pub(crate) fn status(&self, id: u64) -> Option<&str> {
        self.statuses.get(&id).map(String::as_str)
    }
}
pub(crate) fn allocate(
    source: &Path,
    target: &Path,
    thread: u64,
    commit: &str,
) -> io::Result<PathBuf> {
    // Caller selected committed state explicitly. Archive intentionally has no cleanup operation.
    git::create(source, target, &format!("rpgui/{thread}"), commit)
}
