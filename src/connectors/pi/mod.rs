//! Bounded continuously-drained ordinary Pi RPC transport. No automatic mutation retries.
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const MAX_FRAME: usize = 2 * 1024 * 1024;
const MAX_RETAINED: usize = 8 * 1024 * 1024;
const MAX_RECORDS: usize = 2048;
#[derive(Debug)]
pub(crate) enum Update {
    Record(Value),
    Failed(&'static str),
    Closed,
}
struct Mailbox {
    records: VecDeque<(Value, usize)>,
    bytes: usize,
    terminal: Option<Update>,
    sealed: bool,
}
impl Mailbox {
    fn push(&mut self, value: Value, bytes: usize) {
        if self.sealed {
            return;
        }
        if self.records.len() >= MAX_RECORDS || self.bytes + bytes > MAX_RETAINED {
            self.sealed = true;
            self.terminal = Some(Update::Failed(
                "RPC presentation overflow; execution outcome is uncertain.",
            ));
            self.records.clear();
            self.bytes = 0;
        } else {
            self.records.push_back((value, bytes));
            self.bytes += bytes;
        }
    }
    fn fail(&mut self, reason: &'static str) {
        if !self.sealed {
            self.terminal = Some(Update::Failed(reason));
        }
        self.sealed = true;
    }
}
enum Intention {
    Send(Value),
    Close,
}
#[derive(Clone)]
pub(crate) struct Lease {
    scope: PathBuf,
    mailbox: Arc<Mutex<Mailbox>>,
    closing: Arc<std::sync::atomic::AtomicBool>,
}
impl Lease {
    pub(crate) fn healthy(&self) -> bool {
        !self.closing.load(std::sync::atomic::Ordering::Acquire)
            && self
                .mailbox
                .lock()
                .map(|mailbox| !mailbox.sealed)
                .unwrap_or(false)
    }
    pub(crate) fn permits(&self, scope: &Path) -> bool {
        self.scope == scope && self.healthy()
    }
}
pub(crate) struct Client {
    commands: mpsc::SyncSender<Intention>,
    mailbox: Arc<Mutex<Mailbox>>,
    next: u64,
    process_id: u32,
    scope: Option<PathBuf>,
    closing: Arc<std::sync::atomic::AtomicBool>,
}
pub(crate) struct PreparedLaunch {
    command: Command,
    scope: Option<PathBuf>,
}
impl PreparedLaunch {
    pub(crate) fn start(self) -> io::Result<Client> {
        let mut client = Client::spawn(self.command)?;
        client.scope = self.scope;
        Ok(client)
    }
}
impl Client {
    // Pure preparation: all deterministic validation happens before an attempt is allocated.
    pub(crate) fn prepare_launch(request: &Path) -> io::Result<PreparedLaunch> {
        if ["NODE_OPTIONS", "NODE_PATH"]
            .iter()
            .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
        {
            return Err(io::Error::other(
                "Unsupported Node runtime hooks; refusing unaudited execution before resource startup.",
            ));
        }
        let command = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/pi-runtime/launch.mjs");
        let mut launch = Command::new(std::env::current_exe()?);
        launch
            .arg("--rpgui-retained-node")
            .arg(command)
            .arg(request);
        Ok(PreparedLaunch {
            command: launch,
            scope: request.parent().map(Path::to_owned),
        })
    }
    pub(crate) fn lease(&self) -> Option<Lease> {
        self.scope.as_ref().map(|scope| Lease {
            scope: scope.clone(),
            mailbox: self.mailbox.clone(),
            closing: self.closing.clone(),
        })
    }
    fn spawn(mut launch: Command) -> io::Result<Self> {
        let mut process = launch
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let process_id = process.id();
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Pi stdout missing"))?;
        let stderr = process
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Pi stderr missing"))?;
        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Pi stdin missing"))?;
        use std::os::fd::AsRawFd;
        // SAFETY: stdin's owned descriptor is valid here; fcntl does not retain or close it.
        unsafe {
            let flags = libc::fcntl(stdin.as_raw_fd(), libc::F_GETFL);
            if flags < 0
                || libc::fcntl(stdin.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) < 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        let mailbox = Arc::new(Mutex::new(Mailbox {
            records: VecDeque::new(),
            bytes: 0,
            terminal: None,
            sealed: false,
        }));
        let (wire_send, wire_receive) = mpsc::sync_channel(256);
        let failure = mailbox.clone();
        let (eof_send, eof_receive) = mpsc::channel();
        thread::spawn(move || {
            drain(stdout, wire_send, failure);
            let _ = eof_send.send(());
        });
        thread::spawn(move || {
            let _ = io::copy(&mut stderr.take(u64::MAX), &mut io::sink());
        });
        let (commands, intentions) = mpsc::sync_channel(64);
        let output = mailbox.clone();
        let close_gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let close_signal = close_gate.clone();
        thread::spawn(move || {
            let mut stdin = Some(stdin);
            let mut pending: HashMap<String, (String, Instant)> = HashMap::new();
            let mut closing = None;
            let mut outgoing: VecDeque<(Vec<u8>, usize)> = VecDeque::new();
            let mut exited = None;
            loop {
                while let Ok(record) = wire_receive.try_recv() {
                    let kind = record["type"].as_str().unwrap_or("");
                    if kind == "response" {
                        let id = record["id"].as_str().unwrap_or("");
                        // Guardian command replies are internal and not GUI mutations.
                        if !id.starts_with("rpgui-owner-") {
                            match pending.remove(id) {
                                Some((command, _))
                                    if record["command"].as_str() == Some(command.as_str()) => {}
                                _ => {
                                    output
                                        .lock()
                                        .unwrap()
                                        .fail("Uncorrelated RPC response; admission stopped.");
                                    closing.get_or_insert(Instant::now());
                                    stdin.take();
                                }
                            }
                        }
                    }
                    let bytes = record.to_string().len();
                    output.lock().unwrap().push(record, bytes);
                }
                if pending
                    .values()
                    .any(|(_, deadline)| Instant::now() >= *deadline)
                {
                    output.lock().unwrap().fail("RPC acknowledgement deadline; dispatch may have executed. No automatic retry.");
                    closing.get_or_insert(Instant::now());
                    stdin.take();
                    pending.clear();
                }
                if output.lock().unwrap().sealed
                    || close_signal.load(std::sync::atomic::Ordering::Acquire)
                {
                    closing.get_or_insert(Instant::now());
                    stdin.take();
                }
                match intentions.recv_timeout(Duration::from_millis(10)) {
                    Ok(Intention::Send(record)) if closing.is_none() => {
                        let id = record["id"].as_str().unwrap_or("").to_owned();
                        let kind = record["type"].as_str().unwrap_or("").to_owned();
                        if kind != "extension_ui_response" {
                            pending.insert(id, (kind, Instant::now() + Duration::from_secs(30)));
                        }
                        if outgoing.len() >= 64 {
                            output
                                .lock()
                                .unwrap()
                                .fail("RPC write backlog exceeded; dispatch uncertain.");
                        } else {
                            outgoing.push_back((format!("{record}\n").into_bytes(), 0));
                        }
                    }
                    Ok(Intention::Close) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        closing.get_or_insert(Instant::now());
                        stdin.take();
                    }
                    _ => {}
                }
                if let Some(input) = stdin.as_mut()
                    && let Some((bytes, offset)) = outgoing.front_mut()
                {
                    match input.write(&bytes[*offset..]) {
                        Ok(0) => output
                            .lock()
                            .unwrap()
                            .fail("RPC pipe closed during dispatch; outcome uncertain."),
                        Ok(count) => {
                            *offset += count;
                            if *offset == bytes.len() {
                                outgoing.pop_front();
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                        Err(_) => output
                            .lock()
                            .unwrap()
                            .fail("RPC write failed after possible dispatch; outcome uncertain."),
                    }
                }
                if exited.is_none() {
                    match process.try_wait() {
                        Ok(Some(status)) => exited = Some(status),
                        Err(_) => {
                            output
                                .lock()
                                .unwrap()
                                .fail("Pi owner unavailable; execution ownership uncertain.");
                            break;
                        }
                        _ => {}
                    }
                }
                if let Some(status) = exited
                    && eof_receive.try_recv().is_ok()
                {
                    // One more loop drains records decoded just before EOF before emitting the terminal result.
                    while let Ok(record) = wire_receive.try_recv() {
                        if record["type"] == "response"
                            && let Some(id) = record["id"].as_str()
                        {
                            pending.remove(id);
                        }
                        let bytes = record.to_string().len();
                        output.lock().unwrap().push(record, bytes);
                    }
                    let mut out = output.lock().unwrap();
                    if !out.sealed {
                        out.terminal = Some(if status.success() && pending.is_empty() {
                            Update::Closed
                        } else {
                            Update::Failed(
                                "Pi owner exited; interrupted or ambiguously dispatched work is uncertain.",
                            )
                        });
                        out.sealed = true;
                    }
                    break;
                }
                // Never kill the guardian: it must retain nested-work ownership after frontend loss.
                if closing.is_some_and(|time| time.elapsed() > Duration::from_secs(35)) {
                    output
                        .lock()
                        .unwrap()
                        .fail("Shutdown proof deadline; retained owner outcome uncertain.");
                    break;
                }
            }
        });
        Ok(Self {
            commands,
            mailbox,
            next: 0,
            process_id,
            scope: None,
            closing: close_gate,
        })
    }
    pub(crate) fn send(&mut self, mut record: Value) -> Result<String, &'static str> {
        if self.mailbox.lock().unwrap().sealed
            || self.closing.load(std::sync::atomic::Ordering::Acquire)
        {
            return Err("Execution owner is unavailable; admission refused.");
        }
        self.next += 1;
        let id = format!("gui-{}", self.next);
        if record["type"] != "extension_ui_response" {
            record["id"] = json!(id);
        }
        if record.to_string().len() > MAX_FRAME {
            return Err("Command exceeds transport limit; not dispatched.");
        }
        self.commands
            .try_send(Intention::Send(record))
            .map_err(|_| "RPC input unavailable or full; command not dispatched.")?;
        Ok(id)
    }
    pub(crate) fn drain(&self) -> Vec<Update> {
        let mut mailbox = self.mailbox.lock().unwrap();
        let mut result: Vec<_> = mailbox
            .records
            .drain(..)
            .map(|(record, _)| Update::Record(record))
            .collect();
        mailbox.bytes = 0;
        if let Some(terminal) = mailbox.terminal.take() {
            result.push(terminal);
        }
        result
    }
    pub(crate) fn process_id(&self) -> u32 {
        self.process_id
    }
    pub(crate) fn close(&self) {
        // A close intention bypasses a full command queue via the sticky admission gate.
        self.closing
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = self.commands.try_send(Intention::Close);
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.close();
    }
}
fn drain(mut reader: impl Read, send: mpsc::SyncSender<Value>, mailbox: Arc<Mutex<Mailbox>>) {
    let mut frame = Vec::new();
    let mut bytes = [0; 8192];
    let mut invalid = false;
    loop {
        let count = match reader.read(&mut bytes) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => {
                mailbox
                    .lock()
                    .unwrap()
                    .fail("RPC read failure; outcome uncertain.");
                break;
            }
        };
        for byte in &bytes[..count] {
            if invalid {
                continue;
            }
            if *byte == b'\n' {
                if frame.last() == Some(&b'\r') {
                    frame.pop();
                }
                match serde_json::from_slice::<Value>(&frame) {
                    Ok(value) if value.is_object() && value["type"].is_string() => {
                        if send.try_send(value).is_err() {
                            mailbox
                                .lock()
                                .unwrap()
                                .fail("RPC consumer overflow; outcome uncertain.");
                            invalid = true;
                        }
                    }
                    _ => {
                        mailbox
                            .lock()
                            .unwrap()
                            .fail("Malformed Pi JSONL protocol; admission stopped.");
                        invalid = true;
                    }
                }
                frame.clear();
            } else if frame.len() < MAX_FRAME {
                frame.push(*byte);
            } else {
                mailbox
                    .lock()
                    .unwrap()
                    .fail("Pi frame exceeds transport limit; admission stopped.");
                frame.clear();
                invalid = true;
            }
        }
    }
    if !frame.is_empty() && !invalid {
        mailbox
            .lock()
            .unwrap()
            .fail("Incomplete Pi protocol record; outcome uncertain.");
    }
}
#[cfg(test)]
#[path = "tests/process.rs"]
mod process_tests;
#[cfg(test)]
#[path = "tests/transport.rs"]
mod tests;
