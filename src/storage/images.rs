//! Bounded image file admission; image bytes are never persisted in GUI metadata or diagnostics.
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    path::Path,
};
pub(crate) fn read(path: &Path) -> io::Result<Value> {
    let mut bytes = Vec::new();
    use std::os::unix::fs::OpenOptionsExt;
    let path = std::fs::canonicalize(path)?;
    let file = std::fs::File::options()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Image input must be a regular file"));
    }
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(io::Error::other("Image exceeds 1 MiB input limit"));
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return Err(io::Error::other(
            "Unsupported image signature (PNG/JPEG/GIF/WebP required)",
        ));
    };
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut data = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        data.push(TABLE[a >> 2] as char);
        data.push(TABLE[((a & 3) << 4) | (b >> 4)] as char);
        data.push(if chunk.len() > 1 {
            TABLE[((b & 15) << 2) | (c >> 6)] as char
        } else {
            '='
        });
        data.push(if chunk.len() > 2 {
            TABLE[c & 63] as char
        } else {
            '='
        });
    }
    Ok(json!({"type":"image", "mimeType":mime, "data":data}))
}
#[cfg(test)]
#[path = "tests/images.rs"]
mod tests;
