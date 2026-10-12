//! GUI metadata only. Pi owns conversation history. One stable kernel lock per canonical profile.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub version: u32,
    pub projects: Vec<Project>,
    pub threads: Vec<Thread>,
    pub muted: bool,
    pub next_id: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Project {
    pub id: u64,
    pub name: String,
    pub directory: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Thread {
    pub id: u64,
    pub project: u64,
    pub name: String,
    pub directory: PathBuf,
    pub session: Option<PathBuf>,
    pub archived: bool,
    pub interrupted: bool,
    pub isolated: bool,
    #[serde(default)]
    pub trusted: bool,
}
pub(crate) struct Profile {
    root: PathBuf,
    _lock: File,
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = self._lock.unlock();
    }
}
impl Profile {
    pub(crate) fn open(root: &Path) -> io::Result<Self> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::create_dir_all(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let root = fs::canonicalize(root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("profile.lock"))?;
        if !lock.metadata()?.is_file() {
            return Err(io::Error::other(
                "Profile lock must be a stable regular file",
            ));
        }
        lock.try_lock()
            .map_err(|_| io::Error::other("Profile already owned; refusing writable startup."))?;
        Ok(Self { root, _lock: lock })
    }
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn load(&self) -> io::Result<Metadata> {
        use std::{io::Read, os::unix::fs::OpenOptionsExt};
        let bytes = match File::options()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(self.root.join("workspace.json"))
        {
            Ok(file) => {
                if !file.metadata()?.is_file() {
                    return Err(io::Error::other("Metadata must be a regular file"));
                }
                let mut bytes = Vec::new();
                file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
                bytes
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Metadata {
                    version: 1,
                    ..Default::default()
                });
            }
            Err(e) => return Err(e),
        };
        if bytes.len() > 1024 * 1024 {
            return Err(io::Error::other("Workspace metadata exceeds limit."));
        }
        let state: Metadata = serde_json::from_slice(&bytes)?;
        if state.version != 1 {
            return Err(io::Error::other("Unsupported workspace metadata version."));
        }
        let mut ids = std::collections::HashSet::new();
        for p in &state.projects {
            if !ids.insert(p.id) || !p.directory.is_absolute() {
                return Err(io::Error::other("Invalid project identity."));
            }
        }
        let mut threads = std::collections::HashSet::new();
        for t in &state.threads {
            if !threads.insert(t.id) || !ids.contains(&t.project) || !t.directory.is_absolute() {
                return Err(io::Error::other("Invalid thread association."));
            }
        }
        Ok(state)
    }
    pub(crate) fn save(&self, metadata: &Metadata) -> io::Result<()> {
        let bytes = serde_json::to_vec(metadata)?;
        if bytes.len() > 1024 * 1024 {
            return Err(io::Error::other("Workspace metadata exceeds limit"));
        }
        let path = self.root.join("workspace.json");
        let temporary = self.root.join("workspace.json.next");
        // The profile lock serializes writers. Never unlink/recreate the lock itself.
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&temporary)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("Metadata staging must be a regular file"));
        }
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        File::open(&self.root)?.sync_all()
    }
    pub(crate) fn session_directory(&self, thread: u64) -> io::Result<PathBuf> {
        self.directory(&PathBuf::from("sessions").join(thread.to_string()))
    }
    pub(crate) fn directory(&self, relative: &Path) -> io::Result<PathBuf> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut path = self.root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(io::Error::other("Invalid private directory"));
            };
            path.push(name);
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&path)?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err(io::Error::other(
                            "Private namespace symlink or non-directory refused",
                        ));
                    }
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(path)
    }
    pub(crate) fn owned_session(&self, thread: u64, session: &Path) -> io::Result<PathBuf> {
        let directory = self.session_directory(thread)?;
        if fs::symlink_metadata(session)?.file_type().is_symlink() {
            return Err(io::Error::other("Session symlinks are refused"));
        }
        let path = fs::canonicalize(session)?;
        if path.parent() != Some(directory.as_path()) || !path.is_file() {
            return Err(io::Error::other(
                "Unknown session ownership: writable opening refused. A terminal session requires an explicit safe ownership handoff.",
            ));
        }
        Ok(path)
    }
}
#[cfg(test)]
#[path = "tests/workspace.rs"]
mod tests;
