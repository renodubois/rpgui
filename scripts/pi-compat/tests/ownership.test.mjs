import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { Probe, sandbox } from '../support.mjs';

// The kernel flock is real; this is an admission experiment, not GUI storage code.
function lock(path) {
  const child = spawn('flock', ['--no-fork', '--exclusive', '--nonblock', '--conflict-exit-code', '73', path, process.execPath, '-e', 'process.stdout.write("owned\\n"); process.stdin.resume();'], { stdio: ['pipe', 'pipe', 'pipe'] });
  const exit = new Promise((resolve, reject) => { child.once('error', reject); child.once('close', (code) => resolve(code)); });
  const ready = new Promise((resolve, reject) => {
    child.stdout.once('data', () => resolve(true));
    child.once('error', reject);
    child.once('close', (code) => resolve(code === 0));
  });
  return { child, ready, exit };
}

test('profile admission: kernel lock excludes a second process and releases after owner death', { timeout: 15000 }, async () => {
  const box = await sandbox();
  const owned = [];
  try {
    const path = join(box.root, 'gui-profile.lock');
    const first = lock(path); owned.push(first);
    assert.equal(await first.ready, true);
    const second = lock(path); owned.push(second);
    assert.equal(await second.exit, 73);
    first.child.kill('SIGKILL'); // --no-fork: this PID is the owned lock holder.
    await first.exit;
    const third = lock(path); owned.push(third);
    assert.equal(await third.ready, true);
    third.child.stdin.end();
    assert.equal(await third.exit, 0);
  } finally {
    for (const owner of owned) owner.child.stdin.end();
    await Promise.all(owned.map((owner) => owner.exit));
    await box.cleanup();
  }
});

test('saved-session ownership: ordinary Pi permits duplicate writable open; private copy preserves cwd and source', { timeout: 20000 }, async () => {
  const box = await sandbox();
  const probes = [];
  try {
    const original = await Probe.start(box); probes.push(original);
    const done = original.wait((r) => r.type === 'agent_settled');
    await original.command('prompt', { message: 'saved original' });
    await done;
    const state = await original.command('get_state');
    const competing = await Probe.start(box, ['--session', state.sessionFile]); probes.push(competing);
    assert.equal((await competing.command('get_state')).sessionFile, state.sessionFile);
    assert.equal((await competing.command('get_state')).sessionId, state.sessionId);
    await competing.stop();
    await original.stop();
    // Copy only a quiescent owned source in this experiment. This does not make
    // copying an actively changing unknown terminal file safe or atomic.
    const before = await readFile(state.sessionFile, 'utf8');
    const copied = join(box.root, 'sessions/imported.jsonl');
    await copyFile(state.sessionFile, copied);
    const otherCwd = join(box.root, 'different-project');
    await mkdir(otherCwd);
    const reopened = await Probe.start(box, ['--session', copied], otherCwd); probes.push(reopened);
    assert.equal(reopened.records.some((r) => r.type === 'agent_start'), false);
    assert.equal((await reopened.command('get_state')).sessionFile, copied);
    const after = reopened.records.length;
    const entry = reopened.wait((r) => r.type === 'entry_appended' && r.entry.customType === 'fixture-cwd', after);
    await reopened.command('prompt', { message: '/fixture-cwd' });
    assert.equal((await entry).entry.data.cwd, box.cwd);
    await reopened.stop();
    assert.equal(await readFile(state.sessionFile, 'utf8'), before);
    assert.equal(JSON.parse(before.split('\n')[0]).cwd, box.cwd);
  } finally {
    for (const probe of probes) await probe.stop();
    await box.cleanup();
  }
});

test('normal synthetic configuration excludes original Q+A before startup and retains other global resources', { timeout: 20000 }, async () => {
  const box = await sandbox();
  let probe;
  try {
    for (const path of ['agent/extensions', 'agent/prompts', 'agent/skills/global-proof']) await mkdir(join(box.root, path), { recursive: true });
    await writeFile(join(box.root, 'agent/extensions/qna-ui.ts'), `import { writeFileSync } from 'node:fs';
export default function(pi) { writeFileSync('forbidden-qna-startup', 'conflict'); pi.registerCommand('conflicting-qna', { handler: async () => {} }); }`);
    await writeFile(join(box.root, 'agent/extensions/keep.ts'), `export default function(pi) { pi.registerCommand('kept-global', { handler: async () => {} }); }`);
    await writeFile(join(box.root, 'agent/prompts/global-template.md'), '---\ndescription: Controlled global template\n---\nFixture.');
    await writeFile(join(box.root, 'agent/skills/global-proof/SKILL.md'), '---\nname: global-proof\ndescription: Controlled global skill\n---\nFixture.');
    const settingsPath = join(box.root, 'agent/settings.json');
    const settings = JSON.parse(await readFile(settingsPath, 'utf8'));
    await writeFile(settingsPath, JSON.stringify({ ...settings, extensions: ['-extensions/qna-ui.ts'], defaultThinkingLevel: 'off' }));
    const before = await readFile(settingsPath, 'utf8');
    probe = await Probe.start(box, [], box.cwd, { discover: true });
    const commands = (await probe.command('get_commands')).commands;
    for (const name of ['kept-global', 'global-template', 'skill:global-proof']) assert.ok(commands.some((c) => c.name === name), name);
    assert.equal(commands.some((c) => c.name === 'conflicting-qna'), false);
    await assert.rejects(readFile(join(box.cwd, 'forbidden-qna-startup')), { code: 'ENOENT' });
    const after = probe.records.length;
    const done = probe.wait((r) => r.type === 'agent_settled', after);
    await probe.command('prompt', { message: 'qna:{"question":"Configured?","options":[{"label":"Yes"}],"allowCustom":false}' });
    const request = await probe.wait((r) => r.type === 'extension_ui_request' && r.method === 'select', after);
    await probe.send({ type: 'extension_ui_response', id: request.id, value: request.options[0] });
    await done;
    assert.equal(probe.records.slice(after).find((r) => r.type === 'tool_execution_end').result.details.answer, 'Yes');
    await probe.stop();
    assert.equal(await readFile(settingsPath, 'utf8'), before);
    // GUI-scoped reviewed allowlist alternative: original user config need not
    // contain an exclusion. Normal non-extension discovery remains enabled.
    await writeFile(settingsPath, JSON.stringify({ ...settings, defaultThinkingLevel: 'off' }));
    const unfiltered = await readFile(settingsPath, 'utf8');
    probe = await Probe.start(box, ['--no-extensions', '-e', join(box.root, 'agent/extensions/keep.ts')], box.cwd, { discover: true });
    const filteredCommands = (await probe.command('get_commands')).commands;
    for (const name of ['kept-global', 'global-template', 'skill:global-proof']) assert.ok(filteredCommands.some((c) => c.name === name), name);
    assert.equal(filteredCommands.some((c) => c.name === 'conflicting-qna'), false);
    await assert.rejects(readFile(join(box.cwd, 'forbidden-qna-startup')), { code: 'ENOENT' });
    await probe.stop();
    assert.equal(await readFile(settingsPath, 'utf8'), unfiltered);
  } finally { if (probe) await probe.stop(); await box.cleanup(); }
});
