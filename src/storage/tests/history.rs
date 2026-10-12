use super::*;
#[test]
fn passive_history_and_web_custom_output() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.jsonl");
    let original = "{\"type\":\"session\",\"id\":\"pi-session\",\"cwd\":\"/original\"}\n{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"saved output\"}]}}\n{\"type\":\"custom\",\"customType\":\"web-search-results\",\"data\":{\"source\":\"persisted\"}}\n";
    std::fs::write(&path, original).unwrap();
    let history = read(&path).unwrap();
    assert!(history.markdown.contains("saved output"));
    assert!(history.inspection.contains("persisted"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}
#[test]
fn malformed_history_is_refused_without_repair() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.jsonl");
    std::fs::write(&path, "{\"type\":\"message\"}\n").unwrap();
    assert!(read(&path).is_err());
    std::fs::write(&path, "partial").unwrap();
    assert!(read(&path).is_err());
}
#[test]
fn passive_images_are_identified_and_oversized_frames_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.jsonl");
    std::fs::write(&path,"{\"type\":\"session\",\"cwd\":\"/original\"}\n{\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"image\",\"data\":\"not-projected\"}]}}\n").unwrap();
    let history = read(&path).unwrap();
    assert!(history.markdown.contains("Attached image"));
    assert!(!history.markdown.contains("not-projected"));
    std::fs::write(&path, vec![b'x'; 2 * 1024 * 1024 + 1]).unwrap();
    assert!(read(&path).is_err());
}
