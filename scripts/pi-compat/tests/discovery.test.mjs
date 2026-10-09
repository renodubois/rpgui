import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { sandbox, Probe } from '../support.mjs';

function git(box, ...args) {
  const result = spawnSync('git', args, { cwd: box.cwd, env: { ...box.env, GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' }, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
}
async function absent(path) {
  await assert.rejects(access(path), { code: 'ENOENT' });
}

test('committed worktree resources load only with explicit trust; dirty files stay behind', { timeout: 30000 }, async () => {
  const box = await sandbox();
  let probe;
  try {
    for (const path of ['.pi/extensions', '.pi/prompts', '.pi/skills/proof']) await mkdir(join(box.cwd, path), { recursive: true });
    await writeFile(join(box.cwd, '.pi/extensions/proof.ts'), `import { writeFileSync } from 'node:fs';
export default function(pi) {
  writeFileSync('startup-marker', 'executed');
  pi.registerCommand('project-proof', { handler: async (_args, ctx) => { pi.appendEntry('project-proof', { cwd: ctx.cwd }); } });
}`);
    await writeFile(join(box.cwd, '.pi/prompts/project-template.md'), '---\ndescription: Disposable project prompt\n---\nFixture prompt.');
    await writeFile(join(box.cwd, '.pi/skills/proof/SKILL.md'), '---\nname: proof\ndescription: Disposable proof skill\n---\nFixture instructions.');
    git(box, 'init', '-b', 'main');
    git(box, 'add', '.pi');
    git(box, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-m', 'disposable fixture');
    await writeFile(join(box.cwd, 'dirty-user-file'), 'must not copy');
    const worktree = join(box.root, 'worktree');
    git(box, 'worktree', 'add', '-b', 'proof-thread', worktree, 'HEAD');
    await absent(join(worktree, 'dirty-user-file'));
    probe = await Probe.start(box, [], worktree, { discover: true });
    let commands = (await probe.command('get_commands')).commands;
    assert.equal(commands.some((c) => ['project-proof', 'project-template', 'skill:proof'].includes(c.name)), false);
    await absent(join(worktree, 'startup-marker'));
    assert.equal((await probe.command('get_state')).model.provider, 'gui-fixture');
    await probe.stop();
    probe = await Probe.start(box, [], worktree, { discover: true, trust: true });
    commands = (await probe.command('get_commands')).commands;
    for (const name of ['project-proof', 'project-template', 'skill:proof']) assert.ok(commands.some((c) => c.name === name), name);
    assert.equal(await readFile(join(worktree, 'startup-marker'), 'utf8'), 'executed');
    const after = probe.records.length;
    const entry = probe.wait((r) => r.type === 'entry_appended' && r.entry.customType === 'project-proof', after);
    assert.equal((await probe.command('prompt', { message: '/project-proof' })).disposition, 'handled');
    assert.equal((await entry).entry.data.cwd, worktree);
    await absent(join(box.cwd, 'startup-marker'));
  } finally {
    if (probe) await probe.stop();
    await box.cleanup();
  }
});

test('ancestor resource semantics: trusted .agents skills inherit, parent .pi extensions/settings do not', { timeout: 20000 }, async () => {
  const box = await sandbox({ steeringMode: 'all' });
  let probe;
  try {
    const nested = join(box.cwd, 'nested');
    for (const path of ['.agents/skills/ancestor-proof', '.pi/extensions', 'nested/.pi']) await mkdir(join(box.cwd, path), { recursive: true });
    await writeFile(join(box.cwd, '.agents/skills/ancestor-proof/SKILL.md'), '---\nname: ancestor-proof\ndescription: Controlled ancestor skill\n---\nFixture.');
    await writeFile(join(box.cwd, '.pi/extensions/parent.ts'), `import { writeFileSync } from 'node:fs';
export default function(pi) { writeFileSync('parent-startup-marker', 'wrong cwd resource'); pi.registerCommand('parent-only', { handler: async () => {} }); }`);
    await writeFile(join(box.cwd, '.pi/settings.json'), JSON.stringify({ steeringMode: 'one-at-a-time' }));
    await writeFile(join(nested, '.pi/settings.json'), JSON.stringify({ followUpMode: 'all' }));
    probe = await Probe.start(box, [], nested, { discover: true });
    assert.equal((await probe.command('get_commands')).commands.some((c) => c.name === 'skill:ancestor-proof'), false);
    assert.equal((await probe.command('get_state')).followUpMode, 'one-at-a-time');
    await probe.stop();
    probe = await Probe.start(box, [], nested, { discover: true, trust: true });
    const commands = (await probe.command('get_commands')).commands;
    assert.ok(commands.some((c) => c.name === 'skill:ancestor-proof'));
    assert.equal(commands.some((c) => c.name === 'parent-only'), false);
    const state = await probe.command('get_state');
    assert.equal(state.steeringMode, 'all');
    assert.equal(state.followUpMode, 'all');
    await absent(join(nested, 'parent-startup-marker'));
  } finally { if (probe) await probe.stop(); await box.cleanup(); }
});
