//! One retained feature owner per ordinary Pi thread; no view owns transport or run state.
use crate::connectors::pi::{Client, Update};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Passive,
    Starting,
    Idle,
    Running,
    Compacting,
    Retrying,
    Attention,
    Failed,
    Uncertain,
    Closing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Steer,
    FollowUp,
}
pub(crate) struct Dialog {
    pub record: Value,
    deadline: Option<Instant>,
}
pub(crate) struct Thread {
    pub id: u64,
    pub attempt: u64,
    pub phase: Phase,
    pub text: String,
    pub inspection: String,
    pub omitted: bool,
    pub dialog: Option<Dialog>,
    pub notifications: Vec<String>,
    pub state: Value,
    pub statistics: Value,
    pub models: Vec<Value>,
    pub levels: Vec<String>,
    pub commands: Vec<Value>,
    pub steering: Vec<String>,
    pub follow_up: Vec<String>,
    pub error: Option<String>,
    pub interrupted: bool,
    pub draft: String,
    pub images: Vec<Value>,
    pub image_revision: u64,
    pub image_pending: bool,
    pub children: Value,
    pub child_observed: Option<u64>,
    owner_seen: Option<Instant>,
    started_at: Option<Instant>,
    subagents: bool,
    child_ready: bool,
    commands_ready: bool,
    admission_refused: bool,
    safe_commands: Vec<Value>,
    subagent_sequence: u64,
    child_applied: u64,
    next_inspection: Option<Instant>,
    pending_prompt: Option<(String, String)>,
    pending_attempt: u64,
    pending_image_revision: u64,
    client: Option<Client>,
}
impl Thread {
    pub(crate) fn passive(id: u64) -> Self {
        Self {
            id,
            attempt: 0,
            phase: Phase::Passive,
            text: String::new(),
            inspection: String::new(),
            omitted: false,
            dialog: None,
            notifications: Vec::new(),
            state: Value::Null,
            statistics: Value::Null,
            models: Vec::new(),
            levels: Vec::new(),
            commands: Vec::new(),
            steering: Vec::new(),
            follow_up: Vec::new(),
            error: None,
            interrupted: false,
            draft: String::new(),
            images: Vec::new(),
            image_revision: 0,
            image_pending: false,
            children: Value::Null,
            child_observed: None,
            owner_seen: None,
            started_at: None,
            subagents: false,
            child_ready: false,
            commands_ready: false,
            admission_refused: false,
            safe_commands: Vec::new(),
            subagent_sequence: 0,
            child_applied: 0,
            next_inspection: None,
            pending_prompt: None,
            pending_attempt: 0,
            pending_image_revision: 0,
            client: None,
        }
    }
    pub(crate) fn attach(&mut self, client: Client) {
        self.attempt += 1;
        self.owner_seen = None;
        self.started_at = Some(Instant::now());
        self.next_inspection = None;
        self.subagents = false;
        self.child_ready = false;
        self.commands_ready = false;
        self.admission_refused = false;
        self.safe_commands.clear();
        self.child_applied = 0;
        self.phase = Phase::Starting;
        self.error = None;
        self.client = Some(client);
    }
    pub(crate) fn live(&self) -> bool {
        self.client.is_some()
    }
    pub(crate) fn active(&self) -> bool {
        matches!(
            self.phase,
            Phase::Starting
                | Phase::Running
                | Phase::Compacting
                | Phase::Retrying
                | Phase::Attention
                | Phase::Closing
        )
    }
    fn dispatch(&mut self, command: Value) -> Option<String> {
        if let Some(client) = &mut self.client {
            match client.send(command) {
                Ok(id) => Some(id),
                Err(error) => {
                    self.error = Some(error.into());
                    None
                }
            }
        } else {
            self.error = Some("Open this owned thread explicitly before sending commands.".into());
            None
        }
    }
    fn command(&mut self, command: Value) {
        if self.client.is_some() {
            let _ = self.dispatch(command);
        }
    }
    pub(crate) fn rename_session(&mut self, name: String) {
        self.command(json!({"type":"set_session_name","name":name}));
    }
    pub(crate) fn select_model(&mut self, provider: String, model: String) {
        if !matches!(self.phase, Phase::Idle | Phase::Failed) || !self.live() {
            self.error = Some("Model changes require an idle owned thread.".into());
            return;
        }
        self.command(json!({"type":"set_model","provider":provider,"modelId":model}));
    }
    pub(crate) fn select_thinking(&mut self, level: String) {
        if !matches!(self.phase, Phase::Idle | Phase::Failed) || !self.live() {
            self.error = Some("Thinking changes require an idle owned thread.".into());
            return;
        }
        self.command(json!({"type":"set_thinking_level","level":level}));
    }
    pub(crate) fn compact(&mut self) {
        if !matches!(self.phase, Phase::Idle | Phase::Failed) || !self.live() {
            self.error = Some("Stop generation before manual compaction.".into());
            return;
        }
        if self.dispatch(json!({"type":"compact"})).is_some() {
            self.phase = Phase::Compacting;
        }
    }
    pub(crate) fn remove_image(&mut self) {
        self.image_revision += 1;
        self.image_pending = false;
        self.images.clear();
    }
    pub(crate) fn unknown_admission(&self) -> bool {
        self.pending_prompt.is_some()
            && (self.phase == Phase::Uncertain || self.pending_attempt != self.attempt)
    }
    pub(crate) fn acknowledge_unknown_admission(&mut self) {
        if self.unknown_admission() {
            self.pending_prompt = None;
            self.error=Some("Prior prompt outcome remains unknown. Nothing replayed; any new send is a new explicit intention.".into());
        }
    }
    pub(crate) fn prompt(&mut self, text: String, delivery: Delivery, images: Vec<Value>) {
        if self.image_pending {
            self.error =
                Some("Wait for the requested image read, or remove it, before sending.".into());
            return;
        }

        if text.len() > 1024 * 1024 {
            self.error =
                Some("Text exceeds the 1 MiB admission budget; nothing dispatched.".into());
            return;
        }
        if self.pending_prompt.is_some() {
            self.error = Some(
                "Previous prompt acknowledgement pending; do not duplicate ambiguous dispatch."
                    .into(),
            );
            return;
        }
        if !matches!(
            self.phase,
            Phase::Idle
                | Phase::Running
                | Phase::Attention
                | Phase::Compacting
                | Phase::Retrying
                | Phase::Failed
        ) {
            self.error = Some("Thread is not ready for admission.".into());
            return;
        }
        if text.starts_with('/') {
            let name = text
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_start_matches('/');
            if !self.commands.iter().any(|command| command["name"] == name) {
                self.error=Some("Unsupported/undiscovered slash workflow. Native controls replace TUI-only commands; nothing dispatched.".into());
                return;
            }
        }
        if let Some(command_text) = text.strip_prefix('/') {
            if !images.is_empty() {
                self.error = Some(
                    "Images cannot accompany slash commands; remove the staged image first.".into(),
                );
                return;
            }
            let name = command_text.split_whitespace().next().unwrap_or("");
            let command = self.commands.iter().find(|command| command["name"] == name);
            let admitted = command.is_some_and(|command| {
                command["source"] != "extension"
                    || self.safe_commands.iter().any(|policy| {
                        let path = command["sourceInfo"]["path"].as_str().unwrap_or("");
                        let root = policy["root"].as_str().unwrap_or("");
                        (policy["name"] == name || policy["name"] == "*")
                            && !root.is_empty()
                            && (path == root
                                || (!root.starts_with("builtin:")
                                    && path.starts_with(&format!("{root}/"))))
                    })
            });
            if !admitted {
                self.error=Some("Unknown or unaudited slash workflow refused before RPC dispatch. Use discovered supported commands or ordinary guarded tools.".into());
                return;
            }
        }
        let command = json!({"type":"prompt","message":text,"images":images,"streamingBehavior":match delivery{Delivery::Steer=>"steer",Delivery::FollowUp=>"followUp"}});
        if let Some(id) = self.dispatch(command) {
            self.pending_attempt = self.attempt;
            self.pending_image_revision = self.image_revision;
            self.pending_prompt = Some((id, text));
            self.error = None;
        }
    }
    pub(crate) fn stop(&mut self) {
        // Serial wire order: queue removal is dispatched before abort. Neither implies detached-owner cleanup.
        self.command(json!({"type":"clear_queue"}));
        self.command(json!({"type":"abort"}));
        self.command(json!({"type":"abort_bash"}));
        self.dialog = None;
    }
    pub(crate) fn close(&mut self) {
        if self.client.is_none() {
            return;
        }
        self.interrupted = self.interrupted
            || self.active()
            || self.children["fleet"]["totalActive"]
                .as_u64()
                .is_some_and(|count| count > 0);
        self.phase = Phase::Closing;
        self.dialog = None;
        if let Some(client) = &self.client {
            client.close();
        }
    }
    pub(crate) fn answer(&mut self, attempt: u64, id: &str, mut answer: Value, now: Instant) {
        if attempt != self.attempt
            || self.dialog.as_ref().is_none_or(|dialog| {
                dialog.record["id"] != id || dialog.deadline.is_some_and(|deadline| now >= deadline)
            })
        {
            return;
        }
        answer["type"] = json!("extension_ui_response");
        answer["id"] = json!(id);
        self.command(answer);
        self.dialog = None;
        self.phase = Phase::Running;
    }
    pub(crate) fn poll(&mut self, now: Instant) -> bool {
        let updates = self.client.as_ref().map(Client::drain).unwrap_or_default();
        let changed = !updates.is_empty();
        for update in updates {
            self.apply(self.attempt, update, now);
        }
        if self.client.is_some()
            && self
                .owner_seen
                .is_some_and(|last| now.saturating_duration_since(last) > Duration::from_secs(5))
            && self.phase != Phase::Closing
        {
            self.apply(self.attempt, Update::Failed("Retained execution owner heartbeat lost; admission refused and outcome uncertain."), now);
            return true;
        }
        if self.subagents
            && !self.child_ready
            && self.phase == Phase::Starting
            && self
                .started_at
                .is_some_and(|start| now.saturating_duration_since(start) > Duration::from_secs(15))
        {
            self.apply(
                self.attempt,
                Update::Failed("Subagent ownership capability deadline; admission refused."),
                now,
            );
            return true;
        }
        if self.subagents
            && self.child_ready
            && self.client.is_some()
            && self.phase != Phase::Closing
            && self.next_inspection.is_none_or(|deadline| now >= deadline)
        {
            self.inspect_children(None);
            self.next_inspection = Some(now + Duration::from_secs(2));
        }
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.deadline.is_some_and(|deadline| now >= deadline))
        {
            self.dialog = None;
            if self.phase == Phase::Attention {
                self.phase = Phase::Running;
            }
            return true;
        }
        changed
    }
    pub(crate) fn apply(&mut self, attempt: u64, update: Update, now: Instant) {
        if attempt != self.attempt {
            return;
        }
        let record = match update {
            Update::Failed(error) => {
                if !self.admission_refused {
                    self.error = Some(match self.error.take() {
                        Some(previous) => format!("{error}\nPrevious error: {previous}"),
                        None => error.into(),
                    });
                    self.interrupted = true;
                    self.phase = Phase::Uncertain;
                }
                self.dialog = None;
                self.client.take();
                return;
            }
            Update::Closed => {
                if !matches!(self.phase, Phase::Uncertain | Phase::Failed) {
                    self.phase = Phase::Passive;
                }
                self.dialog = None;
                self.client.take();
                return;
            }
            Update::Record(record) => record,
        };
        match record["type"].as_str().unwrap_or("") {
            "rpgui_owner_alive" => self.owner_seen = Some(now),
            "rpgui_owner_ready" => {
                self.owner_seen = Some(now);
                self.subagents = record["subagents"] == true;
                self.phase = Phase::Starting;
                self.child_ready = !self.subagents;
                self.safe_commands = record["safeCommands"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if self.subagents {
                    self.child_request("ping", None);
                }
                for kind in [
                    "get_state",
                    "get_messages",
                    "get_available_models",
                    "get_available_thinking_levels",
                    "get_commands",
                    "get_session_stats",
                ] {
                    self.command(json!({"type":kind}));
                }
            }
            "rpgui_admission_refused" => {
                self.admission_refused = true;
                self.phase = Phase::Failed;
                self.error = Some(format!(
                    "Compatibility admission refused: {}",
                    record["blocked"]
                ));
            }
            "agent_start" => {
                self.phase = Phase::Running;
                self.interrupted = false;
            }
            // agent_end intentionally does not settle: queued follow-ups/retries can still execute.
            "agent_settled" => {
                self.interrupted = record["aborted"] == true;
                if self.phase != Phase::Failed {
                    self.phase = Phase::Idle;
                }
                self.dialog = None;
                for kind in ["get_state", "get_messages", "get_session_stats"] {
                    self.command(json!({"type":kind}));
                }
            }
            "compaction_start" | "auto_compaction_start" => self.phase = Phase::Compacting,
            "compaction_end" | "auto_compaction_end" => {
                if self.phase == Phase::Compacting {
                    self.phase = Phase::Running;
                }
            }
            "auto_retry_start" => self.phase = Phase::Retrying,
            "auto_retry_end" => {
                if record["success"] == false {
                    self.error = Some(
                        "Pi provider retry ended unsuccessfully; inspect the final error.".into(),
                    );
                }
            }
            "message_end" => {
                if record["message"]["stopReason"] == "error" {
                    self.phase = Phase::Failed;
                    self.error = Some(
                        record["message"]["errorMessage"]
                            .as_str()
                            .unwrap_or("Provider failed after prompt acceptance.")
                            .chars()
                            .take(8192)
                            .collect(),
                    );
                }
            }
            "message_start" => {
                if record["message"]["role"] == "assistant" {
                    append(&mut self.text, "\n\n**Assistant**\n\n", &mut self.omitted);
                }
                if record["message"]["role"] == "user" {
                    append(&mut self.text, "\n\n**User**\n\n", &mut self.omitted);
                    let content = &record["message"]["content"];
                    if let Some(text) = content.as_str() {
                        append(&mut self.text, text, &mut self.omitted);
                    }
                    if let Some(parts) = content.as_array() {
                        for part in parts {
                            if let Some(text) = part["text"].as_str() {
                                append(&mut self.text, text, &mut self.omitted);
                            }
                            if part["type"] == "image" {
                                append(&mut self.text, "\n[Attached image]\n", &mut self.omitted);
                            }
                        }
                    }
                }
            }
            "message_update" => {
                let event = &record["assistantMessageEvent"];
                if event["type"] == "text_delta" {
                    append(
                        &mut self.text,
                        event["delta"].as_str().unwrap_or(""),
                        &mut self.omitted,
                    );
                }
                if event["type"] == "thinking_delta" {
                    append(
                        &mut self.inspection,
                        event["delta"].as_str().unwrap_or(""),
                        &mut self.omitted,
                    );
                }
            }
            "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
                let displayed = if record["toolName"] == "subagent" {
                    json!({"type":record["type"],"toolName":"subagent", "toolCallId":record["toolCallId"], "inspection":"Use structured child projection below; private routing metadata is not displayed."})
                } else {
                    record.clone()
                };
                append(
                    &mut self.inspection,
                    &format!("\n{}\n", displayed),
                    &mut self.omitted,
                );
                if record["type"] == "tool_execution_end" {
                    self.dialog = None;
                    if self.phase == Phase::Attention {
                        self.phase = Phase::Running;
                    }
                }
            }
            "extension_ui_request" => match record["method"].as_str().unwrap_or("") {
                "select" | "confirm" | "input" | "editor" => {
                    let deadline = record["timeout"]
                        .as_u64()
                        .map(|ms| now + Duration::from_millis(ms.min(86400000)));
                    self.dialog = Some(Dialog { record, deadline });
                    self.phase = Phase::Attention;
                }
                "notify" => {
                    if let Some(message) = record["message"].as_str()
                        && let Ok(envelope) = serde_json::from_str::<Value>(message)
                        && envelope["bridge"] == "rpgui-subagents"
                        && envelope["version"] == 1
                    {
                        self.child_observed = envelope["observedAt"].as_u64();
                        if envelope["kind"] == "reply" {
                            let Some(id) = envelope["data"]["requestId"].as_str() else {
                                return;
                            };
                            let Some(sequence) = id
                                .strip_prefix(&format!("gui-child-{}-", self.attempt))
                                .and_then(|id| id.rsplit('-').next())
                                .and_then(|seq| seq.parse::<u64>().ok())
                            else {
                                return;
                            };
                            if sequence <= self.child_applied || sequence > self.subagent_sequence {
                                return;
                            }
                            self.child_applied = sequence;
                            if envelope["data"]["requestId"]
                                .as_str()
                                .is_some_and(|id| id.contains("-ping-"))
                            {
                                if envelope["data"]["success"] == true
                                    && envelope["data"]["data"]["capabilities"]["fleetStatus"]["version"]
                                        == 1
                                {
                                    self.child_ready = true;
                                    if self.commands_ready {
                                        self.phase = Phase::Idle;
                                    }
                                } else {
                                    self.apply(attempt,Update::Failed("Unsupported child ownership capability response; admission refused."),now);
                                }
                                return;
                            }
                            self.children = envelope["data"]["data"].clone();
                        } else {
                            self.children = envelope["data"].clone();
                        }
                        return;
                    }
                    self.notifications.push(
                        record["message"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(8192)
                            .collect(),
                    );
                    if self.notifications.len() > 16 {
                        self.notifications.remove(0);
                    }
                }
                "set_editor_text" => self.draft = record["text"].as_str().unwrap_or("").into(),
                "setStatus" | "setWidget" | "setTitle" => {
                    append(
                        &mut self.inspection,
                        &format!("\n{}\n", record),
                        &mut self.omitted,
                    );
                }
                _ => {
                    self.error =
                        Some("Unsupported extension UI method; not silently approved.".into());
                }
            },
            "response" => {
                if self
                    .pending_prompt
                    .as_ref()
                    .is_some_and(|(id, _)| record["id"] == *id)
                {
                    let (_, text) = self.pending_prompt.take().unwrap();
                    if record["success"] == true {
                        if self.draft == text {
                            self.draft.clear();
                        }
                        if self.image_revision == self.pending_image_revision {
                            self.remove_image();
                        }
                    }
                }
                if record["success"] != true {
                    if record["command"] == "compact" && self.phase == Phase::Compacting {
                        self.phase = Phase::Idle;
                    }
                    self.error = Some(
                        record["error"]
                            .as_str()
                            .unwrap_or("Pi command failed; do not infer durable success.")
                            .chars()
                            .take(8192)
                            .collect(),
                    );
                    return;
                }
                let data = &record["data"];
                match record["command"].as_str().unwrap_or("") {
                    "get_state" => self.state = data.clone(),
                    "set_model" | "set_thinking_level" => {
                        for kind in [
                            "get_state",
                            "get_available_thinking_levels",
                            "get_session_stats",
                        ] {
                            self.command(json!({"type":kind}));
                        }
                    }
                    "get_session_stats" => self.statistics = data.clone(),
                    "get_available_models" => {
                        self.models = data["models"].as_array().cloned().unwrap_or_default()
                    }
                    "get_available_thinking_levels" => {
                        self.levels = data["levels"]
                            .as_array()
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|v| v.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default()
                    }
                    "get_commands" => {
                        self.commands = data["commands"].as_array().cloned().unwrap_or_default();
                        self.commands_ready = true;
                        if self.child_ready && self.phase == Phase::Starting {
                            self.phase = Phase::Idle;
                        }
                    }
                    "get_messages" => self.history(data),
                    "clear_queue" => {
                        self.steering.clear();
                        self.follow_up.clear();
                    }
                    "compact" => {
                        self.phase = Phase::Idle;
                        for kind in ["get_state", "get_messages", "get_session_stats"] {
                            self.command(json!({"type":kind}));
                        }
                    }
                    _ => {}
                }
            }
            "queue_update" => {
                self.steering = strings(&record["steering"]);
                self.follow_up = strings(&record["followUp"]);
            }
            "rpgui_owner_done"
                if (record["uncertain"] == true
                    || record["scope"]["uncertain"] == true
                    || record["parent"]["signal"].is_string()) =>
            {
                self.phase = Phase::Uncertain;
                self.error = Some(
                    "Retained shutdown proof is uncertain; history/worktree preserved.".into(),
                );
            }
            _ => {}
        }
    }
    pub(crate) fn inspect_children(&mut self, id: Option<String>) {
        if !self.subagents {
            self.error = Some("No admitted structured subagent owner is active.".into());
            return;
        }
        self.child_request("status", id);
    }
    fn child_request(&mut self, method: &str, id: Option<String>) {
        self.subagent_sequence += 1;
        let params = id.map(|id| json!({"id":id,"view":"transcript","lines":80}));
        let request = json!({"version":1,"requestId":format!("gui-child-{}-{method}-{}",self.attempt,self.subagent_sequence),"method":method,"params":params});
        self.command(json!({"type":"prompt","message":format!("/rpgui-subagents {request}")}));
    }
    fn history(&mut self, data: &Value) {
        self.text.clear();
        if let Some(messages) = data["messages"].as_array() {
            for message in messages {
                let role = message["role"].as_str().unwrap_or("entry");
                if role == "system" {
                    continue;
                }
                if let Some(error) = message["errorMessage"].as_str() {
                    append(
                        &mut self.text,
                        &format!(
                            "\nProvider error: {}\n",
                            error.chars().take(8192).collect::<String>()
                        ),
                        &mut self.omitted,
                    );
                }
                append(
                    &mut self.text,
                    &format!("\n\n**{role}**\n\n"),
                    &mut self.omitted,
                );
                if let Some(text) = message["content"].as_str() {
                    append(&mut self.text, text, &mut self.omitted);
                }
                if let Some(parts) = message["content"].as_array() {
                    for part in parts {
                        if let Some(text) = part["text"].as_str() {
                            append(&mut self.text, text, &mut self.omitted);
                        }
                        if part["type"] == "image" {
                            append(&mut self.text, "\n[Attached image]\n", &mut self.omitted);
                        }
                    }
                }
            }
        }
    }
}
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap_or("").to_owned())
                .collect()
        })
        .unwrap_or_default()
}
fn append(target: &mut String, text: &str, omitted: &mut bool) {
    const LIMIT: usize = 1024 * 1024;
    target.push_str(text);
    if target.len() > LIMIT {
        let mut offset = target.len() - LIMIT;
        while !target.is_char_boundary(offset) {
            offset += 1;
        }
        target.drain(..offset);
        *omitted = true;
    }
}
#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod tests;
