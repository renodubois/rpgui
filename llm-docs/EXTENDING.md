# Extending the native Pi GUI

Keep workflow ownership above replaceable presentation; do not introduce a universal connector, product event bus, global dialog registry or catch-all app state.

| Change | Owner / placement |
| --- | --- |
| Native control or local panel | `src/views/workspace.rs`, or an actually needed owner-private panel |
| Project/thread navigation, persistence policy, exit | `src/workspace/` |
| Pi thread intention, queue/dialog/generation transitions | `src/threads/` |
| RPC framing, correlation, transport budgets | `src/connectors/pi/` |
| Retained local descendant mechanics | `src/connectors/process_scope.rs`, `scripts/pi-runtime/process-scope.mjs` |
| Git command mechanics / worktree policy | `src/connectors/git/` / `src/worktrees/` |
| Files, process identity, passive history, image mechanics | `src/storage/` |
| Build/resource admission and ownership contracts | `scripts/pi-runtime/`, `extensions/pi-gui-guard.ts` |
| Theme/assets/window dependency construction | `src/theme.rs`, `src/main.rs` |

Views submit semantic owner intentions, including typed model/thinking/compaction actions. Standard dialog values remain scoped to thread, attempt and request ID; they are not global product-dialog events. Feature owners decide admission, acknowledgement, retirement and uncertainty. Connectors own wire/process mechanics; storage owns file mechanics. The shell retains owners and delivers opaque results.

Before admitting another extension/provider workflow:
- Review the pinned public protocol/source and all autonomous/background ownership seams.
- Classify its ordinary selected configuration before startup; refuse unknown owners without silent disabled-resource filtering.
- Prove scoped shutdown and authoritative history. Runner PID death alone is not writer-close proof; bounded status projections are not completion.
- Use public launch/preflight/required-extension APIs. Never copy private intercom targets, inbox routes or control metadata into native authority.
- Reject stale attempts/replies and never retry an ambiguous mutation. Keep unsupported selections explicit and update compatibility evidence.

Tests belong to their owning Rust module's `tests/<subject>.rs`, declared by that production module. Use fake outcomes/time, temporary files and disposable loopback services. Real-process production tests must execute the shipped launcher/guard, not duplicate its workflow in fixtures. Update `scripts/check.sh` integration coverage and the acceptance ledger. Run both default and explicit compatibility checks.

Keep generated docs under `llm-docs/`; human README files are read-only. Consult `recipes/STREAMING.md`, `recipes/OWNED-BACKEND.md` and `recipes/AUTHENTICATION.md` for underlying ownership/safety conventions. No real keyring/provider tests, native launch or desktop automation without separate explicit consent.
