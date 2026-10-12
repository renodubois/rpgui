use super::*;
fn record(t: &mut Thread, attempt: u64, value: Value) {
    t.apply(attempt, Update::Record(value), Instant::now());
}
#[test]
fn acceptance_and_agent_end_are_not_settled_and_stale_attempts_are_ignored() {
    let mut t = Thread::passive(7);
    t.attempt = 2;
    record(&mut t, 2, json!({"type":"agent_start"}));
    record(
        &mut t,
        2,
        json!({"type":"response","command":"prompt","success":true,"data":{"disposition":"started"}}),
    );
    assert_eq!(t.phase, Phase::Running);
    record(&mut t, 2, json!({"type":"agent_end"}));
    assert_eq!(t.phase, Phase::Running);
    record(&mut t, 1, json!({"type":"agent_settled"}));
    assert_eq!(t.phase, Phase::Running);
    record(&mut t, 2, json!({"type":"agent_settled"}));
    assert_eq!(t.phase, Phase::Idle);
}
#[test]
fn independent_threads_route_hidden_dialogs_and_retire_timeout_tool_end_crash() {
    let mut a = Thread::passive(1);
    let mut b = Thread::passive(2);
    record(&mut a, 0, json!({"type":"agent_start"}));
    record(
        &mut b,
        0,
        json!({"type":"extension_ui_request", "id":"question", "method":"confirm", "timeout":1}),
    );
    assert_eq!(a.phase, Phase::Running);
    assert_eq!(b.phase, Phase::Attention);
    b.answer(9, "question", json!({"confirmed":true}), Instant::now());
    assert!(b.dialog.is_some());
    b.poll(Instant::now() + Duration::from_secs(1));
    assert!(b.dialog.is_none());
    record(
        &mut b,
        0,
        json!({"type":"extension_ui_request", "id":"next", "method":"input"}),
    );
    record(&mut b, 0, json!({"type":"tool_execution_end"}));
    assert!(b.dialog.is_none());
    b.apply(0, Update::Failed("crash after dispatch"), Instant::now());
    assert_eq!(b.phase, Phase::Uncertain);
    assert_eq!(a.phase, Phase::Running);
}
#[test]
fn queues_are_complete_projections_and_huge_output_is_bounded() {
    let mut t = Thread::passive(1);
    record(
        &mut t,
        0,
        json!({"type":"queue_update","steering":["change"],"followUp":["later"]}),
    );
    assert_eq!(t.steering, ["change"]);
    assert_eq!(t.follow_up, ["later"]);
    record(
        &mut t,
        0,
        json!({"type":"queue_update","steering":[],"followUp":[]}),
    );
    assert!(t.steering.is_empty());
    record(
        &mut t,
        0,
        json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"é".repeat(1024*1024)}}),
    );
    assert!(t.omitted);
    assert!(t.text.len() <= 1024 * 1024);
}

#[test]
fn child_capability_ping_gates_startup_and_users_stream_from_authoritative_events() {
    let mut t = Thread::passive(8);
    t.attempt = 2;
    record(
        &mut t,
        2,
        json!({"type":"rpgui_owner_ready","subagents":true}),
    );
    assert_eq!(t.phase, Phase::Starting);
    record(
        &mut t,
        2,
        json!({"type":"extension_ui_request","method":"notify","message":json!({"bridge":"rpgui-subagents","version":1,"kind":"reply","data":{"requestId":"gui-child-2-ping-1","success":true,"data":{"capabilities":{"fleetStatus":{"version":1}}}}}).to_string()}),
    );
    assert_eq!(t.phase, Phase::Starting);
    record(
        &mut t,
        2,
        json!({"type":"response","command":"get_commands","success":true,"data":{"commands":[]}}),
    );
    assert_eq!(t.phase, Phase::Idle);
    record(
        &mut t,
        2,
        json!({"type":"message_start","message":{"role":"user","content":[{"type":"text","text":"Sent prompt"}]}}),
    );
    assert!(t.text.contains("Sent prompt"));
}

#[test]
fn provider_failure_and_auto_compaction_are_not_silent() {
    let mut t = Thread::passive(1);
    record(&mut t, 0, json!({"type":"agent_start"}));
    record(&mut t, 0, json!({"type":"auto_compaction_start"}));
    assert_eq!(t.phase, Phase::Compacting);
    record(&mut t, 0, json!({"type":"auto_compaction_end"}));
    assert_eq!(t.phase, Phase::Running);
    record(
        &mut t,
        0,
        json!({"type":"message_end","message":{"role":"assistant","stopReason":"error","errorMessage":"Controlled provider error"}}),
    );
    record(&mut t, 0, json!({"type":"agent_settled"}));
    assert_eq!(t.phase, Phase::Failed);
    assert!(
        t.error
            .as_ref()
            .unwrap()
            .contains("Controlled provider error")
    );
    t.apply(
        0,
        Update::Failed("owner lost after provider error"),
        Instant::now(),
    );
    assert_eq!(t.phase, Phase::Uncertain);
    assert!(t.interrupted);
    let error = t.error.as_ref().unwrap();
    assert!(error.contains("owner lost after provider error"));
    assert!(error.contains("Controlled provider error"));
    t.close();
    assert_eq!(t.phase, Phase::Uncertain);
}

#[test]
fn disconnected_close_preserves_phase_and_explicit_refusal_is_not_dispatch_uncertainty() {
    for phase in [Phase::Passive, Phase::Failed, Phase::Uncertain] {
        let mut t = Thread::passive(1);
        t.phase = phase;
        t.close();
        assert_eq!(t.phase, phase);
        assert!(!t.interrupted);
    }
    let mut t = Thread::passive(1);
    record(
        &mut t,
        0,
        json!({"type":"rpgui_admission_refused","blocked":[{"code":"unreviewed-owner"}]}),
    );
    t.apply(0, Update::Failed("launcher refused"), Instant::now());
    assert_eq!(t.phase, Phase::Failed);
    assert!(t.error.as_ref().unwrap().contains("unreviewed-owner"));
}
#[test]
fn package_slash_bypass_is_refused_before_native_rpc_dispatch() {
    let mut t = Thread::passive(1);
    t.phase = Phase::Idle;
    t.commands = vec![
        json!({"name":"subagents-detach","source":"extension","sourceInfo":{"path":"/reviewed/package/index.js"}}),
    ];
    t.prompt("/subagents-detach unsafe".into(), Delivery::Steer, vec![]);
    assert!(t.error.as_ref().unwrap().contains("before RPC dispatch"));
    assert!(t.pending_prompt.is_none());
}
#[test]
fn admission_ack_does_not_clear_an_attachment_selected_for_the_next_draft() {
    let mut t = Thread::passive(1);
    t.attempt = 3;
    t.pending_attempt = 3;
    t.pending_prompt = Some(("receipt".into(), "submitted".into()));
    t.pending_image_revision = 0;
    t.image_revision = 1;
    t.images.push(json!({"type":"image","data":"next-draft"}));
    record(
        &mut t,
        3,
        json!({"type":"response","id":"receipt","command":"prompt","success":true}),
    );
    assert_eq!(t.images.len(), 1);
    assert!(t.pending_prompt.is_none());
}
