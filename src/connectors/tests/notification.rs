use super::*;
#[test]
fn ding_is_short_bounded_and_fades_without_native_playback() {
    let bytes = samples();
    assert_eq!(bytes.len(), 8820);
    let last = i16::from_le_bytes([bytes[8818], bytes[8819]]);
    assert!(last.abs() < 10);
}
