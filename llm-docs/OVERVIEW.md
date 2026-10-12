# Native Pi GUI

Milestone 1 replaces the catalog demo with a retained multi-thread native Pi workspace. One project/thread sidebar selects one conversation pane; selecting or recreating that pane does not stop another thread. Pi remains the execution and history authority.

Read `ARCHITECTURE.md`, `EXTENDING.md`, `VERIFY.md`, the recipes, `PI-GUI-PLAN.md` and `PI-GUI-COMPATIBILITY.md` before changes. Actual reconciliation is in `PI-GUI-MILESTONE-1.md`; the remaining hands-on acceptance checklist is in `PI-GUI-USER-REVIEW.md`.

## Runtime

Rust 1.97.0 and GPUI Kit 0.7.1 are pinned. Reviewed runtime: Node v24.13.0, Pi 1.1.0 and the source/build fingerprints in `scripts/pi-runtime/build-lock.json`. Runtime mechanics currently require Linux `/proc`, subreaper and pidfd support. Other platforms are not accepted.

Ordinary Pi model/auth/settings are reused from `PI_CODING_AGENT_DIR` or the ordinary agent directory. GUI metadata lives under `$XDG_CONFIG_HOME/rpgui` (or `$HOME/.config/rpgui`), with stable canonical kernel locking. The GUI never edits global Pi configuration or stores credentials. History reopens passively; writable runtime opening is explicit.

Admission is deliberately conservative. Enabled unaudited owners are errors, not hidden configuration filtering. In particular MCP, llama.cpp and codemode's model helpers are not admitted. A user must explicitly disable unsupported selections in their chosen ordinary configuration; the GUI does not do so. Supported subagents must use fresh local ordinary Pi children, audited explicit extensions and supported public ownership APIs. Web admission supports reviewed HTTP search/raw/readable extraction and persisted output, not browser/cookies/accounts, curator/terminal or Git clone workflows.

## Workflow

1. Click **Add project…** and select an existing directory with the system folder picker (Linux desktop file-chooser portal required).
2. Use the project's **New thread** (+) control. The dialog suggests a name and asynchronously resolves the current full commit ID for an isolated worktree; edit those values or explicitly choose shared directory, then submit. Isolated worktrees exclude uncommitted changes. Missing commits/non-Git projects require an explicit shared choice or a valid full commit ID; cancellation creates nothing.
3. Use **Open runtime** to open its owned runtime explicitly. Edit text or stage one PNG/JPEG/GIF/WebP image of at most 1 MiB.
4. Send with explicit steering/follow-up mode; inspect queues, model/thinking, Markdown, thinking/tools/commands, child activity/history, usage/context or Git status.
5. Hidden-thread standard dialogs remain routed by thread/attempt without changing selection. Attention transitions may ding; mute is persisted and unavailable audio is reported.
6. Rename/archive preserve history/worktrees. Exit confirmation clears/aborts admitted work and retains uncertain shutdown outcomes. Restart never replays an interrupted prompt.

Unknown/external sessions can be inspected passively but are not adopted for writing. There is no terminal attachment, branching, automatic Git integration/cleanup, remote execution, global coordinator or arbitrary custom-TUI fallback.

## Compact workspace presentation

The dark developer workspace uses a fixed project/thread sidebar, flat selectable thread rows, readable status labels/icons and a distinct selected-row accent. Project groups collapse without changing the conversation; attention counts remain visible on collapsed groups. The conversation header keeps runtime opening, Git status, details and a thread-action menu separate from messaging. Rename, compaction, resource trust, runtime closing and archive are on demand; saved-session inspection opens a read-only form rather than leaving path/name fields in navigation.

Conversation and Activity are separate tabs. Directory/trust/runtime identity, model/thinking choices, attachments and queue contents expand only when requested; usage/context stays in a compact status strip. The framed composer keeps explicit steering/follow-up choices and one primary Send action; Stop clears queued work before aborting. Ctrl+Enter sends, ordinary Enter edits multiline text, scrolling upward pauses auto-follow, and Live resumes it. Sending is disabled for passive runtimes, image reads, unknown admission and exit; owners still enforce all admission/receipt rules. Switching threads restores its retained draft and resets local panels without affecting execution.

This refresh has headless interaction/layout coverage, not native screenshot or accessibility acceptance. See the refresh entry in `PI-GUI-MILESTONE-1.md` and the pending user review.

## Verification and safety

Run `./scripts/check.sh` and, with the reviewed local Pi packages installed, `PI_GUI_CHECK_COMPAT=1 ./scripts/check.sh`. Builds/tests are not native GUI launches. Automated fixtures use disposable profiles, synthetic credentials, fake models and owned loopback services only.

Native automation needs explicit consent and an unlocked isolated graphical session; never unlock or use ordinary profiles/keyrings/providers for drills. The implementation's consented Xvfb journey and unverified audio/IME/accessibility/live-provider cases are recorded separately in the acceptance ledger. Human README files remain read-only to agents; generated documentation belongs here. Licensing/distribution remain deferred; do not publish without the owner's decision.
