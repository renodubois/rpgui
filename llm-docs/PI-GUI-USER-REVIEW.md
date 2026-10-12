# Milestone 1: user acceptance review

Code review and automated checks are complete for the admitted Linux baseline. Your remaining decision is whether the native workflows are usable and trustworthy for your work. Automated passing tests are not sign-off for the unverified native journeys below.

The compact developer-workspace refresh has automated interaction/layout coverage but has not been launched natively. Review its new hierarchy, icon tooltips, menus, compact typography and small-window behavior; earlier native screenshots are not visual acceptance of this refresh.

## Start safely

- Use an unlocked graphical session and a disposable project/profile first. Human development uses `./dev.sh`; its GUI metadata is worktree-local under `.env.dev-config/rpgui`.
- **That does not isolate Pi configuration or credentials.** Choose a separate `PI_CODING_AGENT_DIR` with deliberate test configuration before launch. Do not copy another worktree's GUI profile. Ordinary configured providers can make paid/live requests when you send.
- Required baseline: Linux with `/proc`, subreaper/pidfd support, Node v24.13.0 and the reviewed Pi 1.1.0/package fingerprints. Set `PI_GUI_PI_PACKAGE` to its package directory if discovery cannot find it.
- Unsupported enabled extensions/providers/workflows must show an explicit admission refusal. The GUI must not silently disable them or edit your ordinary configuration. Unrestricted CLI parity is not promised.
- Use in-app confirmed exit before stopping the development watcher. Source rebuilds and Ctrl+C terminate the frontend; retained owners reconcile shutdown separately. Do not edit runtime/package sources while a runtime is active.

Example using directories you prepared (not an automated fixture setup):

```sh
PI_CODING_AGENT_DIR=/absolute/path/to/isolated-agent \
PI_GUI_PI_PACKAGE=/absolute/path/to/reviewed-pi-package \
./dev.sh
```

## Main acceptance journey

1. **Project/worktree creation.** Click **Add project…** and choose a disposable Git project with a committed file and an uncommitted edit using the system folder picker. Check cancellation adds nothing; Linux requires a desktop file-chooser portal, and unavailable-picker errors should be visible. The new native picker has headless control coverage but still needs your desktop review. Create two isolated threads using a full committed object ID. Verify distinct working directories, committed content only, and no commit/stash/dirty copying in the original project. Explicit shared-directory creation should be clearly labelled; it is not isolation or sandboxing.
2. **Concurrent conversation usability.** Explicitly open both owned threads, send different tasks and switch repeatedly. Both should continue; selection must not interrupt work or mix text, images, dialogs or settings. Review Markdown/code, tool/thinking inspection, history scrolling, resize and keyboard navigation. Optionally scale up to eight live threads.
3. **Hidden attention.** Ask one thread for a standard question while another is selected. Verify the correct thread badge/dialog appears without selection/focus theft; answer and cancel questions. Check one transition ding, no repetitive ding and persisted mute. Audio failure should be visible, not claimed successful. Audible audio, IME and accessibility need your own assessment.
4. **Message controls.** While a generation is busy, try steering and an explicit follow-up. Check queue visibility, then **Stop** (clears queued messages before aborting). Try an image and model/thinking selection, inspect usage/context and run manual compaction when idle. Confirm failures preserve drafts/uncertainty rather than silently resending. The isolated-worktree and native compaction journeys were not previously verified natively.
5. **Child/web inspection.** With admitted configurations, run a local nested child and core HTTP search/fetch. Inspect hierarchy, current/stale/partial activity and retained output. Close/stop the owning thread and verify the intended result. Native child/web inspector usability remains a priority for your review; remote/browser/account workflows are outside this baseline.
6. **Git/history preservation.** Make a harmless file change through an isolated thread, inspect Git status, then use the thread-action (…) menu to rename, close the runtime and archive. Worktree files and Pi history must remain. Unknown external session files are passive read-only snapshots, not writable adoption. There is no automatic integration or deletion.
7. **Exit/restart.** Quit with active and queued work. Verify confirmation, bounded shutdown and explicit acknowledgement if outcomes are uncertain. Restart: saved history should appear passively, with no prompt resend or provider request until an intentional action. Open an owned runtime explicitly to continue. Use this normal journey before considering any deliberate crash drill.

## Report back

For each item: pass/fail, expected versus actual behavior, thread/attempt identity, window size and whether the runtime configuration was admitted. Avoid credentials or private transcript contents in screenshots/logs.

Particularly useful feedback: confusing labels/disabled states, unreadable panels, scrolling/focus surprises, responsiveness under large output, audio/IME/accessibility, isolated worktree creation, compaction and child/web inspection.

Known limits: Linux only; one image up to 1 MiB per prompt; bounded live frames/presentation and passive history; no coordinator, branching, terminal attachment, remote execution or Git integration UI. External effects cannot be rolled back by local shutdown. Previously poisoned profiles with incomplete attempt evidence are not automatically repaired; do not delete ownership evidence merely to bypass admission—inspect the failure first.

If these journeys pass and the documented limits are acceptable, milestone 1 is ready for your qualified user acceptance. No new native GUI/provider/keyring trial was performed during this code-review pass.
