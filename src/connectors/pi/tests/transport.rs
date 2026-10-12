use super::*;
fn mailbox() -> Arc<Mutex<Mailbox>> {
    Arc::new(Mutex::new(Mailbox {
        records: VecDeque::new(),
        bytes: 0,
        terminal: None,
        sealed: false,
    }))
}
#[test]
fn strict_lf_unicode_and_crlf() {
    let shared = mailbox();
    let (send, recv) = mpsc::sync_channel(8);
    drain(
        &b"{\"type\":\"message\",\"text\":\"a\xe2\x80\xa8b\xe2\x80\xa9\xc3\xa9\"}\r\n"[..],
        send,
        shared.clone(),
    );
    assert_eq!(recv.recv().unwrap()["text"], "a\u{2028}b\u{2029}é");
    assert!(shared.lock().unwrap().terminal.is_none());
}
#[test]
fn invalid_large_incomplete_and_slow_consumers_fail_explicitly() {
    for input in [
        b"not json\n".to_vec(),
        b"{\"type\":\"event\"}".to_vec(),
        vec![b'x'; MAX_FRAME + 1],
    ] {
        let shared = mailbox();
        let (send, _) = mpsc::sync_channel(1);
        drain(&input[..], send, shared.clone());
        assert!(matches!(
            shared.lock().unwrap().terminal,
            Some(Update::Failed(_))
        ));
    }
    let shared = mailbox();
    let (send, _receive) = mpsc::sync_channel(1);
    drain(
        &b"{\"type\":\"one\"}\n{\"type\":\"two\"}\n"[..],
        send,
        shared.clone(),
    );
    assert!(matches!(
        shared.lock().unwrap().terminal,
        Some(Update::Failed(_))
    ));
}
#[test]
fn retained_output_is_bounded_and_failure_not_silent() {
    let shared = mailbox();
    let mut shared = shared.lock().unwrap();
    for _ in 0..=MAX_RECORDS {
        shared.push(json!({"type":"delta"}), 16);
    }
    assert_eq!(shared.records.len(), 0);
    assert_eq!(shared.bytes, 0);
    assert!(matches!(shared.terminal, Some(Update::Failed(_))));
}
