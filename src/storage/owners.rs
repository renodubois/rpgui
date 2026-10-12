//! Persisted Linux process-attempt identity inspection. No PID/name search or process termination.
use serde_json::{Value, json};
use std::{fs, io, path::Path};
pub(crate) fn identity(pid: u32) -> io::Result<Value> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields: Vec<_> = stat
        .rsplit_once(") ")
        .ok_or_else(|| io::Error::other("Invalid process identity"))?
        .1
        .split_whitespace()
        .collect();
    Ok(
        json!({"pid":pid,"startTicks":fields.get(19).ok_or_else(||io::Error::other("Invalid process start identity"))?,"bootId":fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim()}),
    )
}
fn alive(expected: &Value) -> bool {
    let Some(pid) = expected["pid"]
        .as_u64()
        .and_then(|pid| u32::try_from(pid).ok())
    else {
        return false;
    };
    if identity(pid).ok().as_ref() != Some(expected) {
        return false;
    }
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rsplit_once(") ").map(|(_, tail)| !tail.starts_with("Z ")))
        .unwrap_or(false)
}
pub(crate) fn admission(
    root: &Path,
    frontend: &Value,
    active: &dyn Fn(&Path) -> bool,
) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut budget = 0;
    for thread in fs::read_dir(root)? {
        let thread = thread?;
        if thread.file_type()?.is_symlink() || !thread.file_type()?.is_dir() {
            return Err(io::Error::other("Invalid retained owner directory"));
        }
        for attempt in fs::read_dir(thread.path())? {
            budget += 1;
            if budget > 4096 {
                return Err(io::Error::other("Retained owner inventory exceeds budget"));
            }
            let attempt = attempt?;
            if attempt.file_type()?.is_symlink() || !attempt.file_type()?.is_dir() {
                return Err(io::Error::other("Invalid process attempt directory"));
            }
            let scope = attempt.path();
            let owner_path = scope.join("guardian-owner.json");
            if !owner_path.exists() {
                if let Ok(launcher) = read(&scope.join("launcher.json"))
                    && launcher["version"] == 1
                    && launcher["frontend"] == *frontend
                    && alive(&launcher["owner"])
                    && active(&scope)
                {
                    continue;
                }
                let refusal = read(&scope.join("owner-refusal.json"))?;
                if refusal["version"] == 1 && refusal["admitted"] == false {
                    continue;
                }
                return Err(io::Error::other(
                    "Incomplete owner launch; admission refused",
                ));
            }
            let owner = read(&owner_path)?;
            if owner["version"] != 1 || owner["scope"].as_str() != scope.to_str() {
                return Err(io::Error::other("Owner identity/scope drift"));
            }
            if owner["phase"] == "runner-observation-complete" {
                let proof = read(&scope.join("guardian-proof.json"))?;
                if proof["type"] == "rpgui_owner_done"
                    && proof["uncertain"] == false
                    && proof["parent"]["code"] == 0
                    && proof["parent"]["signal"].is_null()
                    && proof["descendants"]["proof"] == "retained-linux-subreaper-pidfd"
                    && proof["descendants"]["uncertain"] == false
                    && !alive(&owner["pi"])
                {
                    continue;
                }
            } else if active(&scope)
                && owner["frontend"] == *frontend
                && alive(&owner["owner"])
                && alive(&owner["pi"])
            {
                continue;
            }
            return Err(io::Error::other(
                "Retained execution owner lost or cleanup unproved. Admission refused; history reopens passively without replay.",
            ));
        }
    }
    Ok(())
}
pub(crate) fn record_launcher(scope: &Path, pid: u32, frontend: &Value) -> io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(scope.join("launcher.json"))?;
    file.write_all(
        json!({"version":1,"frontend":frontend,"owner":identity(pid)?})
            .to_string()
            .as_bytes(),
    )?;
    file.sync_all()?;
    fs::File::open(scope)?.sync_all()
}
fn read(path: &Path) -> io::Result<Value> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    let file = fs::File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Owner evidence must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(io::Error::other("Owner evidence exceeds budget"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
#[cfg(test)]
#[path = "tests/owners.rs"]
mod tests;
