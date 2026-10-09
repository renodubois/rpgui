import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { Probe, piPackage, repository, sandbox } from '../support.mjs';
import { assertDurableRoot } from '../durable-lock.mjs';

const installRoot = process.env.PI_GUI_DURABLE_ROOT ?? join(homedir(), 'programs/pi-gui-durable-1.1.0');
async function eventually(check, label) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) { if (await check()) return; await new Promise((resolve) => setTimeout(resolve, 20)); }
  throw new Error(label);
}
async function scenario(run) {
  await assertDurableRoot(installRoot); // Before any downloaded module loads.
  const box = await sandbox(); const sockets = new Set(); const requests = []; const effects = []; const closed = [];
  const children = [];
  const server = createServer(async (req, res) => {
    let text = ''; for await (const chunk of req) { text += chunk; assert.ok(text.length < 1024 * 1024); }
    if (req.url === '/effect') { effects.push(JSON.parse(text)); return; } // Accepted effect, deliberately no receipt.
    assert.equal(req.url, '/v1/chat/completions');
    assert.ok(['Bearer synthetic-key', 'Bearer synthetic-refreshed'].includes(req.headers.authorization));
    const body = JSON.parse(text); assert.equal(body.model, 'controlled'); requests.push(body);
    res.on('close', () => closed.push(body));
    res.writeHead(200, { 'Content-Type': 'text/event-stream' }); res.flushHeaders();
    const last = body.messages.filter((message) => message.role === 'user').at(-1)?.content;
    if (last === 'hold') return;
    const tool = last === 'effect' && !body.messages.some((message) => message.role === 'tool');
    const delta = tool ? { role: 'assistant', tool_calls: [{ index: 0, id: 'effect-call', type: 'function', function: { name: 'approved_effect', arguments: '{}' } }] } : { role: 'assistant', content: 'Durable configured provider proof' };
    for (const [change, finish] of [[delta, null], [{}, tool ? 'tool_calls' : 'stop']]) res.write(`data: ${JSON.stringify({ id: 'controlled', object: 'chat.completion.chunk', choices: [{ index: 0, delta: change, finish_reason: finish }], usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } })}\n\n`);
    res.end('data: [DONE]\n\n');
  });
  server.on('connection', (socket) => { sockets.add(socket); socket.once('close', () => sockets.delete(socket)); });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    await writeFile(join(box.root, 'agent/models.json'), JSON.stringify({ providers: { 'gui-loopback': { baseUrl: base + '/v1', api: 'openai-completions', apiKey: 'synthetic-key', models: [{ id: 'controlled', name: 'Disposable service', reasoning: false, input: ['text'], contextWindow: 200000, maxTokens: 4096, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }] } } }));
    const piRoot = await piPackage(); let sequence = 0;
    async function start(backend, mode, options = {}) {
      const storage = join(box.root, backend === 'jsonl' ? 'durable-jsonl' : 'durable.sqlite');
      const path = join(box.root, `durable-request-${++sequence}.json`);
      await writeFile(path, JSON.stringify({ installRoot, piRoot, storage, backend, base, mode, ...options }));
      // Storage has no built-in owner lock. Use the same actual kernel-lock
      // mechanism for every opening, retained by the exec'd Node process.
      const child = spawn('flock', ['--no-fork', '--nonblock', '--conflict-exit-code', '73', storage + '.lock', process.execPath, join(repository, 'scripts/pi-compat/fixtures/durable-driver.mjs'), path], { env: box.env, cwd: box.cwd, stdio: ['pipe', 'pipe', 'pipe'] });
      const probe = new Probe(child); children.push(probe); return probe;
    }
    await run({ box, start, requests, effects, closed });
  } finally {
    for (const child of children) await child.stop();
    for (const socket of sockets) socket.destroy();
    await new Promise((resolve) => server.close(resolve));
    await box.cleanup();
  }
}

for (const backend of ['sqlite', 'jsonl']) {
  test(`Durable ${backend}: invalid owned storage fails before model admission without fallback`, { timeout: 30000 }, () => scenario(async ({ box, start, requests }) => {
    const storage = join(box.root, backend === 'jsonl' ? 'durable-jsonl' : 'durable.sqlite');
    if (backend === 'sqlite') await mkdir(storage);
    else await writeFile(storage, 'Existing owned file must not be replaced');
    const failed = await start(backend, 'inspect');
    assert.notEqual((await failed.exit).code, 0);
    assert.equal(failed.records.some((record) => record.type === 'durable_admitted'), false);
    assert.equal(requests.length, 0);
    assert.match(failed.stderr, /EEXIST|sqlite|database/i);
    if (backend === 'jsonl') assert.equal(await readFile(storage, 'utf8'), 'Existing owned file must not be replaced');
  }));
  test(`Durable ${backend}: CLI ModelRuntime reuse, request deduplication, close and passive reopen`, { timeout: 30000 }, () => scenario(async ({ start, requests }) => {
    const writer = await start(backend, 'complete');
    const admission = await writer.wait((record) => record.type === 'durable_admitted');
    const answer = await writer.wait((record) => record.type === 'durable_answer');
    assert.match(answer.text, /Durable configured provider proof/); assert.equal(answer.duplicate, true);
    assert.deepEqual(await writer.exit, { code: 0, signal: null }); assert.equal(requests.length, 1);
    const reopened = await start(backend, 'inspect');
    const state = await reopened.wait((record) => record.type === 'durable_inspection');
    assert.equal(state.id, admission.id); assert.equal(state.scheduling, 'paused'); assert.equal(state.tasks.length, 0); assert.ok(state.entries > 0);
    assert.deepEqual(await reopened.exit, { code: 0, signal: null }); assert.equal(requests.length, 1);
  }));
  test(`Durable ${backend}: exclusive owner, active close, passive reopen and explicit abort-mark retirement without model replay`, { timeout: 30000 }, () => scenario(async ({ start, requests, closed }) => {
    const held = await start(backend, 'hold'); await held.wait((record) => record.type === 'durable_admitted');
    await eventually(() => requests.length === 1, 'held model request starts');
    const conflict = await start(backend, 'inspect'); assert.equal((await conflict.exit).code, 73);
    assert.deepEqual(await held.stop(), { code: 0, signal: null });
    await eventually(() => closed.length === 1, 'close cancels owned model socket');
    const reopened = await start(backend, 'inspect');
    const state = await reopened.wait((record) => record.type === 'durable_inspection');
    assert.equal(state.scheduling, 'paused'); assert.ok(state.tasks.length > 0);
    await reopened.exit; assert.equal(requests.length, 1);
    const retired = await start(backend, 'retire');
    assert.equal((await retired.wait((record) => record.type === 'durable_retired')).tasks, 0);
    assert.deepEqual(await retired.exit, { code: 0, signal: null }); assert.equal(requests.length, 1);
    const fresh = await start(backend, 'complete'); await fresh.wait((record) => record.type === 'durable_answer'); await fresh.exit;
    assert.equal(requests.length, 2);
  }));
  test(`Durable ${backend}: SIGKILL after external dispatch retains uncertainty; passive reopen and unsafe-tool recovery never resend`, { timeout: 30000 }, () => scenario(async ({ start, effects, requests }) => {
    const active = await start(backend, 'effect', { effect: true }); await active.wait((record) => record.type === 'durable_admitted');
    await eventually(() => effects.length === 1, 'external effect accepted without receipt');
    active.child.kill('SIGKILL'); assert.equal((await active.exit).signal, 'SIGKILL');
    const reopened = await start(backend, 'inspect', { effect: true });
    const state = await reopened.wait((record) => record.type === 'durable_inspection'); assert.equal(state.scheduling, 'paused'); assert.ok(state.tasks.length > 0);
    await reopened.exit; assert.equal(effects.length, 1); assert.equal(requests.length, 1);
    const recovery = await start(backend, 'resume', { effect: true });
    assert.equal((await recovery.wait((record) => record.type === 'durable_resumed')).tasks, 0);
    assert.deepEqual(await recovery.exit, { code: 0, signal: null });
    assert.equal(effects.length, 1);
    assert.ok(requests.at(-1).messages.some((message) => message.role === 'tool' && /interrupt/i.test(message.content)));
  }));
}

test('Durable reuses CLI credential storage: synthetic expired OAuth refresh persists once and drives actual transport', { timeout: 30000 }, () => scenario(async ({ box, start, requests }) => {
  const authPath = join(box.root, 'agent/auth.json');
  await writeFile(authPath, JSON.stringify({ 'gui-oauth': { type: 'oauth', access: 'synthetic-expired', refresh: 'synthetic-refresh', expires: 1 } }));
  const child = await start('sqlite', 'complete', { oauth: true });
  const result = await child.wait((record) => record.type === 'durable_answer');
  assert.equal(result.refreshes, 1); assert.equal(requests.length, 1);
  assert.deepEqual(await child.exit, { code: 0, signal: null });
  const stored = JSON.parse(await readFile(authPath, 'utf8'))['gui-oauth'];
  assert.equal(stored.access, 'synthetic-refreshed'); assert.equal(stored.refresh, 'synthetic-rotated');
}));
