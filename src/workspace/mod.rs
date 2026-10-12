//! Retained project/thread navigation owner. Blocking file/process mechanics run on one ordered worker.
use crate::{
    connectors::{git, pi::Client},
    storage::workspace::{Metadata, Profile, Project, Thread as SavedThread},
    threads::{Phase, Thread},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Instant,
};

enum Job {
    Save(Metadata),
    CreationBase(u64, PathBuf),
    Create(Metadata, SavedThread, Option<String>),
    Open(Metadata, SavedThread),
    Status(u64, PathBuf),
    Image(u64, u64, PathBuf),
    InspectHistory(u64, PathBuf),
}
enum ResultKind {
    Saved(Result<(), String>),
    CreationBase(u64, Result<String, String>),
    Created(u64, Result<(Metadata, SavedThread), String>),
    Opened(u64, Result<Client, String>),
    Status(u64, Result<git::Status, String>),
    Image(u64, u64, Result<serde_json::Value, String>),
    InspectedHistory(
        u64,
        Result<(PathBuf, crate::storage::history::History), String>,
    ),
}
pub(crate) struct Update(ResultKind);
pub(crate) struct Workspace {
    pub metadata: Metadata,
    pub threads: BTreeMap<u64, Thread>,
    pub selected: Option<u64>,
    pub warning: Option<String>,
    pub external_history: Option<(PathBuf, crate::storage::history::History)>,
    pub storage_failed: bool,
    pending_saves: usize,
    history_request: u64,
    creation_request: u64,
    pub creation_base: Option<(u64, Result<String, String>)>,
    pub worktrees: crate::worktrees::Worktrees,
    pub exit_requested: bool,
    pub confirmed_exit: bool,
    exit_uncertainty_ack: bool,
    pub attention_transitions: u64,
    profile: Arc<Profile>,
    jobs: mpsc::SyncSender<Job>,
    updates: async_channel::Receiver<Update>,
    next_id: u64,
    opening: std::collections::HashSet<u64>,
}
impl Workspace {
    pub(crate) fn new(profile: Profile) -> Result<Self, String> {
        let profile = Arc::new(profile);
        let metadata = profile
            .load()
            .map_err(|_| "Cannot read workspace metadata; refusing to overwrite it.".to_owned())?;
        let threads = metadata.threads.iter().map(|t| {
            let mut owner = Thread::passive(t.id);
            if t.interrupted { owner.error = Some("Previous execution may have been interrupted. Reopening is passive; nothing will be replayed.".into()); }
            if let Some(session) = &t.session {
                match profile.owned_session(t.id, session).and_then(|path| crate::storage::history::read(&path)) {
                    Ok(history) => { if history.cwd!=t.directory{owner.error=Some(format!("Saved Pi working directory is {}. GUI association differs; writable opening is refused.",history.cwd.display()));}owner.text = history.markdown; owner.inspection = history.inspection; owner.omitted = history.omitted; },
                    Err(_) => owner.error = Some("Saved history cannot be safely read; writable opening refused.".into()),
                }
            }
            (t.id, owner)
        }).collect();
        let (jobs, receive) = mpsc::sync_channel(32);
        let (deliver, updates) = async_channel::bounded(32);
        let storage = profile.clone();
        std::thread::spawn(move || {
            let mut leases = Vec::<crate::connectors::pi::Lease>::new();
            while let Ok(job) = receive.recv() {
                let result = match job {
                    Job::CreationBase(request, directory) => ResultKind::CreationBase(
                        request,
                        git::head_commit(&directory).map_err(|_| "No current commit available. Choose Shared directory explicitly, or enter a full commit hash from this repository.".into()),
                    ),
                    Job::Save(metadata) => {
                        ResultKind::Saved(storage.save(&metadata).map_err(|_| {
                            "Workspace save failed; changes may not be durable.".into()
                        }))
                    }
                    Job::Create(mut metadata, mut thread, commit) => {
                        let id = thread.id;
                        let result = (|| {
                            if let Some(commit) = commit {
                                let source = metadata.projects.iter().find(|p| p.id == thread.project).ok_or("Unknown project")?;
                                let target = storage.root().join("worktrees").join(thread.id.to_string());
                                storage.directory(std::path::Path::new("worktrees")).map_err(|_| "Cannot safely create worktree parent")?;
                                thread.directory = crate::worktrees::allocate(&source.directory, &target, thread.id, &commit).map_err(|_| "Worktree creation failed; inspect Git state before retry. No cleanup or dirty copying performed.")?;
                            }
                            if let Some(saved) = metadata.threads.iter_mut().find(|saved| saved.id == thread.id) { *saved = thread.clone(); }
                            storage.save(&metadata).map_err(|_| "Thread metadata save failed; any created worktree is preserved.")?;
                            Ok((metadata, thread))
                        })().map_err(str::to_owned);
                        ResultKind::Created(id, result)
                    }
                    Job::Open(metadata, thread) => {
                        if deliver.is_closed() {
                            continue;
                        }
                        let id = thread.id;
                        leases.retain(|lease|lease.healthy());
                        let result = launch(&storage, &metadata, &thread,&leases);
                        if let Ok(client)=&result && let Some(lease)=client.lease(){leases.push(lease);}
                        ResultKind::Opened(id, result)
                    }
                    Job::InspectHistory(request,path)=>ResultKind::InspectedHistory(request,(||{let path=std::fs::canonicalize(path).map_err(|_|"Cannot resolve saved history")?;let history=crate::storage::history::read(&path).map_err(|_|"Saved history is malformed, unavailable or exceeds the read-only budget")?;Ok((path,history))})().map_err(str::to_owned)),
                    Job::Image(id, attempt, path) => ResultKind::Image(id, attempt, crate::storage::images::read(&path).map_err(|_| "Image input refused: unreadable, unsupported signature or larger than 1 MiB.".into())),
                    Job::Status(id, directory) => ResultKind::Status(
                        id,
                        git::status(&directory).map_err(|_| {
                            "Status unavailable (not Git or operation failed).".into()
                        }),
                    ),
                };
                // Accepted storage writes still drain after the presentation owner drops.
                let _ = deliver.send_blocking(Update(result));
            }
        });
        let next_id = metadata.next_id.max(
            metadata
                .projects
                .iter()
                .map(|p| p.id)
                .chain(metadata.threads.iter().map(|t| t.id))
                .max()
                .unwrap_or(0),
        );
        Ok(Self {
            metadata,
            threads,
            selected: None,
            warning: None,
            external_history: None,
            storage_failed: false,
            pending_saves: 0,
            history_request: 0,
            creation_request: 0,
            creation_base: None,
            worktrees: Default::default(),
            exit_requested: false,
            confirmed_exit: false,
            exit_uncertainty_ack: false,
            attention_transitions: 0,
            profile,
            jobs,
            updates,
            next_id,
            opening: Default::default(),
        })
    }
    fn enqueue(&mut self, job: Job) -> bool {
        if self.jobs.try_send(job).is_err() {
            self.warning =
                Some("Workspace worker unavailable or full; operation not admitted.".into());
            false
        } else {
            true
        }
    }
    fn save(&mut self) {
        if self.enqueue(Job::Save(self.metadata.clone())) {
            self.pending_saves += 1;
        } else {
            self.storage_failed = true;
            self.exit_uncertainty_ack = false;
        }
    }
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.metadata.next_id = self.next_id;
        self.next_id
    }
    pub(crate) fn add_project(&mut self, path: PathBuf) {
        match std::fs::canonicalize(path) {
            // Small path lookup, never executes project resources.
            Ok(directory) if directory.is_dir() => {
                if self
                    .metadata
                    .projects
                    .iter()
                    .any(|p| p.directory == directory)
                {
                    self.warning = Some("Project already exists.".into());
                    return;
                }
                let id = self.id();
                let name = directory
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.metadata.projects.push(Project {
                    id,
                    name,
                    directory,
                });
                self.save();
            }
            _ => self.warning = Some("Choose an existing project directory.".into()),
        }
    }
    pub(crate) fn prepare_creation(&mut self, project: u64) -> u64 {
        self.creation_request += 1;
        self.creation_base = None;
        if let Some(project) = self.metadata.projects.iter().find(|p| p.id == project)
            && !self.enqueue(Job::CreationBase(
                self.creation_request,
                project.directory.clone(),
            ))
        {
            self.creation_base = Some((
                self.creation_request,
                Err("Cannot load current commit.".into()),
            ));
        }
        self.creation_request
    }
    pub(crate) fn create(&mut self, project: u64, name: String, commit: Option<String>) {
        if self.exit_requested
            || name.trim().is_empty()
            || commit
                .as_ref()
                .is_some_and(|base| !git::is_full_object_id(base))
        {
            self.warning = Some("Enter a thread name and a full commit hash, or explicitly choose Shared directory.".into());
            return;
        }
        let Some(directory) = self
            .metadata
            .projects
            .iter()
            .find(|p| p.id == project)
            .map(|p| p.directory.clone())
        else {
            return;
        };
        let id = self.id();
        let thread = SavedThread {
            id,
            project,
            name,
            directory: if commit.is_some() {
                self.profile.root().join("worktrees").join(id.to_string())
            } else {
                directory
            },
            session: None,
            archived: false,
            interrupted: false,
            isolated: commit.is_some(),
            trusted: false,
        };
        self.metadata.threads.push(thread.clone());
        self.threads.insert(id, Thread::passive(id));
        self.opening.insert(id); // Reservation cannot execute until creation/persistence succeeds.
        if !self.enqueue(Job::Create(self.metadata.clone(), thread, commit)) {
            self.opening.remove(&id);
            self.threads.remove(&id);
            self.metadata.threads.retain(|t| t.id != id);
        } else {
            self.select(id);
        }
    }
    pub(crate) fn select(&mut self, id: u64) {
        if self.threads.contains_key(&id) {
            self.selected = Some(id);
            self.history_request += 1;
            self.external_history = None;
        }
    }
    pub(crate) fn open(&mut self, id: u64) {
        if self.exit_requested
            || self.threads.values().any(|t| t.phase == Phase::Uncertain)
            || self.threads.values().filter(|t| t.live()).count() + self.opening.len() >= 8
        {
            self.warning =
                Some("Admission stopped or eight top-level threads already open.".into());
            return;
        }
        if self.threads.get(&id).is_none_or(|t| t.live()) || self.opening.contains(&id) {
            return;
        }
        let Some(thread) = self
            .metadata
            .threads
            .iter_mut()
            .find(|t| t.id == id && !t.archived)
        else {
            return;
        };
        thread.interrupted = true;
        let thread = thread.clone();
        if self.enqueue(Job::Open(self.metadata.clone(), thread)) {
            self.opening.insert(id);
        }
    }
    pub(crate) fn rename(&mut self, id: u64, name: String) {
        if name.trim().is_empty() {
            return;
        }
        if let Some(thread) = self.metadata.threads.iter_mut().find(|t| t.id == id) {
            thread.name = name.clone();
        }
        if let Some(owner) = self.threads.get_mut(&id)
            && owner.live()
        {
            owner.rename_session(name);
        }
        self.save();
    }
    pub(crate) fn archive(&mut self, id: u64) {
        if self.threads.get(&id).is_some_and(|t| t.live()) || self.opening.contains(&id) {
            self.warning = Some(
                "Close the thread before archiving; archive never deletes history/worktrees."
                    .into(),
            );
            return;
        }
        if let Some(thread) = self.metadata.threads.iter_mut().find(|t| t.id == id) {
            thread.archived = true;
        }
        self.save();
    }
    pub(crate) fn toggle_trust(&mut self, id: u64) {
        if self.threads.get(&id).is_some_and(|t| t.live()) || self.opening.contains(&id) {
            self.warning = Some(
                "Close runtime before changing resource trust; no mid-startup overlays.".into(),
            );
            return;
        }
        if let Some(saved) = self.metadata.threads.iter_mut().find(|t| t.id == id) {
            saved.trusted = !saved.trusted;
        }
        self.save();
    }
    pub(crate) fn status(&mut self, id: u64) {
        if let Some(t) = self.metadata.threads.iter().find(|t| t.id == id) {
            self.enqueue(Job::Status(id, t.directory.clone()));
        }
    }
    pub(crate) fn attach_image(&mut self, id: u64, path: PathBuf) {
        if let Some(t) = self.threads.get_mut(&id) {
            if !t.images.is_empty() || t.image_pending {
                t.error = Some(
                    "Remove the existing attachment/read before selecting another image.".into(),
                );
                return;
            }
            t.image_revision += 1;
            t.image_pending = true;
            let revision = t.image_revision;
            if !self.enqueue(Job::Image(id, revision, path))
                && let Some(t) = self.threads.get_mut(&id)
            {
                t.image_pending = false;
            }
        }
    }
    pub(crate) fn open_saved(&mut self, path: PathBuf) {
        let id = self
            .metadata
            .threads
            .iter()
            .find(|t| t.session.as_ref() == Some(&path))
            .map(|t| t.id);
        if let Some(id) = id {
            self.select(id);
        } else {
            self.history_request += 1;
            self.enqueue(Job::InspectHistory(self.history_request, path));
        }
    }
    pub(crate) fn mute(&mut self) {
        self.metadata.muted = !self.metadata.muted;
        self.save();
    }
    pub(crate) fn request_exit(&mut self) {
        self.exit_requested = true;
    }
    pub(crate) fn confirm_exit(&mut self) {
        if self.confirmed_exit
            && self.opening.is_empty()
            && self.threads.values().all(|t| !t.live())
        {
            self.exit_uncertainty_ack = true;
        }
        self.confirmed_exit = true;
        for thread in self.threads.values_mut() {
            if thread.live() {
                thread.close();
            }
        }
    }
    pub(crate) fn storage_pending(&self) -> bool {
        self.pending_saves > 0
    }
    pub(crate) fn ready_to_exit(&self) -> bool {
        self.confirmed_exit
            && self.pending_saves == 0
            && (!self.storage_failed || self.exit_uncertainty_ack)
            && self.opening.is_empty()
            && self.threads.values().all(|t| !t.live())
            && (self.exit_uncertainty_ack
                || self.threads.values().all(|t| t.phase != Phase::Uncertain))
    }
    pub(crate) fn poll(&mut self, now: Instant) -> bool {
        let mut changed = false;
        while let Ok(Update(update)) = self.updates.try_recv() {
            changed = true;
            match update {
                ResultKind::CreationBase(request, result) => {
                    if request == self.creation_request {
                        self.creation_base = Some((request, result));
                    }
                }
                ResultKind::Saved(result) => {
                    self.pending_saves = self.pending_saves.saturating_sub(1);
                    self.storage_failed = result.is_err();
                    if let Err(error) = result {
                        self.exit_uncertainty_ack = false;
                        self.warning = Some(error);
                    }
                }
                ResultKind::InspectedHistory(request, result) => {
                    if request == self.history_request {
                        match result {
                            Ok(history) => {
                                if self.warning.as_deref().is_some_and(|warning|matches!(warning,"Cannot resolve saved history"|"Saved history is malformed, unavailable or exceeds the read-only budget")){self.warning=None;}
                                self.external_history = Some(history);
                                self.selected = None;
                            }
                            Err(error) => self.warning = Some(error),
                        }
                    }
                }
                ResultKind::Created(id, result) => match result {
                    Ok((_metadata, t)) => {
                        self.opening.remove(&t.id);
                        // Never replace newer navigation/name/mute/session state with a stale worker snapshot.
                        self.save();
                    }
                    Err(error) => {
                        self.opening.remove(&id);
                        self.warning = Some(error);
                        self.metadata.threads.retain(|t| t.id != id);
                        self.threads.remove(&id);
                        if self.selected == Some(id) {
                            self.selected = None;
                        }
                        self.save();
                    }
                },
                ResultKind::Opened(id, result) => {
                    self.opening.remove(&id);
                    match result {
                        Ok(client) => {
                            if let Some(t) = self.threads.get_mut(&id) {
                                t.attach(client);
                                if self.confirmed_exit {
                                    t.close();
                                }
                            }
                        }
                        Err(error) => self.warning = Some(error),
                    }
                }
                ResultKind::Image(id, attempt, result) => {
                    if let Some(t) = self.threads.get_mut(&id)
                        && t.image_revision == attempt
                    {
                        t.image_pending = false;
                        match result { Ok(image) if t.images.is_empty() => t.images.push(image), Ok(_) => t.error = Some("One bounded image per prompt is supported; remove the attachment first.".into()), Err(e) => t.error = Some(e) }
                    }
                }
                ResultKind::Status(id, result) => {
                    self.worktrees.apply_status(id, result);
                }
            }
        }
        let mut persist = false;
        for (&id, thread) in &mut self.threads {
            let before = thread.phase;
            if thread.poll(now) {
                changed = true;
            }
            if before != Phase::Uncertain && thread.phase == Phase::Uncertain {
                self.exit_uncertainty_ack = false;
            }
            if before != Phase::Attention && thread.phase == Phase::Attention {
                self.attention_transitions += 1;
            }
            if let Some(saved) = self.metadata.threads.iter_mut().find(|t| t.id == id) {
                if let Some(path) = thread.state["sessionFile"].as_str() {
                    let path = PathBuf::from(path);
                    if saved.session.as_ref() != Some(&path)
                        && self.profile.owned_session(id, &path).is_ok()
                    {
                        saved.session = Some(path);
                        persist = true;
                    }
                }
                let interrupted = thread.interrupted
                    || thread.live()
                    || thread.active()
                    || thread.phase == Phase::Uncertain;
                if saved.interrupted != interrupted
                    && (thread.phase != Phase::Passive || before == Phase::Closing)
                {
                    saved.interrupted = interrupted;
                    persist = true;
                }
            }
        }
        if persist {
            self.save();
        }
        changed
    }
}
fn launch(
    profile: &Profile,
    metadata: &Metadata,
    thread: &SavedThread,
    leases: &[crate::connectors::pi::Lease],
) -> Result<Client, String> {
    let pi_root = std::env::var_os("PI_GUI_PI_PACKAGE")
        .map(PathBuf::from)
        .or_else(|| {
            let output = std::process::Command::new("which")
                .arg("pi")
                .output()
                .ok()?;
            let path = std::fs::canonicalize(String::from_utf8(output.stdout).ok()?.trim()).ok()?;
            path.parent()?
                .parent()?
                .parent()
                .map(std::path::Path::to_path_buf)
        })
        .ok_or("Pinned Pi CLI not found; set PI_GUI_PI_PACKAGE")?;
    let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".pi/agent")))
        .ok_or("Pi agent configuration directory unavailable")?;
    launch_at(profile, metadata, thread, leases, pi_root, agent_dir)
}
fn launch_at(
    profile: &Profile,
    metadata: &Metadata,
    thread: &SavedThread,
    leases: &[crate::connectors::pi::Lease],
    pi_root: PathBuf,
    agent_dir: PathBuf,
) -> Result<Client, String> {
    let pi_root = std::fs::canonicalize(pi_root).map_err(|_| "Invalid Pi package path")?;
    let agent_dir = std::fs::canonicalize(agent_dir)
        .map_err(|_| "Pi agent configuration directory unavailable")?;
    let session_dir = profile
        .session_directory(thread.id)
        .map_err(|_| "Cannot allocate private session storage")?;
    if let Some(session) = &thread.session {
        profile
            .owned_session(thread.id, session)
            .map_err(|_| "Unknown saved-session ownership; writable open refused")?;
        let history = crate::storage::history::read(session)
            .map_err(|_| "Saved history cannot be safely inspected")?;
        let actual = std::fs::canonicalize(history.cwd)
            .map_err(|_| "Saved session working directory unavailable")?;
        if actual != thread.directory {
            return Err(
                "Saved session cwd differs from GUI association; refusing silent rebinding.".into(),
            );
        }
    }
    let root = profile
        .directory(&PathBuf::from("owners").join(thread.id.to_string()))
        .map_err(|_| "Cannot safely allocate retained owner storage")?;
    let frontend = crate::storage::owners::identity(std::process::id())
        .map_err(|_| "Cannot establish frontend process identity")?;
    crate::storage::owners::admission(&profile.root().join("owners"),&frontend,&|scope|leases.iter().any(|lease|lease.permits(scope))).map_err(|_|"Retained execution ownership unavailable or unproved. Refusing all new admission; reopen history passively and reconcile private scope.")?;
    profile
        .save(metadata)
        .map_err(|_| "Cannot persist interrupted marker before dispatch")?;
    let scope = root.join(format!(
        "attempt-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Invalid clock")?
            .as_nanos()
    ));
    let request = scope.join("launch.json");
    let prepared = Client::prepare_launch(&request).map_err(
        |_| "Cannot prepare retained Pi owner (unsupported runtime hooks); no process started",
    )?;
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&scope)
        .map_err(|_| "Cannot create unique process attempt")?;
    if std::fs::write(&request, json!({"piRoot":pi_root, "cwd":thread.directory, "agentDir":agent_dir, "sessionDir":session_dir, "session":thread.session, "scope":scope,"projectTrusted":thread.trusted,"frontend":frontend}).to_string()).is_err() {
        // No process has started. Remove only our unused reservation, never a dispatched attempt.
        let _ = std::fs::remove_file(&request);
        let _ = std::fs::remove_dir(&scope);
        return Err("Cannot persist launch request; no process started".into());
    }
    let client = prepared.start().map_err(|_| "Cannot launch retained Pi owner; inspect retained attempt (partial process setup may be uncertain)")?;
    crate::storage::owners::record_launcher(&scope, client.process_id(), &frontend)
        .map_err(|_| "Cannot persist launcher ownership; launch interrupted/uncertain")?;
    Ok(client)
}
#[cfg(test)]
#[path = "tests/navigation.rs"]
mod tests;
