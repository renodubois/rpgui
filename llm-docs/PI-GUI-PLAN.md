# Pi GUI plan

Status: user-approved planning baseline from the design interview. Milestone 0 controlled compatibility proof is complete; exact supported/unsupported boundaries are recorded in PI-GUI-COMPATIBILITY.md. Architecture details remain implementation recommendations; GUI/coordinator implementation has not started.

## Agreed product decisions

- Linux, personal use, using this Rust/GPUI template.
- One GUI process/window per profile, containing multiple projects and Pi instances.
- Target 2–8 active top-level threads; saved conversations do not require live processes.
- Native conversation UI with Pi workflow parity, not terminal UI compatibility.
- Initial compatibility priorities: native Q+A (`ask_user`), pi-subagents, pi-web-access.
- Reuse existing Pi configuration for normal use. Keep GUI preferences and coordinator state separate. Development/tests remain isolated.
- Sidebar plus one conversation pane. Switching views never cancels running threads.
- Subagents appear as inspectable children of their owning thread, not independent top-level threads.
- Busy-thread messages default to steering, with an explicit follow-up choice and visible queue.
- In-app attention badges plus a small ding; do not steal focus. Proposed default: muteable sound on transitions into needs-attention.
- Open/continue saved Pi sessions; no attachment to already-running terminal Pi instances.
- Conversation branching, cloning and full session-tree navigation are deferred.
- Editing threads use isolated Git worktrees by default; explicitly shared directories remain available.
- New worktrees start from selected committed state. Never silently copy dirty files, commit or stash user changes.
- Dirty or non-Git directories can use an explicitly shared-directory thread.
- First-release Git scope: create/track worktrees and show branch/change status. Integration stays in the user's existing Git workflow.
- Preserve worktrees/changes when archiving threads; never automatically delete dirty worktrees.
- Trusted-local Pi execution, not sandboxing. A worktree is collision isolation, not a security boundary.
- Running work stops on application exit; confirm when active work would be interrupted.
- Preserve history and mark interrupted work. No automatic replay/continuation on reopening.
- A usable multi-thread GUI is an acceptable first milestone without the coordinator.

## Coordinator decisions

- Later milestone: Pi Durable advisor, one conversation per project.
- On-demand operation, not an active background supervisor.
- May inspect any thread in its project, including bounded transcript excerpts, tool activity and reported subagent status.
- Can explain activity and propose creating, messaging or stopping threads.
- Every state-changing thread action requires explicit user approval.
- Project-scoped context; no global cross-project advisor initially.
- Proposed restriction: no direct shell/filesystem-edit tools for the advisor. Its capabilities are focused project/thread inspection and action proposals.
- Durable persistence does not imply permission to continue work after GUI exit or automatically resume interrupted actions.

## Candidate architecture (implementation recommendations)

### Execution boundary

Ordinary threads remain ordinary Pi coding-agent runtimes, initially one `pi --mode rpc` child process per active thread. The GUI does not reimplement agent tools, compaction, model logic or Pi's session persistence.

Pi Durable is a separate harness, not a persistence flag on ordinary Pi sessions. Host it in an application-owned Node sidecar, communicating over a bounded, versioned local protocol. Do not build an HTTP server, remote service or daemon merely to bridge Rust and TypeScript.

Evaluate whether extension compatibility requires an SDK host or a small extension bridge during the feasibility milestone. Prefer RPC unless verified requirements demonstrate a missing capability. SDK use must explicitly reproduce CLI resource/builtin discovery where required; do not assume SDK defaults equal CLI defaults.

### Ownership

- Startup constructs dependencies, execution adapters, storage and window.
- A retained workspace owner manages project/thread identity and lifecycle, not every product behavior.
- Thread feature owners retain authoritative transcript projections, queues, run state, drafts and process-attempt identity independent of replaceable views.
- Worktree feature owns creation, branch/path association, status and preservation policy; Git connector owns subprocess/wire mechanics.
- Coordinator feature owns project scope, proposals, approval workflow and uncertainty.
- Pi connector owns JSONL framing, command correlation, event decoding and child transport.
- Durable connector owns sidecar transport/decoding; its TypeScript host owns the Durable harness and storage mechanics.
- Storage owns file mechanics, ordered writes and profile locking, not rendering or workflow decisions.
- Stable shell retains owners and delivers opaque updates; views render and submit intentions.
- No catch-all app struct, global event bus or universal connector trait.

Suggested Rust modules: `workspace/`, `threads/`, `worktrees/`, `coordinator/`, `connectors/pi/`, `connectors/git/`, `connectors/coordinator/`, focused `storage/` additions, and owning `views/` children. Final placement follows actual interfaces; do not pre-create empty abstractions.

### Identity and persisted state

Keep project ID, GUI thread ID, Pi session ID/path, worktree path/branch, subagent run ID and process-attempt ID distinct. Session switches, exits and stale events must not mutate another thread.

Pi session files remain authoritative for Pi history. GUI metadata stores project membership, associations, presentation preferences and other GUI-owned state, not a competing agent transcript. Coordinator state belongs to Durable storage, with exactly one owning process.

Saved-session ownership cannot be inferred merely because the GUI has not opened it. If exclusive ownership cannot be established, use a safe copy or refuse writable opening rather than risk another terminal instance writing the same session. Existing sessions retain their actual working-directory semantics; don't silently rebind them to a fresh worktree.

Archived threads remain saved; archiving is distinct from stopping, deleting history and removing a worktree. Proposed default: explicit destructive operations only, deferred automatic cleanup.

### Native conversation baseline

Proposed core controls: streaming Markdown/code, collapsible tool arguments/results and thinking, text/image input, model/thinking selection, prompt-template/skill/extension command discovery, compaction, Stop, steering/follow-up queues, usage/context display, rename/create/open/archive threads.

Map supported built-in terminal workflows to native controls; do not send unsupported TUI commands such as `/settings` blindly through RPC. Implement standard extension dialogs and notifications as native interactions. Do not claim arbitrary custom terminal component support.

Attention distinguishes awaiting user input/approval from normal tool activity, retry, compaction, idle, failed and interrupted states. A hidden thread remains able to receive a native answer. Honor extension timeouts/cancellation and never silently approve requests.

### Transport and lifecycle invariants

- Strict LF-delimited JSONL; correlate commands by ID, not response order. Continuously drain stdout and stderr, with bounded presentation/logging.
- Successful prompt acceptance is not completion. `agent_end` is not necessarily idle; use the installed protocol's settled semantics.
- Use attempt/generation identity to reject stale process updates. A dropped view must not drop a process or add another authoritative stream consumer.
- Coalesce disposable streaming presentation deltas if needed, preserving final messages and terminal/error outcomes. Reconcile projections from authoritative Pi state where possible; do not invent universal replay guarantees.
- Prompt transport failure after dispatch can be ambiguous. Do not retry state-changing commands automatically without an idempotency mechanism.
- Stop semantics distinguish clearing queued work, aborting generation, user bash, nested subagents and external provider jobs.
- On confirmed quit: stop admission, clear pending messages, cancel owned work, then orderly child shutdown with bounded escalation. Preserve history/worktrees and report uncertainty.
- Do not kill unrelated processes. Audit detached subprocess/provider ownership; “GUI stopped” cannot guarantee an external service rolled back a job.
- Restart restores saved state without sending pending prompts or resuming advisor work automatically.

### Coordinator evidence and approvals

Provide focused capabilities such as list project threads, read bounded thread activity, inspect reported child runs and propose actions. Include observation timestamps/source IDs, and describe stale/partial activity honestly. Avoid continuously feeding all transcripts to the model.

Treat transcripts/tool output as evidence, not application authority. A malicious thread cannot grant the advisor execution rights.

A proposal specifies exact project, target thread, requested message/delivery mode or creation directory/base, and action. Approval is enforced by application code, bound to that proposal and target state, not inferred from model text. Reject stale/replayed approval and retain an action record.

Durable task replay cannot guarantee exactly-once effects in an external Pi process. Use stable application action IDs and durable dispatch/outcome records. If a crash occurs between dispatch and recording acceptance, expose an uncertain outcome instead of blindly resending. Unexecuted approvals should not become automatic actions on restart.

## Feasibility gates

1. Confirm installed Pi/version protocol and supported resources with a controlled local harness. Pin/document compatible versions for development; don't depend on unreviewed moving protocol shapes.
2. Native Q+A: installed `~/.pi/agent/extensions/qna-ui.ts` explicitly rejects non-TUI mode and uses `ctx.ui.custom()`. Build a project-owned compatibility path; do not silently modify globally installed extensions.
3. Verify pi-subagents launching, observation, child history, cancellation, detached work and shutdown. Prefer its structured lifecycle/artifact interfaces, not terminal-output scraping. Full interactive control of child runs is not yet promised.
4. Verify pi-web-access commands/tools and any interactive workflow requirements. Core web capability and terminal/browser curator UI are different acceptance scopes.
5. Validate extension/resource discovery in worktrees, project trust and working-directory behavior. Creating an isolated checkout must not inadvertently run untrusted project startup code.
6. Establish how saved-session ownership and GUI profile ownership are enforced. Durable local storage has no built-in cross-process locking.
7. Verify reuse of relevant CLI-configured providers/credentials in Durable without logging credentials or inventing a second auth store. CLI OAuth/settings compatibility is not assumed.
8. Pin a Durable version and verify Node/runtime/storage requirements and orderly close/reopen behavior. Its API is experimental.

These are integration investigations, not permission to use real credentials, launch the native GUI or run live providers.

## Delivery milestones

### 0. Compatibility proof

Complete: executable probes and all eight reconciled feasibility gates are recorded in [PI-GUI-COMPATIBILITY.md](PI-GUI-COMPATIBILITY.md), including the user-authorized pinned Durable artifact. Combined verification passes 65 compatibility tests and 20 Rust tests, formatting, strict clippy and build. This is compatibility proof, not native GUI/coordinator acceptance; unsupported ownership/workflow selections must be refused, and production enforcement remains milestones 1–2.

Controlled tests for RPC lifecycle, Q+A bridge, nested subagent observation/shutdown, web capability and configuration/worktree discovery. Record exact supported/unsupported behaviors. Assess Durable provider integration without blocking basic GUI implementation unnecessarily.

### 1. Usable multi-thread GUI

Replace demo catalog with project/thread navigation and native conversation controls. Support concurrent isolated threads, saved-session opening, worktree lifecycle/status, attention badges/ding, nested subagent inspection, explicit queue/stop behavior and safe exit/reopen. No coordinator, branching, remote execution or Git integration UI required.

### 2. Project-scoped Durable advisor

Local persisted advisor, bounded thread evidence, source-linked explanations, exact action proposals, explicit approval, action uncertainty and safe restart. It may fail/unavailable without preventing ordinary thread use. No active autonomous supervision.

### Deferred

Full session-tree navigation/branching, live TUI attachment, embedded terminal fallback, split panes/multiple windows, cross-platform distribution, remote/background execution, sandboxing, global coordinator, unattended supervision, automatic Git integration/cleanup, arbitrary TUI extension compatibility.

## Acceptance and verification

Core journey: create several editing threads in independent worktrees; run them concurrently; switch views without interrupting work; receive an in-app badge/ding for a hidden thread's Q+A; answer it; inspect child activity and web output; inspect Git status; quit with confirmation; reopen saved history without replaying interrupted work.

Advisor journey: ask what project threads are doing; inspect cited/timestamped evidence; receive a proposed new-thread/message/stop action; reject it with no side effect; approve a current proposal exactly once; handle restart/ambiguous dispatch without duplicate action.

Additional gates: invalid/malformed protocol, startup failure, provider error, queue timeout, process crash, huge tool output, stale callbacks, slow UI consumer, dirty/non-Git project, worktree creation failure, session ownership conflict, storage failure, muted/repeated alerts, canceled/timed-out dialogs and dropping/recreating views.

All Rust suites live in owner-local `tests/<subject>.rs`, declared by the production owner. Controlled adapters must exercise production workflows rather than duplicate them. Sidecar tests use fake models/services and temporary storage; include them in repository validation explicitly. Run `./scripts/check.sh` after implementation changes.

Native GUI launch/automation, real credentials/keyring access and live-provider trials require separate explicit consent. Automated tests use fake requests/time, temporary files and disposable loopback listeners only. Licensing/distribution remain deferred; the starter is unlicensed.

## Sources consulted

- Repository `AGENTS.md`, `llm-docs/{OVERVIEW,ARCHITECTURE,EXTENDING,VERIFY}.md`, streaming/owned-backend recipes.
- Installed Pi README and `docs/{rpc,rpc-commands,rpc-extension-ui,sdk}.md`.
- Installed Q+A extension's mode guard/custom-UI implementation.
- Upstream Pi Durable README: https://github.com/earendil-works/pi/blob/main/packages/durable/README.md (experimental, current upstream; not proof of installed capability).
