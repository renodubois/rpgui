use super::*;
#[test]
fn signature_size_and_base64_are_checked() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image");
    std::fs::write(&path, b"GIF89a").unwrap();
    let result = read(&path).unwrap();
    assert_eq!(result["data"], "R0lGODlh");
    assert_eq!(result["mimeType"], "image/gif");
    std::fs::write(&path, b"not an image").unwrap();
    assert!(read(&path).is_err());
    std::fs::write(&path, vec![0; 1024 * 1024 + 1]).unwrap();
    assert!(read(&path).is_err());
}
