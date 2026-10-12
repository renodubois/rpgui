# Milestone 1 acceptance ledger

Status: production implementation delivered for the admitted Linux baseline; acceptance is qualified by the explicit native and compatibility limits below. This is not a claim of unrestricted Pi extension parity or exhaustive native acceptance. The catalog is no longer a production route.

Native testing was explicitly authorized and performed only in an unlocked, owned Xvfb session with disposable HOME/config/runtime/agent directories, private D-Bus and synthetic loopback data. No ordinary profiles, real provider credentials, keyrings or desktop sessions were borrowed.

## Executed verification

- `./scripts/check.sh`: passed default production tests, formatting, strict locked Clippy, owner-local Rust tests and locked build. Latest full run includes 4 default Node tests and 41 Rust tests.
- `PI_GUI_CHECK_COMPAT=1 ./scripts/check.sh`: passed those same default checks, 65 pinned compatibility tests and 7 production launcher integration tests. Latest post-review output: `/tmp/rpgui-m1-review-final.log`. `git diff --check` and `bash -n dev.sh` also passed.
- Production integration executes the actual launcher, inherited guard, ordinary Pi RPC and retained native Linux helper, not a replacement agent runtime. It covers concurrent sessions/Q+A/settings/history; admitted child observations/scoped stop; persisted search and HTTP fetch output; genuine frontend SIGKILL with detached nested work; eight busy threads and explicit queues/abort; daemonized Bash descendants; and cleanup despite durable phase-write failure.
- Owner-local suites cover transport framing/correlation/deadlines/EOF/backpressure/sticky failure, stale callbacks, metadata intent merging/durability, stable locks/private namespaces, session/cwd validation, passive history/image limits, image cancellation/next-draft receipt races, worktree conflicts/hooks/filters/fsmonitor and retained native control/view recreation.

Counts are separate evidence classes. Compatibility success is not substituted for production/native evidence.

## Final code-review reconciliation

A read-only independent reviewer inspected Rust thread/workspace/view workflows and reviewed the fixes; parent inspection covered transport, runtime/storage/process mechanics, Git and development shutdown. Six concrete findings were fixed:

1. Deterministic package/agent/runtime-hook failures previously allocated incomplete attempts and poisoned future admission. Resolve paths and prepare the launch before allocation; unused request-write reservations are removed, while potentially post-spawn errors retain uncertainty.
2. Closing a disconnected thread previously erased uncertain/passive/failed state. It is now a no-op without an owned client.
3. Provider failure previously masked later terminal owner failure. Terminal faults now preserve interruption/uncertainty and both diagnostics; explicit pre-start admission refusal remains distinct from ambiguous dispatch.
4. Owner-evidence FIFOs could hang the ordered worker. Evidence is opened nonblocking/no-follow and must be a regular file.
5. Git status could execute configured clean/process filters before project trust. Status and checkout now share invocation-local filter suppression, with dirty-file regression coverage.
6. `dev.sh` restart/cleanup group signals could kill retained guardians with the frontend. Client termination now targets only the frontend PID; watcher/build group cleanup remains separate. A disposable-process test exercises the shipped helper without a GUI.

Regression tests and full verification passed. Independent follow-up found no remaining concrete blocker in the fixes' blast radius. Request-write cleanup/post-spawn setup faults and TERM-resistant development frontend escalation lack dedicated regression tests; existing production ownership/fault integration remains separate evidence. Old incomplete attempts are not automatically repaired and must not be blindly deleted to bypass admission. Changes remain unstaged/uncommitted; no new native GUI, real-provider or credential trial was performed.

User acceptance remains qualified and pending the hands-on checklist in [PI-GUI-USER-REVIEW.md](PI-GUI-USER-REVIEW.md), especially native audio/IME/accessibility, isolated worktrees, compaction and child/web inspector usability.

## Project folder-picker follow-up

`Add project…` now invokes GPUI's system single-directory picker; no typed project path is required. The remaining path field is explicitly for passive saved-session inspection. Selection is revalidated by the workspace owner; cancellation is a no-op, duplicate/file selections remain refused, pending prompts prevent duplicate launches, and replies after exit begins are ignored. Picker failure is visibly reported with Linux desktop portal guidance. No new dependency or runtime workflow was added.

`./scripts/check.sh` passed 4 default Node tests, formatting, strict Clippy, 42 Rust tests and locked build; log: `/tmp/rpgui-project-picker-check.log`. A headless UI test checks folder-only/single-selection options, cancel, spaces/Unicode, duplicate and invalid selections, and late exit response. `git diff --check` passed. Native desktop picker appearance/portal availability was not tested; user review remains required. Prior compatibility/integration evidence above was not rerun for this presentation-only follow-up.

## Compact developer-workspace refresh

User-selected direction: compact developer workspace, with sharper visual hierarchy rather than a wall of equal-weight text buttons. Presentation now uses layered charcoal surfaces, compact typography, accented thread selection, labelled/icon-coded phases, collapsible project groups with attention counts, a small conversation toolbar and on-demand thread actions. Conversation/Activity tabs, details/model/image/queue panels, usage status and the unified composer keep secondary controls out of the default conversation path. Rename is prefilled and bound to its original thread even if selection changes; saved-session inspection is an explicit read-only form. Ctrl+Enter sends without inserting a newline; Enter remains multiline editing. Upward scrolling pauses auto-follow. Workflow/runtime/storage owners and transport protocols are unchanged.

Executed after the refresh:
- `./scripts/check.sh`: 4 default Node tests, formatting, strict locked Clippy, 51 Rust tests and locked build passed; `/tmp/rpgui-refresh-check.log`.
- `PI_GUI_CHECK_COMPAT=1 ./scripts/check.sh`: those checks plus 65 pinned compatibility tests and 7 production integration tests passed; `/tmp/rpgui-refresh-compat-check.log`.
- Headless controls cover menu-to-rename routing across selection changes, progressive panels and explicit delivery state, preserved per-thread drafts, collapsed navigation, saved-form cancellation/exit gating, passive-send rejection, Ctrl+Enter versus Enter, upward-scroll pause/resume and baseline composer/viewport layout at 1200×840, 900×700 and 680×520. Existing hidden-dialog and create/picker/view-recreation coverage remains passing. Theme tests check distinct surfaces, density and color-token synchronization.

No native GUI was launched and no new screenshots, real credentials/keyrings or live providers were used. Headless layout checks do not establish rendered-pixel quality, IME/accessibility or exhaustive crowded-panel/live-runtime resizing. Prior native screenshots describe the pre-refresh presentation; the new visual hierarchy remains pending native user review. No dependency, runtime admission or preservation policy changed.

## Requirement-by-requirement reconciliation

| Requirement / gate | Production implementation and executed evidence | Acceptance reconciliation |
|---|---|---|
| Projects and 2–8 top-level threads | `workspace`, `threads`, retained `AppShell`; actual eight-thread production integration and native two-thread drill | Implemented; eight-thread automation and two-thread native evidence |
| Sidebar, one retained conversation pane | Native workspace controls retain owners across selection and view recreation; headless control suite and hidden native Q+A | Implemented; no selection theft from background question |
| Markdown/code streaming and inspection | Kit Markdown/code rendering, bounded transcript/tool/thinking projections, passive Pi JSONL authority | Implemented; native Markdown/code visible; exhaustive large native output unverified |
| Text/images | Explicit dispatch/receipt state and capability check; one bounded PNG/JPEG/GIF/WebP; draft-local read/cancellation revisions | Implemented; native PNG actually reached synthetic API; cancellation/receipt races covered locally |
| Models/thinking and discovery | Public ordinary RPC queries, typed selection intentions, declared available levels/commands | Implemented; production RPC tests and native synthetic model/off selection; no live account claim |
| Compaction, usage/context | Typed compaction and ordinary Pi usage/context queries, displayed estimates | Implemented; production RPC evidence; native controls/usage displayed; native compaction journey unverified |
| Create/open/rename/archive | Reserved creation, passive opening, ordered private metadata writes, archive preserves files/worktrees | Implemented and owner-local/production tested; no automatic deletion |
| Steering/follow-up and visible queues | Explicit delivery, one pending admission receipt, ordinary queue snapshots; clear then abort/close | Implemented; eight busy-thread integration and native queued follow-up; acceptance is not settlement |
| Standard dialogs and hidden Q+A | Thread/attempt scoped requests, confirm/select/input/editor, cancel/timeout/stale retirement | Implemented; actual native hidden question answered without selecting its thread; other dialog variants automated |
| Attention and muteable transition ding | Retained phase attention, transition-only notifications, persisted mute and bounded owned player | Implemented; hidden attention visible and sound failure surfaced; audible native ding remains unverified |
| Nested child hierarchy/activity/tokens/history | Required inherited guard, public launch preflight, bounded versioned public observations and scoped inspection/stop | Implemented; actual admitted production nested work and frontend-loss cleanup tested; native nested inspector UX unverified |
| Web workflows and persisted output | Actual fetchRouting.providers schema, HTTP-only routing, explicit remote-hosted false, core SearXNG selection, guarded tool overrides; Pi-persisted search/fetch results | Implemented for admitted core; real package integration persists and passively reopens output; other providers/workflows explicitly refused, native inspector UX unverified |
| Isolated committed worktree/shared directory | Explicit full committed object or explicit shared directory; no dirty copying, safe Git invocation/status/errors | Implemented; Git tests prove committed content and preservation; native shared policy visible; isolated native journey unverified |
| Saved session/profile safety | Stable canonical kernel lock, private namespace, owned session/cwd checks; unknown history is read-only with actual saved cwd | Implemented; local tests and actual native duplicate-profile refusal/external snapshot; no writable adoption or terminal handoff |
| Confirmed exit and passive reopening | Retained closure, accepted-save drain, explicit failed-durability/unknown-outcome acknowledgement; no replay | Implemented; actual native active-work confirmation, durable stop and passive restart; frontend SIGKILL production test |
| Gate 1: transport | Nonblocking bounded pipes, continuous diagnostics drain, correlated IDs/deadlines, stale-attempt rejection and sticky failure | Passed owner-local real-pipe tests; no automatic ambiguous mutation retry |
| Gate 2: Q+A replacement before startup | Reviewed offline inventory and pinned GUI replacement; unsupported owners refuse before extension startup | Passed production and compatibility evidence; no global configuration edits |
| Gate 3: retained ownership/guardian | Durable process identities, public whole-run writer proof plus native subreaper/pidfd closure; live transport leases, not mere frontend PID, authorize active scopes | Passed nested detached/SIGKILL/daemonized Bash and phase-write-failure integration; missing proof remains uncertainty |
| Gate 4: web classification | Strict selected config/overrides and complete own web-dist fingerprint; unsupported workflow never silently disabled | Passed selected refusal units and actual persisted core HTTP integration |
| Gate 5: ordinary selected resources | Public offline discovery, pinned own runtime modules, explicit audited selection; cached unselected drift is not an owner | Passed default admission and pinned matrix; unresolved selected owners are refused |
| Gate 6: profile/session writes | Stable lock lifetime, no-follow/nonregular checks, bounded private metadata/history/image reads, session/cwd ownership validation | Passed storage and native startup refusal; history remains Pi authority, not a second GUI transcript |
| Failure acceptance | Crash/ambiguous receipt/huge output/slow drain/storage/worktree conflict/shutdown tested within owner suites and production integration | Automated evidence passed; exhaustive native failure and renderer responsiveness journeys remain unverified |

## Native drill evidence

Disposable root: `/tmp/rpgui-native-B5HPfl` (pointer `/tmp/rpgui-native-root`). Initial and final profiles were disposable and separate. Screenshot artifacts include `hidden-qna.png`, `markdown.png`, `queued.png`, `exit-confirm.png`, `image-panel.png`, `passive-final.png`, `resize-final.png`, `profile-conflict.png`, `settings-final.png`, `reading-history-final.png` and `external-readonly-final.png`.

Observed: actual synthetic dispatch; two simultaneous threads; a hidden standard Q+A answered without selection theft; a held generation with explicit queued follow-up that was not dispatched before stop; Markdown/code; PNG forwarding; active-work exit confirmation; positive retained descendant closure with parent code 0 and uncertain false; passive restart without another API request; 900×700 resize; current-build startup/profile conflict refusal; synthetic model/thinking controls and reading-history toggle; an unknown saved session shown read-only with its saved cwd and no adoption/replay. The request ledger contains five intentional synthetic model requests. Query-only reopening/settings/snapshot inspection did not add requests.

An empty-path snapshot attempt exposed a stale read-error banner; matching history-read errors now retire on successful current-ticket snapshot completion. Sound was unavailable and visibly reported; audibility was not established.

## Remaining limits and risks

- Currently requires Linux `/proc`, subreaper and pidfd APIs. Startup probes required pidfd operations. Cross-platform distribution is not established.
- Native audible ding, real provider/keyring use, IME/accessibility, nested-child/web-inspector UX, isolated-worktree/compaction journeys and exhaustive fault/large-output/resizing coverage remain unverified. No real-provider consent is inferred from isolated native consent.
- Admission is deliberately narrower than unrestricted CLI parity: unsupported selected MCP/llama.cpp/codemode model helpers, remote/browser/account/curator/clone/media workflows, arbitrary custom TUI and unaudited autonomous owners refuse before startup. No silent configuration filtering.
- Hard budgets are visible safety limits, not unlimited history guarantees: 2 MiB RPC frames, bounded mailboxes/presentation, 1 MiB text/image/metadata admission, one image per prompt, 32 MiB passive history consumption and bounded child snapshots/process closure. Large whole-session RPC replies can exceed the frame limit and refuse live admission even when passive history is readable; omissions/refusals are not completion proof.
- Kernel closure is distinct from package whole-run writer-close proof. Deadline, storage or missing-proof failures preserve uncertainty and block new admission; acknowledgement never replays a prompt or establishes successful side effects.
- This is not a sandbox or rollback of HTTP/provider/external side effects. Reviewed installations should not be mutated while runtimes are active; fingerprints are startup checks, not OS-level protection against hostile same-user filesystem changes. Do not attach a noncooperating terminal writer to a GUI-owned session.
- No automatic worktree cleanup/integration, dirty copying, branching, terminal attachment, remote execution, Durable advisor/coordinator or arbitrary custom TUI is included.

The final isolated GUI closed normally. Three final-profile guardian proofs each recorded parent code 0, descendant uncertainty false and overall uncertainty false. Exact owned server/Xvfb PID files were validated against their disposable script/log paths and stopped using pidfds; no unrelated processes were targeted. The five-request invariant remained unchanged.

Changes remain unstaged/uncommitted. Human README files were not edited.

## Exact changed paths

Tracked deletions below remove the disconnected catalog runtime/views/tests; remaining paths are modifications or new milestone production owners/mechanics/tests/documentation.

- `Cargo.lock`
- `Cargo.toml`
- `dev.sh`
- `extensions/pi-gui-guard.ts`
- `llm-docs/ARCHITECTURE.md`
- `llm-docs/EXTENDING.md`
- `llm-docs/OVERVIEW.md`
- `llm-docs/PI-GUI-MILESTONE-1.md`
- `llm-docs/PI-GUI-USER-REVIEW.md`
- `llm-docs/PI-GUI-PLAN.md`
- `llm-docs/VERIFY.md`
- `scripts/check.sh`
- `scripts/pi-runtime/admission.mjs`
- `scripts/pi-runtime/build-lock.json`
- `scripts/pi-runtime/build-lock.mjs`
- `scripts/pi-runtime/catalog.mjs`
- `scripts/pi-runtime/extension-lock.json`
- `scripts/pi-runtime/guardian.mjs`
- `scripts/pi-runtime/launch.mjs`
- `scripts/pi-runtime/process-scope.mjs`
- `scripts/pi-runtime/scope-stop.mjs`
- `scripts/pi-runtime/tests/catalog.test.mjs`
- `scripts/pi-runtime/tests/dev.test.mjs`
- `scripts/pi-runtime/tests/integration/frontend.mjs`
- `scripts/pi-runtime/tests/integration/launch.test.mjs`
- `src/catalog/mod.rs`
- `src/catalog/state.rs`
- `src/catalog/tests/coordinator.rs`
- `src/catalog/tests/state.rs`
- `src/connectors/catalog/error.rs`
- `src/connectors/catalog/http.rs`
- `src/connectors/catalog/memory.rs`
- `src/connectors/catalog/mod.rs`
- `src/connectors/catalog/tests/binding.rs`
- `src/connectors/catalog/tests/http.rs`
- `src/connectors/catalog/types.rs`
- `src/connectors/git/mod.rs`
- `src/connectors/git/tests/worktrees.rs`
- `src/connectors/mod.rs`
- `src/connectors/notification.rs`
- `src/connectors/pi/mod.rs`
- `src/connectors/pi/tests/process.rs`
- `src/connectors/pi/tests/transport.rs`
- `src/connectors/process_scope.rs`
- `src/connectors/tests/notification.rs`
- `src/main.rs`
- `src/runtime.rs`
- `src/storage/history.rs`
- `src/storage/images.rs`
- `src/storage/mod.rs`
- `src/storage/owners.rs`
- `src/storage/preferences.rs`
- `src/storage/tests/history.rs`
- `src/storage/tests/images.rs`
- `src/storage/tests/owners.rs`
- `src/storage/tests/preferences.rs`
- `src/storage/tests/workspace.rs`
- `src/storage/workspace.rs`
- `src/test_support/catalog.rs`
- `src/test_support/http.rs`
- `src/test_support/mod.rs`
- `src/threads/mod.rs`
- `src/threads/tests/lifecycle.rs`
- `src/views/app_shell.rs`
- `src/views/catalog/mod.rs`
- `src/views/catalog/record_detail.rs`
- `src/views/catalog/record_list.rs`
- `src/views/catalog/search.rs`
- `src/views/catalog/tests/interaction.rs`
- `src/views/mod.rs`
- `src/views/tests/app_shell.rs`
- `src/views/tests/composition.rs`
- `src/views/tests/journeys.rs`
- `src/views/tests/mod.rs`
- `src/views/tests/workspace.rs`
- `src/views/workspace.rs`
- `src/workspace/mod.rs`
- `src/workspace/tests/navigation.rs`
- `src/worktrees/mod.rs`

