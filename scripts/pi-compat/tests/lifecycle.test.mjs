import assert from 'node:assert/strict';
import { readFile, mkdir, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { Probe, repository, withProbe } from '../support.mjs';

async function eventually(fn, description) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const value = await fn();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(description);
}
async function terminal(pid) {
  try { return (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
  catch (error) { if (error.code === 'ENOENT') return true; throw error; }
}

test('active RPC stdin-close aborts fake generation, persists shutdown and does not replay queues', { timeout: 20000 }, async () => withProbe(async (probe) => {
  await probe.command('prompt', { message: 'hold' });
  await probe.wait((r) => r.type === 'message_start' && r.message.role === 'assistant');
  await probe.command('follow_up', { message: 'must not replay on shutdown' });
  const state = await probe.command('get_state');
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const entries = (await readFile(state.sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(entries.some((e) => e.customType === 'fixture-shutdown'));
  assert.equal(entries.filter((e) => e.message?.role === 'user').length, 1);
}));

for (const ending of ['abort_bash', 'stdin-close']) {
  test(`production user bash process tree: ${ending} stops both controlled descendants`, { timeout: 20000 }, async () => withProbe(async (probe, box) => {
    const marker = join(box.root, 'processes.json');
    const command = `${JSON.stringify(process.execPath)} ${JSON.stringify(join(repository, 'scripts/pi-compat/fixtures/process-tree.mjs'))} ${JSON.stringify(marker)}`;
    let pids;
    const result = probe.command('bash', { command, excludeFromContext: true });
    void result.catch(() => {}); // Orderly exit can retire an in-flight response.
    try {
      pids = await eventually(async () => {
        try { return JSON.parse(await readFile(marker, 'utf8')); }
        catch (error) { if (error.code === 'ENOENT') return false; throw error; }
      }, 'owned bash descendants started');
      assert.equal(await terminal(pids.child), false);
      if (ending === 'abort_bash') {
        await probe.command('abort_bash'); // Stop acceptance, not process proof.
        assert.equal((await result).cancelled, true);
      } else { assert.deepEqual(await probe.stop(), { code: 0, signal: null }); await result.catch(() => {}); }
      await eventually(async () => await terminal(pids.parent) && await terminal(pids.child), 'both recorded process identities terminal');
    } finally {
      await probe.command('abort_bash').catch(() => {});
      await result.catch(() => {});
      // A failure still waits for finite owned work before temporary-file cleanup.
      if (pids) await eventually(async () => await terminal(pids.parent) && await terminal(pids.child), 'finite bash cleanup');
    }
  }));
}

test('lost acceptance after dispatch is ambiguous: saved work is reopened without automatic resend', { timeout: 20000 }, async () => withProbe(async (probe, box) => {
  const state = await probe.command('get_state');
  const gate = join(box.root, 'ambiguous-dispatch-gate');
  probe.child.stdout.pause();
  await probe.send({ type: 'prompt', id: 'lost-acceptance', message: `gate:${gate}` });
  await eventually(async () => { try { return JSON.parse(await readFile(`${gate}.started`, 'utf8')); } catch (error) { if (error.code === 'ENOENT') return false; throw error; } }, 'actual model starts while acceptance transport is paused');
  assert.equal(probe.records.some((record) => record.id === 'lost-acceptance'), false);
  probe.child.stdout.destroy(); // Acknowledgement is irretrievably unavailable to this client.
  probe.child.kill('SIGKILL');
  await probe.exit;
  const entries = (await readFile(state.sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.equal(entries.filter((entry) => entry.message?.role === 'user').length, 1);
  const reopened = await Probe.start(box, ['--session', state.sessionFile]);
  try {
    assert.equal(reopened.records.some((record) => record.type === 'agent_start'), false);
    assert.equal((await reopened.command('get_state')).pendingMessageCount, 0);
    assert.equal((await reopened.command('get_messages')).messages.filter((message) => message.role === 'user').length, 1);
  } finally { await reopened.stop(); }
}));

test('actual persistence failure is correlated and may leave an in-memory mutation; never infer durable success', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const done = probe.wait((record) => record.type === 'agent_settled');
  await probe.command('prompt', { message: 'durable baseline' });
  await done;
  const state = await probe.command('get_state');
  const original = await readFile(state.sessionFile, 'utf8');
  await rm(state.sessionFile); await mkdir(state.sessionFile); // Disposable owned path only.
  try {
    const failed = probe.wait((record) => record.type === 'response' && record.id === 'failed-save');
    await probe.send({ type: 'set_session_name', id: 'failed-save', name: 'Unpersisted title' });
    const result = await failed;
    assert.equal(result.success, false);
    assert.match(result.error, /EISDIR/);
    assert.equal((await probe.command('get_state')).sessionName, 'Unpersisted title');
  } finally {
    await rm(state.sessionFile, { recursive: true }); await writeFile(state.sessionFile, original);
  }
}));

test('actual output artifact write failure crashes Pi; a returned path is not durable output proof', { timeout: 20000 }, async () => withProbe(async (probe, box) => {
  const temporary = box.env.TMPDIR;
  const marker = join(box.root, 'artifact-writer.json');
  await rm(temporary, { recursive: true }); await writeFile(temporary, 'Unavailable disposable output directory');
  try {
    const script = `require('node:fs').writeFileSync(${JSON.stringify(marker)},JSON.stringify({pid:process.pid}));process.stdout.write('x'.repeat(1024*1024))`;
    const result = probe.command('bash', { command: `${JSON.stringify(process.execPath)} -e ${JSON.stringify(script)}` }).then((data) => ({ data }), (error) => ({ error }));
    const exited = await probe.exit;
    assert.notEqual(exited.code, 0);
    assert.match(probe.stderr, /ENOTDIR/);
    const outcome = await result;
    if (outcome.data?.fullOutputPath) await assert.rejects(readFile(outcome.data.fullOutputPath), { code: 'ENOTDIR' });
    else assert.ok(outcome.error, 'No successful artifact receipt after its failed write');
    const { pid } = JSON.parse(await readFile(marker, 'utf8'));
    await eventually(async () => {
      try { return (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].startsWith('Z '); }
      catch (error) { if (error.code === 'ENOENT') return true; throw error; }
    }, 'owned artifact writer terminal before removing storage');
  } finally { await rm(temporary); await mkdir(temporary); }
}));

test('session switch retires waiting Q+A, rejects late answer reuse and restores saved cwd without replay', { timeout: 20000 }, async () => withProbe(async (probe, box) => {
  const otherCwd = join(box.root, 'other-project'); await mkdir(otherCwd);
  const writer = await Probe.start(box, [], otherCwd);
  let saved;
  try {
    const done = writer.wait((record) => record.type === 'agent_settled');
    await writer.command('prompt', { message: 'other saved history' }); await done;
    saved = await writer.command('get_state');
  } finally { await writer.stop(); }
  const after = probe.records.length;
  const question = 'qna:{"question":"Choose?","options":[{"label":"A"},{"label":"B"}],"allowCustom":false}';
  await probe.command('prompt', { message: question });
  const old = await probe.wait((record) => record.type === 'extension_ui_request' && record.method === 'select', after);
  assert.equal((await probe.command('switch_session', { sessionPath: saved.sessionFile })).cancelled, false);
  const state = await probe.command('get_state');
  assert.equal(state.sessionId, saved.sessionId); assert.equal(state.isStreaming, false);
  await probe.send({ type: 'extension_ui_response', id: old.id, value: old.options[0] });
  const cursor = probe.records.length;
  const cwdEntry = probe.wait((record) => record.type === 'entry_appended' && record.entry.customType === 'fixture-cwd', cursor);
  await probe.command('prompt', { message: '/fixture-cwd' });
  assert.equal((await cwdEntry).entry.data.cwd, otherCwd);
  const next = probe.records.length;
  const settled = probe.wait((record) => record.type === 'agent_settled', next);
  await probe.command('prompt', { message: question });
  const fresh = await probe.wait((record) => record.type === 'extension_ui_request' && record.method === 'select', next);
  assert.notEqual(fresh.id, old.id);
  await probe.send({ type: 'extension_ui_response', id: fresh.id, value: fresh.options[1] });
  await settled;
  assert.equal(probe.records.slice(next).find((record) => record.type === 'tool_execution_end' && record.toolName === 'ask_user').result.details.answer, 'B');
}));

for (const pauseMs of [200, 1500]) {
  test(`actual RPC consumer paused ${pauseMs}ms drains large bash output and authoritative artifact`, { timeout: 20000 }, async () => withProbe(async (probe, box) => {
  const after = probe.records.length;
  const bytes = (pauseMs === 200 ? 1 : 4) * 1024 * 1024;
  const script = `process.stdout.write('x'.repeat(${bytes}));`;
  probe.child.stdout.pause(); // Controlled consumer stall, not terminal scraping.
  const resume = setTimeout(() => probe.child.stdout.resume(), pauseMs);
  try {
    const result = await probe.command('bash', { command: `${JSON.stringify(process.execPath)} -e ${JSON.stringify(script)}`, excludeFromContext: true });
    assert.equal(result.exitCode, 0);
    assert.equal(typeof result.truncated, 'boolean');
    assert.ok(result.output.length < bytes); // Rolling buffer may report truncated:false.
    assert.ok(result.fullOutputPath.startsWith(join(box.root, 'tmp') + '/'));
    const output = await readFile(result.fullOutputPath, 'utf8');
    assert.equal(output.length, bytes);
    const updates = probe.records.slice(after).filter((r) => r.type === 'bash_execution_update');
    assert.ok(updates.length);
    assert.ok(updates.every((r) => typeof r.id === 'string'));
    assert.equal(updates.map((r) => r.delta).join('').length, bytes);
    assert.equal((await probe.command('get_state')).isStreaming, false);
  } finally { clearTimeout(resume); probe.child.stdout.resume(); }
  }));
}

test('production retry emits structured activity and settles only after recovery', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const done = probe.wait((r) => r.type === 'agent_settled', after);
  await probe.command('prompt', { message: 'retry-once' });
  await done;
  const events = probe.records.slice(after);
  assert.ok(events.some((r) => r.type === 'auto_retry_start' && r.attempt === 1));
  assert.ok(events.some((r) => r.type === 'auto_retry_end' && r.success === true));
  assert.equal(events.filter((r) => r.type === 'agent_settled').length, 1);
  assert.equal((await probe.command('get_messages')).messages.at(-1).stopReason, 'stop');
  assert.ok((await probe.command('get_entries')).entries.some((e) => e.message?.stopReason === 'error'));
}, [], { retry: { enabled: true, maxRetries: 1, baseDelayMs: 10 } }));

test('RPC native control mapping: model/thinking, queues, stats, rename and compaction', { timeout: 20000 }, async () => withProbe(async (probe) => {
  assert.equal((await probe.command('set_model', { provider: 'gui-fixture', modelId: 'controlled' })).provider, 'gui-fixture');
  assert.deepEqual((await probe.command('get_available_thinking_levels')).levels, ['off']);
  await probe.command('set_thinking_level', { level: 'off' });
  await probe.command('set_steering_mode', { mode: 'all' });
  await probe.command('set_follow_up_mode', { mode: 'one-at-a-time' });
  await probe.command('set_session_name', { name: 'Controlled title' });
  assert.equal((await probe.command('get_state')).sessionName, 'Controlled title');
  const after = probe.records.length;
  const done = probe.wait((r) => r.type === 'agent_settled', after);
  await probe.command('prompt', { message: 'compactible history '.repeat(20) });
  await done;
  const secondDone = probe.wait((r) => r.type === 'agent_settled', probe.records.length);
  await probe.command('prompt', { message: 'recent turn' });
  await secondDone;
  const stats = await probe.command('get_session_stats');
  assert.ok(stats.tokens.total > 0);
  const start = probe.records.length;
  const compacted = await probe.command('compact', { customInstructions: 'Summarize controlled history.' });
  assert.ok(compacted.summary);
  assert.ok(probe.records.slice(start).some((r) => r.type === 'compaction_start'));
  assert.ok(probe.records.slice(start).some((r) => r.type === 'compaction_end'));
  assert.ok((await probe.command('get_entries')).entries.some((e) => e.type === 'compaction'));
}, [], { compaction: { enabled: false, keepRecentTokens: 1, reserveTokens: 100 } }));
