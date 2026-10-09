import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertDurableRoot } from '../durable-lock.mjs';
import { assertRuntimeBuild } from '../build-lock.mjs';

const request = JSON.parse(await readFile(process.argv[2], 'utf8'));
assert.equal(new URL(request.base).hostname, '127.0.0.1');
await assertDurableRoot(request.installRoot);
await assertRuntimeBuild(request.piRoot, '@earendil-works/pi-coding-agent');
const modules = join(request.installRoot, 'node_modules');
const load = (name, file) => import(pathToFileURL(join(modules, name, file)).href);
const { Harness, createRegistry, defineTool, defineExtension, AssistantEntry } = await load('@earendil-works/pi-durable', 'dist/index.js');
const { BACKGROUND_CONTEXT: context } = await load('@earendil-works/chord', 'dist/context/index.js');
const { ModelRuntime } = await import(pathToFileURL(join(request.piRoot, 'dist/index.js')).href);
const runtime = await ModelRuntime.create({ authPath: join(process.env.PI_CODING_AGENT_DIR, 'auth.json'), modelsPath: join(process.env.PI_CODING_AGENT_DIR, 'models.json'), modelsStorePath: join(process.env.PI_CODING_AGENT_DIR, 'models-cache.json'), allowModelNetwork: false, refreshOnCreate: false });
const model = runtime.getModel('gui-loopback', 'controlled');
assert.equal(model?.baseUrl, request.base + '/v1');
let refreshes = 0;
if (request.oauth) {
  const original = runtime.getProvider('gui-loopback');
  runtime.registerNativeProvider({
    id: 'gui-oauth', name: 'Synthetic OAuth', baseUrl: model.baseUrl,
    auth: { oauth: {
      name: 'Synthetic OAuth', login: async () => { throw new Error('Login is forbidden in this probe'); },
      refresh: async (credential) => { assert.equal(credential.refresh, 'synthetic-refresh'); refreshes++; return { type: 'oauth', access: 'synthetic-refreshed', refresh: 'synthetic-rotated', expires: Date.now() + 3600000 }; },
      toAuth: async (credential) => ({ apiKey: credential.access }),
    } },
    getModels: () => [{ ...model, provider: 'gui-oauth' }],
    stream: (selected, transcript, options) => original.stream({ ...selected, provider: 'gui-loopback' }, transcript, options),
    streamSimple: (selected, transcript, options) => original.streamSimple({ ...selected, provider: 'gui-loopback' }, transcript, options),
  });
}
const registry = createRegistry();
if (request.effect) registry.install(defineExtension({ name: 'preapproved-fixture', tools: [defineTool({
  name: 'approved_effect', description: 'Explicitly preapproved disposable service effect', parameters: { type: 'object', properties: {}, additionalProperties: false },
  // Default replay policy is NOT safe. This is a fake external service/tool seam,
  // not an implementation of Durable scheduling or the future approval owner.
  execute: async (_args, _api, callContext) => {
    const response = await fetch(request.base + '/effect', { method: 'POST', body: JSON.stringify({ actionId: 'controlled-action' }), signal: callContext.abortSignal });
    return { content: [{ type: 'text', text: await response.text() }] };
  },
})] }));
const storage = request.backend === 'jsonl'
  ? await (await load('@earendil-works/pi-durable', 'dist/storage/jsonl/node.js')).openNodeJsonlStorage(request.storage, context, { fsync: true })
  : await (await load('@earendil-works/pi-durable', 'dist/storage/sqlite/node.js')).openNodeSqliteStorage(request.storage);
const harness = await Harness.open(storage, { models: runtime, registry, settings: { retry: { maxRetries: 0 }, compaction: { enabled: false }, stream: { timeoutMs: 8000 } } }, context);
const root = await harness.root(context, { agent: { model: { provider: request.oauth ? 'gui-oauth' : 'gui-loopback', modelId: 'controlled' }, ...(request.effect ? {} : { tools: [] }) } });
const emit = (data) => process.stdout.write(JSON.stringify(data) + '\n');
try {
  if (request.mode === 'inspect') {
    const inspection = await harness.inspect(context);
    const entries = await root.entries({}, 100, undefined, context);
    emit({ type: 'durable_inspection', id: root.id, scheduling: inspection.scheduling, tasks: inspection.tasks.map((item) => ({ id: item.record.id, status: item.record.state.status, abortRequested: item.record.abortRequested })), entries: entries.items.length });
  } else if (request.mode === 'retire') {
    const inspection = await harness.inspect(context);
    assert.equal(inspection.scheduling, 'paused');
    for (const task of inspection.tasks) await harness.abortTask(task.record.id, context);
    const marked = await harness.inspect(context);
    assert.ok(marked.tasks.every((item) => item.record.abortRequested));
    harness.resume(); // Only after every retained task has an explicit abort mark.
    await harness.waitForIdle(context);
    emit({ type: 'durable_retired', tasks: (await harness.inspect(context)).tasks.length });
  } else if (request.mode === 'resume') {
    harness.resume(); await harness.waitForIdle(context);
    emit({ type: 'durable_resumed', tasks: (await harness.inspect(context)).tasks.length });
  } else {
    const submission = await root.submit({ type: 'input', content: request.effect ? 'effect' : request.mode === 'hold' ? 'hold' : 'complete', requestId: request.mode }, context);
    emit({ type: 'durable_admitted', id: root.id, submissionId: submission.id });
    if (request.mode === 'hold' || request.mode === 'effect') {
      await new Promise((resolve) => { process.stdin.once('end', resolve); process.stdin.resume(); });
    } else {
      const settled = await submission.wait(context);
      assert.equal(settled.status, 'done');
      const answer = await root.commit((tx) => tx.entry(AssistantEntry, settled.answer), context);
      const duplicate = await root.submit({ type: 'input', content: 'complete', requestId: request.mode }, context);
      assert.equal(duplicate.id, submission.id);
      emit({ type: 'durable_answer', text: JSON.stringify(answer.model), refreshes, duplicate: true });
    }
  }
} finally { await harness.close(context); emit({ type: 'durable_closed' }); }
