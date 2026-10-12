# Verification and safety

## Automated commands

From the repository root:

```sh
./scripts/check.sh
PI_GUI_CHECK_COMPAT=1 ./scripts/check.sh
```

Default checks run production admission unit tests, formatting, strict locked Clippy, owner-local Rust tests and a locked build. Compatibility mode additionally runs the pinned ordinary-RPC harness and production launcher integration after the native retained helper has been built.

Executed evidence and counts are in `PI-GUI-MILESTONE-1.md`. Production integration covers concurrent ordinary threads, actual hidden Q+A/settings/history, admitted child observations/scoped stop, persisted web output/passive reopening, genuine frontend SIGKILL with nested detached work, eight busy independent threads/queues/abort and daemonized Bash descendant closure and cleanup despite durable phase-write failure. Fixtures use synthetic API keys, owned loopback listeners and temporary HOME/agent/session directories only. No live accounts, ordinary profiles or real keyrings are required.

Rust suites are owner-local. Real pipes exercise bounds, incomplete/oversized frames, correlation, EOF ordering, sticky failure, huge diagnostics and ambiguous dispatch without retries. Storage/worktree tests cover conflicts, invalid formats, concurrent metadata intent preservation, private session paths, interrupted ownership, history/image bounds and checkout hooks/filters/fsmonitor refusal. A headless GPUI control test exercises hidden dialog routing and view recreation while retaining thread ownership.

## Native consent and isolation

Native launch/automation and real credential/keyring/provider access need separate explicit consent. Never unlock a desktop. Use an already unlocked isolated graphical session, fresh HOME/config/runtime/agent directories, private D-Bus session and fake provider/local-loopback data. XDG_CONFIG_HOME alone does not isolate shared credentials. Stop only exact owned processes.

The consented implementation drill used a fresh owned Xvfb server, not the ordinary desktop. It verified rendering and actual synthetic model dispatch, simultaneous hidden Q+A without selection theft, queued follow-up visibility, Markdown/code, native PNG forwarding, active-work exit confirmation, durable retained stop passive restart without another provider request, 900×700 resize, duplicate-profile startup refusal, synthetic model/thinking controls, reading-history mode and unknown saved-history read-only inspection. Audio was unavailable and visibly reported; no audible-success claim is made.

Still separate/unverified: audible native ding, real provider/keyring use, IME/accessibility, native nested-child/web-inspector UX, cross-platform distribution and exhaustive native large-output/fault and exhaustive size journeys. Automated implementation evidence must not be presented as proof of those native behaviors.

## Safety and residual scope

History is Pi JSONL authority, never a GUI transcript store. Refuse unknown writable session handoff and unproved prior owner cleanup. Preserve worktrees/history and uncertainty; never replay prompts, retry ambiguous mutations, unlock profiles manually, terminate unrelated processes, or perform Git cleanup/integration.

Linux descendant closure is kernel process closure, not rollback of external provider/HTTP side effects and not a sandbox for arbitrary shell programs. Unsupported configured autonomous owners/workflows fail explicitly before startup. No staging, commits or publishing are part of validation.
