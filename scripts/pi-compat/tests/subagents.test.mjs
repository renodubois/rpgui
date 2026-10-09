import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import { mkdir, readFile, writeFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { homedir } from 'node:os';
import { Probe, sandbox, repository } from '../support.mjs';
import { assertRuntimeBuild } from '../build-lock.mjs';

const defaultPackage = join(homedir(), '.pi/agent/npm/node_modules/pi-subagents');
async function setup({ guarded = false, guardianFrontend = false, supervisorTimeoutMs = 8000 } = {}) {
  const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
  const metadata = JSON.parse(await readFile(join(pkg, 'package.json'), 'utf8'));
  assert.equal(metadata.name, 'pi-subagents');
  assert.equal(metadata.version, '0.76.1', 'Review subagents version drift before repinning');
  await assertRuntimeBuild(pkg, 'pi-subagents');
  assert.equal(createHash('sha256').update(await readFile(join(pkg, 'index.js'))).digest('hex'), '3b6bd20de05a9c4aa92ba79c6a69bdbfccc3254362a75ecb0bed668e9cb27007', 'Subagents entry build drift');
  const box = await sandbox();
  box.env.PI_SUBAGENTS_TEMP_ROOT = join(box.root, 'subagents-temp');
  box.env.TMPDIR = join(box.root, 'tmp');
  box.env.PI_INTERCOM_ASK_TIMEOUT_MS = String(supervisorTimeoutMs);
  try {
    await mkdir(box.env.PI_SUBAGENTS_TEMP_ROOT, { mode: 0o700 }); // Own scope before Pi can admit children.
    await mkdir(join(box.root, 'agent/agents'));
    await mkdir(join(box.root, 'agent/extensions/subagent'), { recursive: true });
    await writeFile(join(box.root, 'agent/extensions/subagent/config.json'), JSON.stringify({ toolActivation: 'eager', defaultSubagentContext: 'fresh', scheduledRuns: { enabled: false }, artifactDir: 'session', maxSubagentDepth: 2 }));
    const provider = join(repository, 'scripts/pi-compat/fixtures/provider.ts');
    for (const [name, tools] of [['proof-worker', ''], ['proof-inherit', ''], ['proof-nested', 'subagent'], ['proof-attention', 'contact_supervisor']])
      await writeFile(join(box.root, `agent/agents/${name}.md`), `---\nname: ${name}\ndescription: Disposable compatibility probe\nmodel: gui-fixture/controlled\ntools: ${tools}\nextensions: ${name === 'proof-inherit' ? '' : provider}\n---\nUse the controlled provider only.\n`);
    const probe = await Probe.start(box, ['--tools', 'subagent,subagent_supervisor', '-e', join(pkg, 'index.js'), '-e', join(repository, 'scripts/pi-compat/fixtures/subagents-bridge.ts')], box.cwd, { guardianPackage: guarded ? pkg : undefined, guardianFrontend });
    return { box, probe };
  } catch (error) { await box.cleanup(); throw error; }
}
function envelope(record) {
  if (record.type !== 'extension_ui_request' || record.method !== 'notify') return;
  try { const value = JSON.parse(record.message); if (value.bridge === 'pi-gui-subagents-probe') return value; } catch {}
}
let sequence = 0;
async function rpc(probe, method, params) {
  const requestId = `sub-proof-${++sequence}`;
  const after = probe.records.length;
  await probe.command('prompt', { message: `/fixture-subagents ${JSON.stringify({ version: 1, requestId, method, params })}` });
  const record = await probe.wait((r) => envelope(r)?.kind === 'reply' && envelope(r).data.requestId === requestId, after);
  const reply = envelope(record).data;
  assert.equal(reply.success, true, JSON.stringify(reply.error));
  if (method === 'spawn') {
    assert.ok(reply.data.details.asyncDir.startsWith(probe.box.root + '/'));
    probe.ownedRuns.push(reply.data.details);
  }
  return reply.data;
}
async function eventually(fn, description) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) { const value = await fn(); if (value) return value; await new Promise((resolve) => setTimeout(resolve, 30)); }
  throw new Error(`Deadline: ${description}`);
}
async function started(gate) { return eventually(async () => { try { return JSON.parse(await readFile(`${gate}.started`, 'utf8')); } catch (e) { if (e.code !== 'ENOENT') throw e; } }, 'fake model started'); }
async function processState(pid) {
  assert.ok(Number.isSafeInteger(pid) && pid > 0);
  try { return (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z ') ? 'terminal' : 'running'; }
  catch (error) { if (error.code === 'ENOENT') return 'terminal'; throw error; }
}
async function using(run, options) {
  const state = await setup(options);
  state.probe.box = state.box;
  state.probe.ownedRuns = [];
  try { await run(state); } finally {
    // Release only our bounded fake-provider gates, including after parent death.
    for (const file of await readdir(state.box.root))
      if (file.endsWith('.started')) await writeFile(join(state.box.root, file.slice(0, -8)), 'cleanup');
    if (!state.probe.closed) {
      for (const owned of state.probe.ownedRuns)
        await rpc(state.probe, 'stop', { id: owned.asyncId }).catch(() => {}); // Already terminal is not stoppable.
    }
    await state.probe.stop();
    for (const owned of state.probe.ownedRuns)
      await eventually(async () => {
        const status = JSON.parse(await readFile(join(owned.asyncDir, 'status.json'), 'utf8'));
        if (!['complete', 'failed', 'stopped', 'cancelled'].includes(status.state)) return false;
        // Logical completion can precede process exit; orphaned runners cannot
        // rely on the dead parent's close-event proof. Read only our recorded PID.
        try { return (await readFile(`/proc/${status.pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
        catch (error) { if (error.code === 'ENOENT') return true; throw error; }
      }, 'owned runner terminal before removing storage');
    // A separately detached nested runner is not necessarily a top-level receipt.
    // Its marker was written by our fake provider; verify every owned gate PID.
    for (const file of await readdir(state.box.root)) {
      if (!file.endsWith('.started')) continue;
      const { pid } = JSON.parse(await readFile(join(state.box.root, file), 'utf8'));
      await eventually(async () => {
        try { return (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
        catch (error) { if (error.code === 'ENOENT') return true; throw error; }
      }, 'all owned fake-model PIDs terminal');
    }
    const retained = state.probe.records.find((record) => record.type === 'fixture_guardian_ready');
    if (retained) await eventually(async () => (await processState(retained.pid)) === 'terminal', 'retained owner terminal before removing its scope');
    await state.box.cleanup();
  }
}

test('subagents public RPC launches real child, status/history and stop', () => using(async ({ probe, box }) => {
  const ping = await rpc(probe, 'ping');
  assert.equal(ping.capabilities.fleetStatus.version, 1);
  const gate = join(box.root, 'child-gate');
  const spawn = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
  const id = spawn.details.asyncId;
  assert.equal(spawn.details.launchResolvedExtensions.disableAmbientExtensions, true);
  const child = await started(gate);
  assert.equal(child.model, 'gui-fixture/controlled');
  assert.notEqual(child.pid, probe.child.pid);
  const status = await rpc(probe, 'status');
  assert.equal(status.fleet.totalActive, 1);
  assert.equal(status.fleet.entries[0].model, 'gui-fixture/controlled');
  assert.ok(status.asyncSnapshot.runs.some((r) => r.id === id && r.state === 'running' && r.activity.lastActivityAt));
  const parentTurn = probe.records.length;
  await probe.command('prompt', { message: 'hold-parent' });
  await probe.wait((r) => r.type === 'message_start' && r.message?.role === 'assistant', parentTurn);
  await probe.command('abort');
  await probe.wait((r) => r.type === 'agent_settled', parentTurn);
  assert.ok((await rpc(probe, 'status', { id })).asyncSnapshot.runs.some((r) => r.id === id && r.state === 'running'));
  const transcript = await rpc(probe, 'status', { id, view: 'transcript', lines: 20 });
  assert.match(transcript.text, /Task: gate:/); // Package-returned transcript, not terminal scraping.
  const stop = await rpc(probe, 'stop', { id });
  assert.equal(stop.state, 'stopping'); // Request receipt is not terminal proof.
  const terminal = await eventually(async () => {
    const status = JSON.parse(await readFile(join(spawn.details.asyncDir, 'status.json'), 'utf8'));
    return status.state === 'stopped' && status;
  }, 'stop reaches stopped');
  assert.equal(terminal.state, 'stopped');
  const proof = await probe.wait((r) => envelope(r)?.kind === 'subagent:process-terminal' && envelope(r).data.runId === id);
  assert.equal(envelope(proof).data.state, 'observed');
  await probe.wait((r) => envelope(r)?.kind === 'subagent:async-complete');
  assert.ok(probe.records.some((r) => envelope(r)?.kind === 'subagent:async-started'));
  assert.equal(probe.records.some((r) => r.type?.startsWith('subagent:')), false);
}));

for (const ending of ['stdin-close', 'SIGKILL']) {
  test(`detached async work survives parent ${ending}; finite child preserves history`, () => using(async ({ probe, box }) => {
    const gate = join(box.root, 'survival-gate');
    const spawn = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
    const child = await started(gate);
    if (ending === 'SIGKILL') { probe.child.kill('SIGKILL'); assert.equal((await probe.exit).signal, 'SIGKILL'); }
    else assert.deepEqual(await probe.stop(), { code: 0, signal: null });
    const status = JSON.parse(await readFile(join(spawn.details.asyncDir, 'status.json'), 'utf8'));
    assert.equal(status.state, 'running');
    process.kill(child.pid, 0); // Liveness check only; no unrelated termination.
    await writeFile(gate, 'release');
    const terminal = await eventually(async () => {
      const current = JSON.parse(await readFile(join(spawn.details.asyncDir, 'status.json'), 'utf8'));
      return current.state === 'complete' && current;
    }, 'detached runner completes after parent exit');
    assert.equal(terminal.lifecycleArtifactVersion, 3);
    assert.equal(terminal.processTerminal.state, 'pending'); // Dead parent cannot observe runner close.
    assert.ok(terminal.sessionFile.startsWith(box.root + '/'));
    const history = (await readFile(terminal.sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
    assert.ok(history.some((r) => r.message?.role === 'assistant' && r.message.content.some((c) => c.type === 'text' && c.text.includes('fixture'))));
  }));
}

test('foreground production child inherits fake provider; parent abort stops its run', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'foreground-gate');
  const after = probe.records.length;
  await probe.command('prompt', { message: `subagent:${JSON.stringify({ agent: 'proof-inherit', task: `gate:${gate}`, async: false, context: 'fresh' })}` });
  const child = await started(gate);
  assert.equal(child.pid, probe.child.pid);
  await probe.command('abort');
  await probe.wait((r) => r.type === 'agent_settled', after);
  const messages = await probe.command('get_messages');
  const result = messages.messages.find((m) => m.role === 'toolResult' && m.toolName === 'subagent');
  assert.equal(result.isError, true);
  assert.deepEqual(result.details, {}); // Pi discards final aborted-tool details.
  const update = probe.records.slice(after).find((r) => r.type === 'tool_execution_update' && r.partialResult?.details?.results[0]?.exitCode === 1);
  assert.ok(update.partialResult.details.runId);
  assert.equal(update.partialResult.details.results[0].index, 0);
  assert.match(update.partialResult.details.results[0].error, /Fixture aborted/);
  assert.deepEqual(update.partialResult.details.results[0].launchResolvedExtensions.configured, []);
}));

test('attached active child stdin-close cancels real SDK generation before parent shutdown', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'attached-shutdown-gate');
  await probe.command('prompt', { message: `subagent:${JSON.stringify({ agent: 'proof-inherit', task: `gate:${gate}`, async: false, context: 'fresh' })}` });
  assert.equal((await started(gate)).pid, probe.child.pid);
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  assert.equal(await readFile(`${gate}.aborted`, 'utf8'), 'observed');
}));

test('foreground detach completes through production executor and retained child history reopens without replay', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'detached-completion-gate');
  const after = probe.records.length;
  await probe.command('prompt', { message: `subagent:${JSON.stringify({ agent: 'proof-inherit', task: `gate:${gate}`, async: false, context: 'fresh' })}` });
  await started(gate);
  const update = await probe.wait((record) => record.type === 'tool_execution_update' && record.partialResult?.details?.runId, after);
  await probe.command('prompt', { message: `/subagents-detach ${update.partialResult.details.runId}` });
  await probe.wait((record) => record.type === 'agent_settled', after);
  await writeFile(gate, 'release');
  const files = await readdir(join(box.root, 'sessions'), { recursive: true });
  const relative = files.find((file) => file.endsWith('/run-0/session.jsonl'));
  assert.ok(relative);
  const childFile = join(box.root, 'sessions', relative);
  await eventually(async () => (await readFile(childFile, 'utf8')).trim().split('\n').map(JSON.parse).some((entry) => entry.message?.role === 'assistant' && entry.message.stopReason === 'stop'), 'detached SDK child persists completion');
  await probe.stop();
  const reopened = await Probe.start(box, ['--session', childFile]);
  try {
    assert.equal(reopened.records.some((record) => record.type === 'agent_start'), false);
    assert.ok((await reopened.command('get_messages')).messages.some((message) => message.role === 'assistant' && message.stopReason === 'stop'));
  } finally { await reopened.stop(); }
}));

test('foreground detach is same-process work; abort leaves it active and parent exit interrupts history', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'detach-gate');
  const after = probe.records.length;
  await probe.command('prompt', { message: `subagent:${JSON.stringify({ agent: 'proof-inherit', task: `gate:${gate}`, async: false, context: 'fresh' })}` });
  assert.equal((await started(gate)).pid, probe.child.pid);
  const update = await probe.wait((r) => r.type === 'tool_execution_update' && r.partialResult?.details?.runId, after);
  const id = update.partialResult.details.runId;
  await probe.command('prompt', { message: `/subagents-detach ${id}` });
  await probe.wait((r) => r.type === 'agent_settled', after);
  const { messages } = await probe.command('get_messages');
  const result = messages.find((m) => m.role === 'toolResult' && m.toolName === 'subagent');
  assert.equal(result.details.results[0].detached, true);
  // Detach receipts can omit sessionFile until reconciliation; enumerate only
  // this disposable session root, never ambient/terminal session directories.
  const files = await readdir(join(box.root, 'sessions'), { recursive: true });
  const relativeSession = files.find((file) => file.endsWith('/run-0/session.jsonl'));
  assert.ok(relativeSession);
  const sessionFile = join(box.root, 'sessions', relativeSession);
  await probe.command('abort');
  const status = await rpc(probe, 'status', { id });
  assert.match(status.text, /State: running/);
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const history = (await readFile(sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.equal(history.some((r) => r.message?.role === 'assistant' && r.message.stopReason === 'stop'), false);
}));

test('async nested activity is structured; top-level stop cancels its in-runner leaf', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'nested-gate');
  const inner = { agent: 'proof-worker', task: `gate:${gate}`, async: false, context: 'fresh' };
  const spawn = await rpc(probe, 'spawn', { agent: 'proof-nested', task: `subagent:${JSON.stringify(inner)}`, context: 'fresh' });
  const child = await started(gate);
  const status = JSON.parse(await readFile(join(spawn.details.asyncDir, 'status.json'), 'utf8'));
  assert.equal(child.pid, status.pid);
  const { run, nested } = await eventually(async () => {
    const live = await rpc(probe, 'status', { id: spawn.details.asyncId });
    const run = live.asyncSnapshot.runs.find((r) => r.id === spawn.details.asyncId);
    const nested = run?.children?.flatMap((step) => step.children ?? []).find((r) => r.label === 'proof-worker');
    return nested && { run, nested };
  }, 'nested snapshot catches up with fake-model startup marker');
  assert.equal(nested.kind, 'subagent');
  assert.equal(nested.state, 'running');
  assert.notEqual(nested.id, run.id);
  assert.equal((await rpc(probe, 'stop', { id: spawn.details.asyncId })).state, 'stopping');
  await eventually(async () => JSON.parse(await readFile(join(spawn.details.asyncDir, 'status.json'), 'utf8')).state === 'stopped', 'nested runner stopped');
}));

test('owned async exit: stop every admitted run, confirm process-terminal, then close and reopen without replay', () => using(async ({ probe, box }) => {
  const session = await probe.command('get_state');
  const runs = [];
  for (let index = 0; index < 2; index++) {
    const gate = join(box.root, `exit-gate-${index}`);
    const run = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
    runs.push(run);
    await started(gate);
  }
  await probe.command('clear_queue');
  await probe.command('abort');
  for (const run of runs) assert.equal((await rpc(probe, 'stop', { id: run.details.asyncId })).state, 'stopping');
  for (const run of runs) {
    await probe.wait((r) => envelope(r)?.kind === 'subagent:process-terminal' && envelope(r).data.runId === run.details.asyncId && envelope(r).data.state === 'observed');
    await eventually(async () => JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8')).state === 'stopped', 'exit run logically stopped');
  }
  assert.equal((await rpc(probe, 'status')).fleet.totalActive, 0);
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
  const reopened = await Probe.start(box, ['--session', session.sessionFile, '--tools', 'subagent', '-e', join(pkg, 'index.js'), '-e', join(repository, 'scripts/pi-compat/fixtures/subagents-bridge.ts')]);
  try {
    assert.equal((await reopened.command('get_state')).sessionId, session.sessionId);
    assert.equal(reopened.records.some((r) => r.type === 'agent_start'), false);
    const status = await rpc(reopened, 'status');
    assert.equal(status.fleet.totalActive, 0);
    for (const run of runs) {
      const retained = await rpc(reopened, 'status', { id: run.details.asyncId, view: 'transcript', lines: 20 });
      assert.match(retained.text, /Task: gate:/);
    }
  } finally { await reopened.stop(); }
}));

test('separately detached nested async leaf has its own process; root stop propagation is confirmed', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'detached-nested-gate');
  const inner = { agent: 'proof-worker', task: `gate:${gate}`, async: true, context: 'fresh' };
  const root = await rpc(probe, 'spawn', { agent: 'proof-nested', task: `subagent:${JSON.stringify(inner)}`, context: 'fresh' });
  const leaf = await started(gate);
  const rootStatus = JSON.parse(await readFile(join(root.details.asyncDir, 'status.json'), 'utf8'));
  assert.equal(rootStatus.state, 'running');
  assert.notEqual(leaf.pid, rootStatus.pid);
  process.kill(leaf.pid, 0); // Only the recorded owned child; observation, not termination.
  const live = await eventually(async () => {
    const status = await rpc(probe, 'status');
    const run = status.asyncSnapshot.runs.find((r) => r.id === root.details.asyncId);
    return run?.children && JSON.stringify(run.children).includes('running') && run;
  }, 'root projects its separately detached live descendant');
  assert.match(JSON.stringify(live.children), /proof-worker/);
  assert.equal((await rpc(probe, 'stop', { id: root.details.asyncId })).state, 'stopping');
  await eventually(async () => JSON.parse(await readFile(join(root.details.asyncDir, 'status.json'), 'utf8')).state === 'stopped', 'nested async root stopped');
  await eventually(async () => {
    try { return (await readFile(`/proc/${leaf.pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'separately detached leaf process terminal without releasing gate');
}));

test('guardian SIGKILL leaves recorded live work uncertain; public scoped reconciliation stops it without replay', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'guardian-loss-gate');
  const run = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
  const runner = await started(gate);
  const ready = probe.records.find((record) => record.type === 'fixture_guardian_ready');
  const owner = JSON.parse(await readFile(join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-owner.json'), 'utf8'));
  assert.equal(owner.owner.pid, ready.pid); assert.equal(owner.pi.pid, ready.piPid);
  assert.equal((await readFile(`/proc/${ready.pid}/stat`, 'utf8')).split(') ')[1].split(' ')[19], owner.owner.startTicks);
  process.kill(ready.pid, 'SIGKILL'); // Identity-checked disposable retained owner only.
  await probe.exit;
  await eventually(async () => (await processState(owner.pi.pid)) === 'terminal', 'Pi observes its lost guardian transport');
  assert.equal(await processState(runner.pid), 'running');
  assert.equal(JSON.parse(await readFile(join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-owner.json'), 'utf8')).phase, 'running');
  await assert.rejects(readFile(join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-proof.json')), { code: 'ENOENT' });
  const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
  const reconcile = spawnSync(process.execPath, [join(repository, 'scripts/pi-compat/fixtures/owned-scope-stop.mjs'), pkg, box.env.PI_SUBAGENTS_TEMP_ROOT], { env: box.env, encoding: 'utf8', timeout: 15000 });
  assert.equal(reconcile.status, 0, reconcile.stderr);
  const observed = JSON.parse(reconcile.stdout);
  assert.equal(observed.wholeRunProof, 'not-inferred');
  assert.ok(observed.runners.some((record) => record.runId === run.details.asyncId && record.terminal));
  assert.equal(await processState(runner.pid), 'terminal');
}, { guarded: true, guardianFrontend: true }));

test('frontend crash before launch receipt consumption reconciles admitted artifacts without replay', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'unexposed-launch-gate');
  const ready = probe.records.find((record) => record.type === 'fixture_guardian_ready');
  assert.ok(ready);
  const owner = JSON.parse(await readFile(join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-owner.json'), 'utf8'));
  assert.equal(owner.phase, 'running');
  assert.equal(owner.owner.pid, ready.pid); assert.equal(owner.pi.pid, ready.piPid);
  assert.ok(owner.owner.startTicks && owner.pi.startTicks && owner.owner.bootId === owner.pi.bootId);
  const request = { version: 1, requestId: 'unexposed-launch', method: 'spawn', params: { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' } };
  probe.child.stdout.pause();
  await probe.send({ type: 'prompt', id: 'unexposed-command', message: `/fixture-subagents ${JSON.stringify(request)}` });
  const runner = await started(gate);
  assert.equal(probe.records.some((record) => record.id === 'unexposed-command' || envelope(record)?.data?.requestId === request.requestId), false);
  probe.child.stdout.destroy(); probe.child.kill('SIGKILL'); await probe.exit;
  const proofPath = join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-proof.json');
  const proof = await eventually(async () => { try { return JSON.parse(await readFile(proofPath, 'utf8')); } catch (error) { if (error.code === 'ENOENT' || error instanceof SyntaxError) return false; throw error; } }, 'guardian reconciles after admission but before frontend receipt');
  assert.ok(['frontend-eof', 'frontend-pipe-loss'].includes(proof.reason));
  assert.equal(proof.scope.uncertain, undefined);
  assert.ok(proof.scope.runners.length >= 1);
  assert.ok(proof.scope.runners.every((record) => record.terminal));
  assert.equal(await processState(runner.pid), 'terminal');
  assert.ok(await eventually(async () => (await processState(ready.pid)) === 'terminal', 'unexposed launch guardian closes'));
}, { guarded: true, guardianFrontend: true }));

test('real frontend SIGKILL closes all pipes; retained guardian independently stops admitted runners', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'frontend-crash-gate');
  await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
  await started(gate);
  const ready = probe.records.find((r) => r.type === 'fixture_guardian_ready');
  assert.ok(ready && ready.pid !== probe.child.pid);
  probe.child.kill('SIGKILL'); // Only the disposable frontend, not its guardian.
  await probe.exit;
  const result = await eventually(async () => {
    try { return JSON.parse(await readFile(join(box.env.PI_SUBAGENTS_TEMP_ROOT, 'guardian-proof.json'), 'utf8')); }
    catch (error) { if (error.code === 'ENOENT') return false; throw error; }
  }, 'independent guardian persists proof after frontend dies');
  assert.equal(result.scope.uncertain, undefined);
  assert.ok(result.scope.runners.every((run) => run.terminal));
  assert.deepEqual(result.parent, { code: 0, signal: null });
  await eventually(async () => {
    try { return (await readFile(`/proc/${ready.pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'owned guardian exits before storage removal');
}, { guarded: true, guardianFrontend: true }));

test('independent guardian owns frontend EOF: queues cleared and separately detached nested runners stop automatically', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'guarded-nested-gate');
  const inner = { agent: 'proof-worker', task: `gate:${gate}`, async: true, context: 'fresh' };
  await rpc(probe, 'spawn', { agent: 'proof-nested', task: `subagent:${JSON.stringify(inner)}`, context: 'fresh' });
  await started(gate);
  const after = probe.records.length;
  const proof = probe.wait((r) => r.type === 'fixture_guardian_done', after, 15000);
  // Pipe EOF is also what the OS delivers when a frontend dies. No manual stop
  // requests and no helper invocation by the test after this handoff.
  probe.child.stdin.end();
  const result = await proof;
  assert.equal(result.reason, 'frontend-eof');
  assert.equal(result.admissionCleared, true);
  assert.ok(result.scope.runners.length >= 2);
  assert.ok(result.scope.runners.every((run) => run.terminal));
  assert.deepEqual(result.parent, { code: 0, signal: null });
  assert.deepEqual(await probe.exit, { code: 0, signal: null });
}, { guarded: true }));

for (const nested of [false, true]) {
  test(`private-scope guardian reconciles ${nested ? 'separately detached nested' : 'single'} launch without an RPC receipt`, () => using(async ({ probe, box }) => {
    const gate = join(box.root, 'unreceipted-gate');
    const inner = { agent: 'proof-worker', task: `gate:${gate}`, async: true, context: 'fresh' };
    const request = { version: 1, requestId: 'unreceipted-spawn', method: 'spawn', params: { agent: nested ? 'proof-nested' : 'proof-worker', task: nested ? `subagent:${JSON.stringify(inner)}` : `gate:${gate}`, context: 'fresh' } };
    // Persist the private scope before dispatch. Deliberately do not correlate or
    // register a launch receipt: the observer must recover from owned artifacts.
    await probe.send({ type: 'prompt', id: 'unobserved-dispatch', message: `/fixture-subagents ${JSON.stringify(request)}` });
    await started(gate);
    assert.equal(probe.ownedRuns.length, 0);
    probe.child.kill('SIGKILL');
    await probe.exit;
    const outside = join(box.root, 'not-owned-by-guardian');
    await mkdir(outside);
    await writeFile(join(outside, 'status.json'), JSON.stringify({ lifecycleArtifactVersion: 3, runId: 'unrelated', pid: process.pid, state: 'running' }));
    const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
    const result = spawnSync(process.execPath, [join(repository, 'scripts/pi-compat/fixtures/owned-scope-stop.mjs'), pkg, box.env.PI_SUBAGENTS_TEMP_ROOT], { cwd: box.cwd, env: box.env, encoding: 'utf8', timeout: 12000, maxBuffer: 8192 });
    assert.equal(result.status, 0, result.stderr);
    const proof = JSON.parse(result.stdout);
    assert.ok(proof.runners.length >= (nested ? 2 : 1));
    assert.ok(proof.runners.every((run) => run.terminal));
    assert.equal(proof.wholeRunProof, 'not-inferred');
    assert.equal(proof.runners.some((run) => run.runId === 'unrelated'), false);
    assert.deepEqual(await readdir(outside), ['status.json']);
  }));
}

test('owned guardian feasibility: public file-control export stops detached work after parent death without reopening', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'guardian-gate');
  const run = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
  const child = await started(gate);
  const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
  const manifest = JSON.parse(await readFile(join(pkg, 'package.json'), 'utf8'));
  assert.equal(manifest.exports['./control-channel'].default, './src/api/control-channel.js');
  probe.child.kill('SIGKILL');
  await probe.exit; // A living owner can observe death and act on admitted receipts.
  assert.ok(run.details.asyncDir.startsWith(box.root + '/'));
  const script = `const { requestAsyncStop } = await import(${JSON.stringify(pathToFileURL(join(pkg, manifest.exports['./control-channel'].default)).href)}); process.stdout.write(JSON.stringify({ requestPath: requestAsyncStop(${JSON.stringify(run.details.asyncDir)}, { source: 'owned-guardian-probe' }) }));`;
  const receipt = spawnSync(process.execPath, ['--input-type=module', '-e', script], { cwd: box.cwd, env: box.env, encoding: 'utf8', timeout: 3000, maxBuffer: 8192 });
  assert.equal(receipt.status, 0, receipt.stderr);
  assert.ok(JSON.parse(receipt.stdout).requestPath.startsWith(run.details.asyncDir + '/'));
  await eventually(async () => JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8')).state === 'stopped', 'public guardian stop logically completes');
  await eventually(async () => {
    try { return (await readFile(`/proc/${child.pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'guardian-controlled owned runner process terminal');
}));

test('noncooperative runner: owned escalation terminates PID but package whole-run proof remains unknown', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'noncoop-gate');
  const run = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `noncoop-gate:${gate}`, context: 'fresh' });
  const child = await started(gate);
  const identity = (await readFile(`/proc/${child.pid}/stat`, 'utf8')).split(') ')[1].split(' ')[19];
  assert.equal((await rpc(probe, 'stop', { id: run.details.asyncId })).state, 'stopping');
  await eventually(async () => { try { return await readFile(`${gate}.aborted`, 'utf8'); } catch (error) { if (error.code === 'ENOENT') return false; throw error; } }, 'noncooperative model receives cancellation');
  const current = (await readFile(`/proc/${child.pid}/stat`, 'utf8')).split(') ')[1].split(' ');
  assert.notEqual(current[0], 'Z'); // Model ignored cancellation; no stop-complete claim.
  assert.equal(current[19], identity, 'Do not signal a reused PID');
  process.kill(child.pid, 'SIGKILL'); // Only the recorded owned runner after identity verification.
  const event = await probe.wait((r) => envelope(r)?.kind === 'subagent:process-terminal' && envelope(r).data.runId === run.details.asyncId);
  assert.equal(envelope(event).data.state, 'unknown');
  assert.equal(envelope(event).data.reason, 'writer-close-unverified');
  await eventually(async () => {
    const status = JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8'));
    return ['failed', 'stopped', 'cancelled'].includes(status.state) && status;
  }, 'package reconciles interrupted runner, without inventing successful completion');
}));

test('lost-parent ownership: reopened same session can stop its retained detached runner without replay', () => using(async ({ probe, box }) => {
  const gate = join(box.root, 'reopen-stop-gate');
  const run = await rpc(probe, 'spawn', { agent: 'proof-worker', task: `gate:${gate}`, context: 'fresh' });
  const child = await started(gate);
  const session = await probe.command('get_state');
  probe.child.kill('SIGKILL');
  await probe.exit;
  const pkg = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? defaultPackage;
  const reopened = await Probe.start(box, ['--session', session.sessionFile, '--tools', 'subagent', '-e', join(pkg, 'index.js'), '-e', join(repository, 'scripts/pi-compat/fixtures/subagents-bridge.ts')]);
  try {
    assert.equal(reopened.records.some((r) => r.type === 'agent_start'), false);
    assert.equal((await rpc(reopened, 'stop', { id: run.details.asyncId })).state, 'stopping');
    await eventually(async () => JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8')).state === 'stopped', 'retained lost-parent child stops');
    await eventually(async () => {
      try { return (await readFile(`/proc/${child.pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
      catch (error) { if (error.code === 'ENOENT') return true; throw error; }
    }, 'lost-parent recorded runner process terminal');
  } finally { await reopened.stop(); }
}));

test('native supervisor attention arrives with structured request identity; a targeted reply releases real child', () => using(async ({ probe }) => {
  const after = probe.records.length;
  const task = `tool:${JSON.stringify({ name: 'contact_supervisor', arguments: { reason: 'need_decision', message: 'Controlled decision needed' } })}`;
  const run = await rpc(probe, 'spawn', { agent: 'proof-attention', task, context: 'fresh' });
  const notice = await probe.wait((r) => r.type === 'message_end' && r.message?.details?.requestBody === 'Controlled decision needed', after);
  const request = notice.message.details;
  assert.equal(request.runId, run.details.asyncId);
  assert.equal(request.expectsReply, true);
  assert.equal(request.reason, 'need_decision');
  // The package's real supervisor wake may have started a finite parent turn.
  await eventually(async () => !(await probe.command('get_state')).isStreaming, 'supervisor wake parent settles');
  const replyAfter = probe.records.length;
  const settled = probe.wait((r) => r.type === 'agent_settled', replyAfter);
  await probe.command('prompt', { message: `tool:${JSON.stringify({ name: 'subagent_supervisor', arguments: { action: 'reply', replyTo: request.requestId, message: 'Controlled reply' } })}` });
  await settled;
  const reply = probe.records.slice(replyAfter).find((r) => r.type === 'tool_execution_end' && r.toolName === 'subagent_supervisor');
  assert.equal(reply.isError, false);
  assert.equal(reply.result.details.replyTo, request.requestId);
  await eventually(async () => JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8')).state === 'complete', 'supervisor child completes after reply');
  await probe.wait((r) => envelope(r)?.kind === 'subagent:process-terminal' && envelope(r).data.runId === run.details.asyncId);
}));

test('supervisor expiry retires attention and rejects a stale targeted reply', () => using(async ({ probe }) => {
  const after = probe.records.length;
  const task = `tool:${JSON.stringify({ name: 'contact_supervisor', arguments: { reason: 'need_decision', message: 'Expiring controlled request' } })}`;
  const run = await rpc(probe, 'spawn', { agent: 'proof-attention', task, context: 'fresh' });
  const notice = await probe.wait((record) => record.type === 'message_end' && record.message?.details?.requestBody === 'Expiring controlled request', after);
  const requestId = notice.message.details.requestId;
  await eventually(async () => ['complete', 'failed', 'stopped'].includes(JSON.parse(await readFile(join(run.details.asyncDir, 'status.json'), 'utf8')).state), 'child request timeout completes its finite workflow');
  await eventually(async () => !(await probe.command('get_state')).isStreaming, 'parent attention wake settles');
  const replyAfter = probe.records.length;
  const settled = probe.wait((record) => record.type === 'agent_settled', replyAfter);
  await probe.command('prompt', { message: `tool:${JSON.stringify({ name: 'subagent_supervisor', arguments: { action: 'reply', replyTo: requestId, message: 'Must not answer expired request' } })}` });
  await settled;
  const reply = probe.records.slice(replyAfter).find((record) => record.type === 'tool_execution_end' && record.toolName === 'subagent_supervisor');
  assert.equal(reply.isError, true);
  assert.match(JSON.stringify(reply.result), /No pending supervisor request/);
}, { supervisorTimeoutMs: 1500 }));

test('child-safe nested delegation uses real SDK children and exposes saved child history', () => using(async ({ probe }) => {
  const after = probe.records.length;
  const inner = { agent: 'proof-worker', task: 'nested-leaf', async: false, context: 'fresh' };
  await probe.command('prompt', { message: `subagent:${JSON.stringify({ agent: 'proof-nested', task: `subagent:${JSON.stringify(inner)}`, async: false, context: 'fresh' })}` });
  await probe.wait((r) => r.type === 'agent_settled', after);
  const { messages } = await probe.command('get_messages');
  const result = messages.find((m) => m.role === 'toolResult' && m.toolName === 'subagent');
  assert.equal(result.details.runFanoutBudget.used, 2);
  const parent = result.details.results[0];
  assert.equal(parent.index, 0);
  assert.equal(parent.progressSummary.toolCount, 1);
  const nested = parent.children[0];
  assert.equal(nested.parentRunId, result.details.runId);
  assert.equal(nested.depth, 1);
  assert.equal(nested.state, 'complete');
  const childHistory = (await readFile(nested.steps[0].sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(childHistory.some((r) => r.message?.role === 'assistant' && r.message.model === 'controlled'));
}));
