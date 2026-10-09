import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { Probe, sandbox } from '../support.mjs';

const tools = ['web_search', 'fetch_content', 'get_search_content', 'source_check'];
async function webProbe(run, activation = 'eager') {
  const box = await sandbox();
  let probe;
  const requests = [];
  const sockets = new Set();
  const closedResponses = new Set();
  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    requests.push(url);
    res.once('close', () => closedResponses.add(url.pathname));
    if (url.pathname === '/pagewait') return;
    if (url.pathname === '/search') {
      if (url.searchParams.get('q') === 'wait') return; // Owned socket closed in finally.
      if (url.searchParams.get('q') === 'failure') { res.writeHead(503); res.end('fixture unavailable'); return; }
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ results: [{ title: 'Controlled source', url: `${base}/${url.searchParams.get('q') === 'background' ? 'pagewait' : 'page'}`, content: 'Compatibility evidence from a disposable service.' }] }));
    } else if (url.pathname === '/page') {
      res.setHeader('Content-Type', 'text/html');
      res.end('<html><head><title>Controlled source</title></head><body><article><p>Compatibility evidence from a disposable service. No external provider was contacted.</p></article></body></html>');
    } else { res.writeHead(404); res.end('fixture missing'); }
  });
  server.on('connection', (socket) => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  try {
    const pkg = process.env.PI_GUI_WEB_PACKAGE ?? join(process.env.HOME, '.pi/agent/npm/node_modules/pi-web-access');
    const manifest = JSON.parse(await readFile(join(pkg, 'package.json'), 'utf8'));
    assert.equal(manifest.name, 'pi-web-access');
    assert.equal(manifest.version, '0.37.0', 'Review package drift before loading');
    assert.deepEqual(manifest.pi.extensions, ['./dist']);
    assert.equal(createHash('sha256').update(await readFile(join(pkg, 'dist/index.js'))).digest('hex'), '42e4354f6f3b5fa8db8228e96b81e7c314d974aafebdfde437eff8873502bb0e', 'Reviewed web bundle changed');
    box.env.TMPDIR = join(box.root, 'tmp');
    box.env.PI_WEB_ACCESS_CACHE_ROOT = join(box.root, 'web-cache');
    await writeFile(join(box.root, 'agent/web-search.json'), JSON.stringify({
      toolActivation: activation, workflow: 'none', autoOpenBrowser: false,
      allowBrowserCookies: false, provider: 'searxng', webSearch: { allowedProviders: ['searxng'] },
      searxngBaseUrl: base, ssrf: { allowRanges: ['127.0.0.1/32'], trustEnvProxy: false },
      fetch: { defaultMode: 'raw', allowedModes: ['raw', 'readable'] }, fetchRouting: { default: ['http'] },
      githubClone: { enabled: false, clonePath: join(box.root, 'clones') },
      commands: { websearch: { enabled: false }, curator: { enabled: false }, 'google-account': { enabled: false } },
    }));
    probe = await Probe.start(box, ['-e', join(pkg, 'dist/index.js'), '--tools', ['ask_user', ...tools, 'web_enable'].join(',')]);
    await run({ probe, box, base, requests, closedResponses });
  } finally {
    if (probe) await probe.stop();
    for (const socket of sockets) socket.destroy();
    await new Promise((resolve) => server.close(resolve));
    await box.cleanup();
  }
}
async function tool(probe, name, args) {
  const after = probe.records.length;
  const done = probe.wait((r) => r.type === 'agent_settled', after);
  await probe.command('prompt', { message: `tool:${JSON.stringify({ name, arguments: args })}` });
  await done;
  const end = probe.records.slice(after).find((r) => r.type === 'tool_execution_end' && r.toolName === name);
  assert.ok(end, `Missing result for ${name}`);
  return end;
}

test('web core RPC: actual search/fetch/retrieval/source-check and retained reopen', { timeout: 30000 }, async () => webProbe(async ({ probe, box, base, requests }) => {
  const commands = (await probe.command('get_commands')).commands;
  assert.ok(commands.some((c) => c.name === 'search'));
  assert.ok(!commands.some((c) => ['websearch', 'curator', 'google-account'].includes(c.name)));
  const search = await tool(probe, 'web_search', { queries: ['proof one', 'proof two'], includeContent: false, workflow: 'none' });
  assert.equal(search.isError, false);
  assert.equal(search.result.details.successfulQueries, 2);
  const responseId = search.result.details.searchId;
  assert.ok(responseId);
  const retrieved = await tool(probe, 'get_search_content', { responseId, queryIndex: 0 });
  assert.equal(retrieved.isError, false);
  assert.match(JSON.stringify(retrieved.result), /Controlled source/);
  const fetched = await tool(probe, 'fetch_content', { url: `${base}/page`, mode: 'raw' });
  assert.equal(fetched.isError, false);
  assert.ok(fetched.result.details.responseId);
  assert.match(JSON.stringify(fetched.result), /Compatibility evidence/);
  const research = await tool(probe, 'source_check', { claim: 'Compatibility evidence', fetchContent: false });
  assert.equal(research.isError, false);
  assert.ok(research.result.details.artifact.sources.length);
  assert.ok(requests.length >= 4);
  assert.ok(!probe.records.some((r) => r.type === 'extension_ui_request' && ['select', 'input'].includes(r.method)));
  const browseAfter = probe.records.length;
  const browse = probe.command('prompt', { message: '/search' });
  const selection = await probe.wait((record) => record.type === 'extension_ui_request' && record.method === 'select', browseAfter);
  await probe.send({ type: 'extension_ui_response', id: selection.id, value: selection.options.find((option) => option.includes('proof one')) ?? selection.options[0] });
  const action = await probe.wait((record) => record.type === 'extension_ui_request' && record.method === 'select' && record.id !== selection.id, browseAfter);
  assert.ok(action.options.includes('View details'));
  await probe.send({ type: 'extension_ui_response', id: action.id, value: 'View details' });
  assert.equal((await browse).disposition, 'handled');
  assert.ok(probe.records.slice(browseAfter).some((record) => record.type === 'extension_ui_request' && record.method === 'notify' && record.message.includes('Type: search')));
  assert.equal(probe.records.slice(browseAfter).some((record) => record.type === 'agent_start'), false);
  const state = await probe.command('get_state');
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const saved = (await readFile(state.sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(saved.some((e) => e.customType === 'web-search-results'));
  const reopened = await Probe.start(box, ['--session', state.sessionFile, '-e', process.env.PI_GUI_WEB_PACKAGE ? join(process.env.PI_GUI_WEB_PACKAGE, 'dist/index.js') : join(process.env.HOME, '.pi/agent/npm/node_modules/pi-web-access/dist/index.js'), '--tools', tools.join(',')]);
  try {
    assert.equal(reopened.records.some((r) => r.type === 'agent_start'), false);
    assert.equal((await tool(reopened, 'get_search_content', { responseId: fetched.result.details.responseId, urlIndex: 0 })).isError, false);
    assert.equal((await tool(reopened, 'get_search_content', { responseId, queryIndex: 0 })).isError, false);
  } finally { await reopened.stop(); }
}));

test('web core RPC: provider failure, missing stored content, cancellation without approval', { timeout: 30000 }, async () => webProbe(async ({ probe, requests }) => {
  const failed = await tool(probe, 'web_search', { query: 'failure', workflow: 'none' });
  assert.equal(failed.isError, true);
  assert.match(JSON.stringify(failed.result), /503/);
  assert.equal((await tool(probe, 'get_search_content', { responseId: 'nonexistent' })).isError, true);
  const after = probe.records.length;
  const done = probe.wait((r) => r.type === 'agent_settled', after);
  await probe.command('prompt', { message: `tool:${JSON.stringify({ name: 'web_search', arguments: { query: 'wait', workflow: 'none' } })}` });
  const deadline = Date.now() + 5000;
  while (!requests.some((u) => u.searchParams.get('q') === 'wait') && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 20));
  assert.ok(requests.some((u) => u.searchParams.get('q') === 'wait'));
  await probe.command('clear_queue');
  await probe.command('abort');
  assert.equal((await done).aborted, true);
  assert.equal((await probe.command('get_state')).isStreaming, false);
}));

test('web background content ownership: parent abort is not fetch cleanup; orderly shutdown closes owned request', { timeout: 30000 }, async () => webProbe(async ({ probe, requests, closedResponses }) => {
  const search = await tool(probe, 'web_search', { query: 'background', includeContent: true, workflow: 'none' });
  assert.equal(search.isError, false);
  assert.ok(search.result.details.fetchId);
  const deadline = Date.now() + 5000;
  while (!requests.some((u) => u.pathname === '/pagewait') && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 20));
  assert.ok(requests.some((u) => u.pathname === '/pagewait'));
  await probe.command('abort');
  await probe.command('get_state');
  assert.equal(closedResponses.has('/pagewait'), false);
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const stopDeadline = Date.now() + 5000;
  while (!closedResponses.has('/pagewait') && Date.now() < stopDeadline) await new Promise((resolve) => setTimeout(resolve, 20));
  assert.equal(closedResponses.has('/pagewait'), true);
}));

test('web dynamic activation uses the package loader through real RPC model calls', { timeout: 30000 }, async () => webProbe(async ({ probe }) => {
  const enabled = await tool(probe, 'web_enable', {});
  assert.equal(enabled.isError, false);
  assert.deepEqual([...enabled.result.details.enabled].sort(), [...tools].sort());
  assert.equal((await tool(probe, 'web_search', { query: 'activated', workflow: 'none' })).isError, false);
}, 'dynamic'));
