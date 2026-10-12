# Architecture and ownership

One Rust binary crate; no server, application framework, universal connector, or product event bus. GPUI Kit is pinned to 0.7.1; Rust to 1.97.0.

## Production owners

- `src/main.rs`: opens the canonical kernel-locked profile, constructs the native window and dependencies. Internal retained-process modes exit before GUI/profile initialization.
- `src/views/app_shell.rs`: retains `Entity<Workspace>`, consumes opaque worker updates, polls independently of selection, and coordinates confirmed window closure. Dropping/recreating the conversation view does not own execution.
- `src/workspace/mod.rs`: project/thread navigation, metadata policy, ordered blocking work, thread creation reservations, Git/image intentions, transition attention, mute and exit workflow. It retains each thread owner and the profile until accepted storage work drains.
- `src/threads/mod.rs`: ordinary Pi thread workflow, attempts, prompt admission/acknowledgement, draft/image state, explicit delivery mode, queues, standard dialogs, model/thinking intentions, compaction, history reconciliation and structured child observations. Prompt acceptance and `agent_end` are not settlement; only `agent_settled` ends a generation.
- `src/views/workspace.rs` and its owner-private `workspace/` presentation modules: compact navigation, conversation/activity viewport, composer/queues, bounded attention/exit surfaces, on-demand session forms, thread-action menu and status presentation. Kit controls submit owner intentions; views own only local panels/scrolling and do not launch processes or access storage. `src/theme.rs` installs the coordinated dark palette and density policy.
- `src/worktrees/mod.rs`: explicit committed-worktree policy. No dirty copying, integration or automatic cleanup.

## Mechanics

`src/connectors/pi/` owns bounded JSONL pipes, continuously drained diagnostics, correlated command IDs, write/read/ack deadlines and sticky terminal faults. A transport error can mean an ambiguous mutation; nothing is retried or replayed. Frame, mailbox and write budgets are explicit. `src/connectors/git/` owns safe Git invocation, worktree mechanics and porcelain decoding. Checkout filters/hooks/fsmonitor are not executed by untrusted GUI operations. `src/connectors/notification.rs` owns only its bounded `paplay` child; failure is surfaced rather than claimed audible.

`src/connectors/process_scope.rs` executes the retained Node launcher with Linux `PR_SET_CHILD_SUBREAPER`. After Pi closure, orphaned local descendants—including daemonized ordinary Bash jobs—remain children of the dedicated per-thread launcher. A native pidfd helper checks actual PPID and start identity before signaling. It never searches process names or signals another thread's process group. Native process closure is independent of the package's whole-run writer-close proof.

`src/storage/` owns atomic metadata writes, stable profile locking, authoritative passive Pi history, bounded image reads and persisted process-identity inspection. Metadata contains project/thread association, directory, session reference, archive/interrupted/trust flags and mute, never a transcript or credentials. Storage validates private namespace components before creating descendants; metadata save/read budgets agree. Draft-local image revisions reject cancelled or old receipt callbacks. Unknown external saved history is an explicitly read-only snapshot, never an adopted execution owner. Invalid metadata refuses startup rather than overwriting it. Legacy/partial process proofs do not establish descendant cleanup.

## Retained runtime

`scripts/pi-runtime/launch.mjs` performs offline resource discovery/build verification and explicit admission before extension startup. It starts ordinary Pi RPC through `guardian.mjs`; it does not implement another agent/tool runtime. `catalog.mjs` classifies reviewed Q+A replacement, guard, tool-search, supported subagents and HTTP web configurations. Unknown owners, MCP, llama.cpp, codemode model helpers and unsupported workflows refuse admission rather than being silently removed. Ordinary model/auth settings remain Pi's responsibility.

`extensions/pi-gui-guard.ts` uses public required-child-extension and launch-contract APIs, inherited into nested SDK children. It refuses unaudited child ownership and per-tool preflight/override bypasses; projects versioned bounded public observations without private routing/control metadata. Capability ping and command discovery gate native admission. Pi executes extension commands before input hooks: native thread command-provenance policy therefore refuses unknown/unaudited slash workflows before RPC dispatch, rather than relying on an ineffective input hook. Snapshot timestamps/caps/omissions remain visible; observations are not completion proof.

Guardian closure clears queues and aborts first, stops private-scope package runners, closes Pi, reconciles again, then waits for retained Linux descendant closure. Active scope admission additionally requires a healthy opaque transport lease retained by the workspace worker, not merely the same live frontend PID. Stop proceeds even when phase journaling fails, reports uncertainty and attempts only owned descendant closure after abnormal guardian exit. Durable identities/proofs preserve uncertainty after owner loss, unsupported whole-run proof, interrupted writers, or stop deadlines. A new GUI may read history passively but refuses new writable admission while retained ownership is unproved.

Local process closure does not roll back external HTTP/provider side effects. This is not a security sandbox for arbitrary shell programs. Remote/background product execution, Durable coordinator, branching, terminal attachment and arbitrary custom TUI remain outside milestone 1.

## Tests and changes

All Rust suites are owner-local `tests/<subject>.rs`, declared by their production modules. Production Node integration lives under `scripts/pi-runtime/tests/integration/`; compatibility fixtures remain separate under `scripts/pi-compat/`. See `VERIFY.md` and `PI-GUI-MILESTONE-1.md` for actual evidence and residual native limitations.
