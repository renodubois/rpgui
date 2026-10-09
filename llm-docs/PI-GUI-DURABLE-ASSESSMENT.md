# Durable compatibility proof (milestone 0)

## Controlled artifact and reproducibility

The user explicitly authorized downloading Durable under `~/programs` and using it for this project. The selected installed root is **`~/programs/pi-gui-durable-1.1.0`**, containing `@earendil-works/pi-durable` **1.1.0**, pi-ai/Chord **1.1.0**, diff **8.0.4** and TypeBox **1.3.27**. Installation used an isolated HOME/XDG/npm configuration/cache, public registry, **disabled install scripts**, and no global changes. The temporary download environment was removed; the authorized package root is retained for this project.

`scripts/pi-compat/durable-package{,-lock}.json` records the installer manifest and exact dependency lock/integrities. `durable-lock.{mjs,json}` checks Node **v24.13.0**, selected versions and a SHA-256 of the complete supplied artifact/dependency tree **before importing any downloaded module**. Relative symlink targets are hashed without following them. Baseline artifact SHA-256: `3c68ad370918a751f726cbf766b046eb450d1535971b3ad89f71a8ac8ac845c5`.

Tests default to that programs root. `PI_GUI_DURABLE_ROOT=/absolute/install/root` selects an equivalent supplied artifact; drift is rejected, not automatically repinned. Compatibility validation never installs packages. A future operator may reproduce the supplied install using the recorded manifest/lock with npm 11.8.0 on the reviewed Linux/Node environment and disabled scripts; inspect any resulting hash difference before acceptance.

The compiled public exports, Harness/Session startup and close, scheduler open/abort/seal, storage adapters, provider/credential interfaces and manifest were inspected before execution. Upstream documentation was read in full as reconnaissance; actual acceptance below uses the downloaded pinned artifact, not mutable `main`.

## Executed evidence — nine actual-Harness probes

`node --test scripts/pi-compat/tests/durable.test.mjs` and the combined `PI_GUI_CHECK_COMPAT=1 ./scripts/check.sh` pass without skips. The combined run passes **65 compatibility tests, 20 Rust tests, formatting, strict clippy and build**.

Every Durable driver runs under disposable HOME/XDG/agent/storage/working roots and an allowlisted environment. Only literal owned loopback API/effect listeners are used. No real credential store, keyring, provider account, browser or GUI is accessed. The Harness is the actual production package; there is no replacement scheduler/executor or package testing module.

### Gate 7: provider/credential reuse

- The installed coding-agent's public **`ModelRuntime` is passed directly as `HarnessOptions.models`**. Disposable ordinary CLI `models.json` supplies the provider, endpoint, model and synthetic API key. Actual API transport produces a Durable answer through the owned listener, not a substitute model/agent workflow.
- The ordinary `ModelRuntime` credential path is reused for a synthetic OAuth provider. An expired stored credential triggers exactly one native refresh, the rotated credential is persisted in the **same CLI auth.json**, and actual transport uses the refreshed value. The OAuth provider's login callback refuses invocation; no login or real-account refresh occurs.
- No credentials are copied to Durable storage or logged. OAuth callbacks are fake provider seams; real-provider OAuth endpoints/subscription semantics are not accepted by this controlled result.
- Durable settings/registry remain separate from CLI discovery. The host must explicitly map relevant settings and register reviewed configured extension providers. Passing `ModelRuntime` does not magically reproduce all CLI extensions/resources.

### Gate 8: Node, storage, ownership and close/reopen

Each SQLite and fsync-enabled JSONL backend executes these four workflows:

1. Invalid owned storage fails **before model admission**, without fallback or replacing the existing file.
2. A normal submission completes; repeating its request ID returns the original submission without another model call. Orderly close and a new-process passive reopen preserve conversation identity/history and make **zero additional model calls**.
3. A retained kernel `flock --no-fork` rejects a concurrent storage owner. Closing an active held model request cancels its owned socket. Reopen remains paused with pending work. Every retained task is marked through public `abortTask` **while scheduling is paused**, then the audited built-in abort handlers are explicitly scheduled. No old model request is replayed; a later deliberately new submission works.
4. A production Durable tool dispatches a preapproved synthetic effect to an independent loopback service. The service records acceptance but withholds its receipt; the owning Node process is actually SIGKILLed. Passive reopen makes no model/effect call. An **explicit** recovery under the default non-replay-safe tool policy produces an interrupted tool result and finishes without resending the external effect. The service's accepted effect remains: this is uncertainty/no duplicate, **not rollback or exactly-once delivery**.

The external tool is a fake service seam, not an implementation of coordinator approvals. It has no shell/filesystem-edit capability. All listeners, sockets and driver processes are drained/closed before disposable storage is removed. The supplied programs artifact is intentionally retained.

## Required production boundaries

- Durable has **no built-in cross-process storage lock**. Retain a canonical stable lock for the entire sidecar/storage lifetime; never unlink it while contenders may exist.
- Passive open/inspect is distinct from submit, wait, resume and conversation abort. Some of the latter start scheduling. Never call them as an assumed harmless reopen operation. The tested abort-mark retirement applies to the audited registry, not unknown migration/abort hooks or remote jobs.
- Close seals admission and joins cooperative invocations. Noncooperative handlers/forced owner death still require bounded escalation and explicit uncertainty; no arbitrary descendant/remote cleanup guarantee is established.
- SQLite WAL uses `synchronous=NORMAL`: newest commits may be lost on host/power failure. JSONL fsync does not create exactly-once external actions.
- Durable request-ID deduplication is internal. External Pi actions need stable application action IDs, explicit state-bound approval and durable dispatch/outcome records; ambiguous dispatch must not automatically resend.
- Watch/backlog, coordinator-specific settings mapping, proposal freshness, approval replay rejection, hostile transcript evidence and real-provider behavior remain coordinator implementation/acceptance work, not hidden passes in milestone 0.

## Outcome

**Feasibility gates 7 and 8 have representative executed proof against a pinned supplied artifact.** No advisor or GUI implementation was started. Durable remains optional for ordinary threads. The source-only assessment was superseded, not silently reclassified as runtime evidence.

## References

- Installed Pi: public `dist/core/model-runtime.d.ts` and pinned runtime in `build-lock.json`.
- Supplied Durable: public `dist/index.js`, `dist/harness/{harness,scheduler}.js`, `dist/session/session.js`, storage adapters and declarations.
- [Upstream documentation](https://github.com/earendil-works/pi/blob/main/packages/durable/README.md), [manifest](https://github.com/earendil-works/pi/blob/main/packages/durable/package.json), [Harness](https://github.com/earendil-works/pi/blob/main/packages/durable/src/harness/harness.ts), [Node SQLite](https://github.com/earendil-works/pi/blob/main/packages/durable/src/storage/sqlite/node.ts): reconnaissance, not executable pins.
