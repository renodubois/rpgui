//! Owned bounded native audio transport. No focus changes, shell or provider/credential access.
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
pub(crate) fn attention() -> mpsc::Receiver<Result<(), &'static str>> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = (|| {
            let mut child=Command::new("paplay").args(["--raw","--rate=44100","--channels=1","--format=s16le"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"Notification audio unavailable (paplay missing or isolated audio server unavailable).")?;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&samples());
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if let Ok(Some(status)) = child.try_wait() {
                    return if status.success() {
                        Ok(())
                    } else {
                        Err("Notification audio unavailable in this session.")
                    };
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Notification audio deadline exceeded.");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })();
        let _ = send.send(result);
    });
    receive
}
fn samples() -> Vec<u8> {
    (0..4410)
        .flat_map(|i| {
            (((i as f32 * 880.0 * std::f32::consts::TAU / 44100.0).sin()
                * 1800.0
                * (1.0 - i as f32 / 4410.0)) as i16)
                .to_le_bytes()
        })
        .collect()
}
#[cfg(test)]
#[path = "tests/notification.rs"]
mod tests;
