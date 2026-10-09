import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile } from 'node:fs/promises';
import { withProbe, Probe } from '../support.mjs';

const settled = (r) => r.type === 'agent_settled';
const dialog = (r) => r.type === 'extension_ui_request' && ['select', 'input'].includes(r.method);
const question = (extra = {}) => `qna:${JSON.stringify({ question: 'Choose\u2028an answer', options: [{ label: 'Same', description: 'First' }, { label: 'Same', description: 'Second' }], ...extra })}`;
async function qna(probe, answers, extra) {
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  await probe.command('prompt', { message: question(extra) });
  let cursor = after;
  for (const answer of answers) {
    const request = await probe.wait(dialog, cursor);
    cursor = probe.records.indexOf(request) + 1;
    await probe.send({ type: 'extension_ui_response', id: request.id, ...answer(request) });
  }
  await done;
  const result = probe.records.slice(after).find((r) => r.type === 'tool_execution_end' && r.toolName === 'ask_user');
  assert.ok(result);
  assert.equal(result.isError, false);
  return result.result.details;
}

test('actual RPC: LF/Unicode framing, malformed input, correlation and settled state', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const parse = probe.wait((r) => r.type === 'response' && r.command === 'parse');
  await probe.raw('{bad json}\n');
  assert.equal((await parse).success, false);
  await Promise.all([probe.command('get_state'), probe.command('get_commands'), probe.command('get_available_models')]);
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  const accepted = await probe.command('prompt', { message: 'hello' });
  assert.equal(accepted.disposition, 'started');
  assert.equal((await done).aborted, false);
  const records = probe.records.slice(after);
  assert.ok(records.findIndex((r) => r.type === 'agent_end') < records.findIndex(settled));
  const delta = records.find((r) => r.type === 'message_update' && r.assistantMessageEvent.type === 'text_delta');
  assert.equal(delta.assistantMessageEvent.delta, 'fixture\u2028answer\u2029🦀');
  assert.equal(delta.message, undefined);
  assert.equal(delta.assistantMessageEvent.partial, undefined);
  assert.equal((await probe.command('get_state')).isStreaming, false);
  assert.equal((await probe.command('get_last_assistant_text')).text, 'fixture\u2028answer\u2029🦀');
}));

test('unknown command is correlated; process crash and startup failure are surfaced', { timeout: 20000 }, async () => {
  await withProbe(async (probe) => {
    const response = probe.wait((r) => r.type === 'response' && r.id === 'invalid-command');
    await probe.send({ type: 'not_a_command', id: 'invalid-command' });
    assert.equal((await response).success, false);
    await probe.command('prompt', { message: 'hold' });
    probe.child.kill('SIGKILL'); // Only this harness-owned child.
    assert.equal((await probe.exit).signal, 'SIGKILL');
    await assert.rejects(probe.command('get_state'), /exited/);
  });
  await assert.rejects(withProbe(() => assert.fail('Invalid CLI flag must not start'), ['-z']), /exited/);
});

test('busy queues are explicit; clear before abort prevents queued replay', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  assert.equal((await probe.command('prompt', { message: 'hold' })).disposition, 'started');
  await probe.wait((r) => r.type === 'message_start' && r.message.role === 'assistant', after);
  assert.equal((await probe.command('steer', { message: 'steering' })).disposition, 'queued');
  assert.equal((await probe.command('follow_up', { message: 'later' })).disposition, 'queued');
  assert.equal((await probe.command('get_state')).pendingMessageCount, 2);
  assert.deepEqual(await probe.command('clear_queue'), { steering: ['steering'], followUp: ['later'] });
  await probe.command('abort');
  assert.equal((await done).aborted, true);
  assert.equal((await probe.command('get_state')).pendingMessageCount, 0);
  const users = (await probe.command('get_messages')).messages.filter((r) => r.role === 'user');
  assert.equal(users.length, 1);
}));

test('follow-up work completes before agent_settled', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  await probe.command('prompt', { message: 'hold' });
  await probe.wait((r) => r.type === 'message_start' && r.message.role === 'assistant', after);
  await probe.command('follow_up', { message: 'follow-up' });
  assert.equal((await probe.command('prompt', { message: '/fixture-release' })).disposition, 'handled');
  await done;
  const records = probe.records.slice(after);
  assert.ok(records.filter((r) => r.type === 'message_end' && r.message.role === 'assistant').length >= 2);
  assert.equal(records.filter(settled).length, 1);
  const users = (await probe.command('get_messages')).messages.filter((r) => r.role === 'user');
  assert.equal(users.length, 2);
}));

test('provider failure after acceptance is a message outcome', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  assert.equal((await probe.command('prompt', { message: 'provider-error' })).disposition, 'started');
  await done;
  const message = probe.records.slice(after).find((r) => r.type === 'message_end' && r.message.role === 'assistant');
  assert.equal(message.message.stopReason, 'error');
}));

test('Q+A bridge: numbered choices, descriptions and result compatibility', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const result = await qna(probe, [(r) => {
    assert.equal(r.method, 'select');
    assert.deepEqual(r.options.slice(0, 2), ['1. Same — First', '2. Same — Second']);
    return { value: r.options[1] };
  }]);
  assert.deepEqual(result, { question: 'Choose\u2028an answer', options: ['Same', 'Same'], answer: 'Same', cancelled: false, wasCustom: false, selectedIndex: 2 });
}));

test('Q+A bridge: custom answer, cancel, blank and invalid response', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const custom = await qna(probe, [(r) => ({ value: r.options.at(-1) }), (r) => {
    assert.equal(r.method, 'input'); return { value: '  My answer  ' };
  }]);
  assert.equal(custom.answer, 'My answer');
  assert.equal(custom.wasCustom, true);
  assert.equal((await qna(probe, [() => ({ cancelled: true })])).cancelled, true);
  assert.equal((await qna(probe, [() => ({ value: 'not an offered choice' })], { allowCustom: false })).cancelled, true);
  assert.equal((await qna(probe, [() => ({ value: ' ' })], { options: [] })).cancelled, true);
  assert.equal((await qna(probe, [], { options: [], allowCustom: false })).cancelled, true);
}));

test('Q+A abort unblocks; late dialog response cannot answer the next question', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const done = probe.wait(settled, after);
  await probe.command('prompt', { message: question() });
  const old = await probe.wait(dialog, after);
  await probe.command('clear_queue');
  await probe.command('abort');
  assert.equal((await done).aborted, true);
  await probe.send({ type: 'extension_ui_response', id: old.id, value: old.options[0] });
  const result = await qna(probe, [(r) => {
    assert.notEqual(r.id, old.id); return { value: r.options[1] };
  }]);
  assert.equal(result.selectedIndex, 2);
}));

test('standard dialog timeout resolves without client approval', { timeout: 20000 }, async () => withProbe(async (probe) => {
  const after = probe.records.length;
  const entry = probe.wait((r) => r.type === 'entry_appended' && r.entry.customType === 'fixture-dialog', after);
  const command = probe.command('prompt', { message: '/fixture-dialog timeout' });
  const request = await probe.wait(dialog, after);
  assert.equal(request.timeout, 25);
  assert.equal((await command).disposition, 'handled');
  assert.equal((await entry).entry.data.answer, null);
}));

test('stdin-close shutdown persists history; reopen does not start a run', { timeout: 20000 }, async () => withProbe(async (probe, box) => {
  const done = probe.wait(settled);
  await probe.command('prompt', { message: 'saved' });
  await done;
  const state = await probe.command('get_state');
  assert.ok(state.sessionFile.startsWith(box.root));
  assert.deepEqual(await probe.stop(), { code: 0, signal: null });
  const entries = (await readFile(state.sessionFile, 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(entries.some((e) => e.customType === 'fixture-shutdown'));
  const reopened = await Probe.start(box, ['--session', state.sessionFile]);
  try {
    assert.equal((await reopened.command('get_state')).sessionId, state.sessionId);
    assert.ok((await reopened.command('get_messages')).messages.some((m) => m.role === 'assistant'));
    assert.equal(reopened.records.some((r) => r.type === 'agent_start'), false);
  } finally { await reopened.stop(); }
}));
