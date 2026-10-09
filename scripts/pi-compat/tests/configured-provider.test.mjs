import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { Probe, sandbox } from '../support.mjs';

test('normal configured provider/model/settings reuse executes actual API transport with synthetic credentials and owned loopback', { timeout: 20000 }, async () => {
  const box = await sandbox({ defaultProvider: 'gui-loopback', defaultModel: 'controlled', defaultThinkingLevel: 'off' });
  const requests = []; const sockets = new Set(); let probe;
  const server = createServer(async (req, res) => {
    let body = ''; for await (const chunk of req) { body += chunk; assert.ok(body.length <= 1024 * 1024); }
    assert.equal(req.url, '/v1/chat/completions');
    assert.equal(req.headers.authorization, 'Bearer disposable-loopback-fixture');
    const input = JSON.parse(body); assert.equal(input.model, 'controlled');
    requests.push(input);
    res.writeHead(200, { 'Content-Type': 'text/event-stream' });
    res.write(`data: ${JSON.stringify({ id: 'controlled', object: 'chat.completion.chunk', choices: [{ index: 0, delta: { role: 'assistant', content: 'Configured transport proof' }, finish_reason: null }] })}\n\n`);
    res.write(`data: ${JSON.stringify({ id: 'controlled', object: 'chat.completion.chunk', choices: [{ index: 0, delta: {}, finish_reason: 'stop' }], usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } })}\n\n`);
    res.end('data: [DONE]\n\n');
  });
  server.on('connection', (socket) => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  box.loopbackBaseUrl = `http://127.0.0.1:${server.address().port}/v1`;
  try {
    const modelPath = join(box.root, 'agent/models.json');
    await writeFile(modelPath, JSON.stringify({ providers: { 'gui-loopback': { baseUrl: box.loopbackBaseUrl, api: 'openai-completions', apiKey: 'disposable-loopback-fixture', models: [{ id: 'controlled', name: 'Configured fake service', reasoning: false, input: ['text'], contextWindow: 200000, maxTokens: 4096, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }] } } }));
    const settingsBefore = await readFile(join(box.root, 'agent/settings.json'), 'utf8');
    const modelsBefore = await readFile(modelPath, 'utf8');
    probe = await Probe.start(box, [], box.cwd, { configuredProvider: true, discover: true });
    const done = probe.wait((record) => record.type === 'agent_settled');
    await probe.command('prompt', { message: 'Controlled configured service' });
    await done;
    assert.equal((await probe.command('get_last_assistant_text')).text, 'Configured transport proof');
    assert.equal(requests.length, 1);
    await probe.stop();
    assert.equal(await readFile(modelPath, 'utf8'), modelsBefore);
    assert.equal(await readFile(join(box.root, 'agent/settings.json'), 'utf8'), settingsBefore);
  } finally {
    if (probe) await probe.stop();
    for (const socket of sockets) socket.destroy();
    await new Promise((resolve) => server.close(resolve));
    await box.cleanup();
  }
});
