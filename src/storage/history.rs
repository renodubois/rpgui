//! Passive, bounded projection from authoritative Pi JSONL. Never writes or replays history.
use serde_json::Value;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read},
    path::Path,
};
pub(crate) struct History {
    pub markdown: String,
    pub inspection: String,
    pub omitted: bool,
    pub cwd: std::path::PathBuf,
}
pub(crate) fn read(path: &Path) -> io::Result<History> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = File::options()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Saved history must be a regular file"));
    }
    if file.metadata()?.len() > 32 * 1024 * 1024 {
        return Err(io::Error::other("Session exceeds passive history budget"));
    }
    let mut history = History {
        markdown: String::new(),
        inspection: String::new(),
        omitted: false,
        cwd: std::path::PathBuf::new(),
    };
    let mut reader = BufReader::new(file);
    let mut frame = Vec::new();
    let mut count = 0;
    let mut consumed = 0;
    loop {
        frame.clear();
        let size = reader
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut frame)?;
        consumed += size;
        if consumed > 32 * 1024 * 1024 {
            return Err(io::Error::other("Session exceeds passive history budget"));
        }
        if size == 0 {
            break;
        }
        if size > 2 * 1024 * 1024 {
            return Err(io::Error::other("Session entry exceeds frame budget"));
        }
        let value: Value = serde_json::from_slice(&frame)?;
        if count == 0 && value["type"] != "session" {
            return Err(io::Error::other("Missing Pi session header"));
        }
        if count == 0 {
            let cwd = value["cwd"]
                .as_str()
                .ok_or_else(|| io::Error::other("Missing saved working directory"))?;
            history.cwd = cwd.into();
            if !history.cwd.is_absolute() {
                return Err(io::Error::other("Saved working directory is not absolute"));
            }
        }
        count += 1;
        if value["type"] == "message" {
            let message = &value["message"];
            if message["role"] == "toolResult" && message["toolName"] == "subagent" {
                retain(
                    &mut history.inspection,
                    "Subagent result retained in Pi history; use reviewed structured child inspection.\n",
                    &mut history.omitted,
                );
                continue;
            }
            retain(
                &mut history.markdown,
                &format!(
                    "\n\n**{}**\n\n",
                    message["role"].as_str().unwrap_or("entry")
                ),
                &mut history.omitted,
            );
            if let Some(error) = message["errorMessage"].as_str() {
                retain(
                    &mut history.markdown,
                    &format!(
                        "\nProvider error: {}\n",
                        error.chars().take(8192).collect::<String>()
                    ),
                    &mut history.omitted,
                );
            }
            if let Some(text) = message["content"].as_str() {
                retain(&mut history.markdown, text, &mut history.omitted);
            }
            if let Some(parts) = message["content"].as_array() {
                for part in parts {
                    if part["type"] == "image" {
                        retain(
                            &mut history.markdown,
                            "\n[Attached image]\n",
                            &mut history.omitted,
                        );
                    }
                    if let Some(text) = part["text"].as_str() {
                        retain(&mut history.markdown, text, &mut history.omitted);
                    }
                    if matches!(part["type"].as_str(), Some("thinking" | "toolCall")) {
                        retain(
                            &mut history.inspection,
                            &format!("{part}\n"),
                            &mut history.omitted,
                        );
                    }
                }
            }
            if message["role"] == "toolResult" {
                retain(
                    &mut history.inspection,
                    &format!("{message}\n"),
                    &mut history.omitted,
                );
            }
        }
        if value["type"] == "custom" {
            let display = if value["customType"] == "web-search-results" {
                value.clone()
            } else {
                serde_json::json!({"customType":value["customType"],"note":"Custom entry preserved in authoritative Pi history; raw control payload not projected."})
            };
            retain(
                &mut history.inspection,
                &format!("{display}\n"),
                &mut history.omitted,
            );
        }
    }
    if count == 0 {
        return Err(io::Error::other("Missing Pi session header"));
    }
    Ok(history)
}
fn retain(target: &mut String, value: &str, omitted: &mut bool) {
    target.push_str(value);
    if target.len() > 1024 * 1024 {
        let mut offset = target.len() - 1024 * 1024;
        while !target.is_char_boundary(offset) {
            offset += 1;
        }
        target.drain(..offset);
        *omitted = true;
    }
}
#[cfg(test)]
#[path = "tests/history.rs"]
mod tests;
