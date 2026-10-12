use super::label;
use crate::threads::Phase;

#[test]
fn every_phase_has_a_distinct_human_readable_status() {
    let phases = [
        Phase::Passive,
        Phase::Starting,
        Phase::Idle,
        Phase::Running,
        Phase::Compacting,
        Phase::Retrying,
        Phase::Attention,
        Phase::Failed,
        Phase::Uncertain,
        Phase::Closing,
    ];
    let labels: std::collections::HashSet<_> = phases.into_iter().map(label).collect();
    assert_eq!(labels.len(), phases.len());
    assert_eq!(label(Phase::Passive), "Saved");
    assert_eq!(label(Phase::Attention), "Needs input");
    assert_eq!(label(Phase::Uncertain), "Uncertain");
}
