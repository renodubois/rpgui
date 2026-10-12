//! Retained Linux descendant ownership. No PID/name search; signals require an atomically held pidfd and our parent scope.
use std::{fs, io, os::unix::process::CommandExt, process::Command};
pub(crate) fn internal_mode() -> bool {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--rpgui-retained-node") => {
            let result = (|| -> io::Result<()> {
                if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0 {
                    return Err(io::Error::last_os_error());
                }
                let descriptor =
                    unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0) } as i32;
                if descriptor < 0 {
                    return Err(io::Error::last_os_error());
                }
                let probe = unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        descriptor,
                        0,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    )
                };
                unsafe { libc::close(descriptor) };
                if probe < 0 {
                    return Err(io::Error::last_os_error());
                }
                let binary = std::env::current_exe()?;
                let mut node = Command::new("node");
                node.args(&args[1..]).env("RPGUI_OWNER_HELPER", binary);
                Err(node.exec())
            })();
            if result.is_err() {
                std::process::exit(1);
            }
            true
        }
        Some("--rpgui-signal-child") => {
            let result = (|| -> io::Result<()> {
                if args.len() != 4 {
                    return Err(io::Error::other("Invalid child signal"));
                }
                let pid: libc::pid_t = args[1].parse().map_err(io::Error::other)?;
                let signal = match args[3].as_str() {
                    "TERM" => libc::SIGTERM,
                    "KILL" => libc::SIGKILL,
                    _ => return Err(io::Error::other("Invalid child signal")),
                };
                let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
                if descriptor < 0 {
                    return Err(io::Error::last_os_error());
                }
                let result = (|| -> io::Result<()> {
                    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
                    let fields: Vec<_> = stat
                        .rsplit_once(") ")
                        .ok_or_else(|| io::Error::other("Invalid child identity"))?
                        .1
                        .split_whitespace()
                        .collect();
                    if fields.get(19) != Some(&args[2].as_str())
                        || fields.get(1).and_then(|s| s.parse::<libc::pid_t>().ok())
                            != Some(unsafe { libc::getppid() })
                    {
                        return Err(io::Error::other("Not a current child of retained scope"));
                    }
                    if unsafe {
                        libc::syscall(
                            libc::SYS_pidfd_send_signal,
                            descriptor,
                            signal,
                            std::ptr::null::<libc::siginfo_t>(),
                            0,
                        )
                    } < 0
                    {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                })();
                unsafe { libc::close(descriptor) };
                result
            })();
            std::process::exit(if result.is_ok() { 0 } else { 1 });
        }
        _ => false,
    }
}
