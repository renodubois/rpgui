import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test } from 'node:test';
import { piPackage, Probe, repository, sandbox } from '../support.mjs';

async function review(path, contract, toolNames = []) {
  return { path, contract, toolNames, sha256: createHash('sha256').update(await readFile(path)).digest('hex') };
}
async function plan(box, request) {
  const path = join(box.root, 'resource-request.json');
  await writeFile(path, JSON.stringify({ piRoot: await piPackage(), cwd: box.cwd, agentDir: join(box.root, 'agent'), builtinExtensions: [], ...request }));
  const result = spawnSync(process.execPath, [join(repository, 'scripts/pi-compat/fixtures/resource-admission-driver.mjs'), path], { env: box.env, cwd: box.cwd, encoding: 'utf8', timeout: 10000, maxBuffer: 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test('public resource inventory replaces reviewed Q+A before package/project startup, preserving supported discovery', { timeout: 30000 }, async () => {
  const box = await sandbox(); let probe;
  try {
    const pkg = join(box.root, 'local-package');
    for (const path of [pkg, join(box.cwd, '.pi/extensions'), join(box.root, 'agent/prompts')]) await mkdir(path, { recursive: true });
    const qna = join(pkg, 'qna.ts'); const kept = join(pkg, 'kept.ts'); const projectQna = join(box.cwd, '.pi/extensions/qna.ts');
    const original = `import { writeFileSync } from 'node:fs'; export default function(pi) { writeFileSync('original-qna-started', 'unsafe conflict'); pi.registerCommand('original-qna', { handler: async () => {} }); pi.registerTool({ name: 'ask_user', label: 'Original Q+A', description: 'Controlled conflicting Q+A', parameters: { type: 'object', properties: {} }, async execute() { return { content: [{ type: 'text', text: 'Not the native bridge' }], details: {} }; } }); }`;
    await writeFile(qna, original); await writeFile(projectQna, original);
    await writeFile(kept, `export default function(pi) { pi.registerCommand('kept-package', { handler: async () => {} }); }`);
    await writeFile(join(pkg, 'package.json'), JSON.stringify({ name: 'controlled-resources', pi: { extensions: ['./qna.ts', './kept.ts'] } }));
    await writeFile(join(box.root, 'agent/prompts/kept-template.md'), '---\ndescription: Controlled template\n---\nRetained.');
    const settingsPath = join(box.root, 'agent/settings.json');
    const settings = JSON.parse(await readFile(settingsPath, 'utf8'));
    await writeFile(settingsPath, JSON.stringify({ ...settings, packages: [{ source: pkg }] }));
    const before = await readFile(settingsPath, 'utf8');
    const replacement = join(repository, 'extensions/pi-gui-qna.ts');
    const reviews = [await review(qna, 'qna-original', ['ask_user']), await review(projectQna, 'qna-original', ['ask_user']), await review(kept, 'no-background-work'), await review(replacement, 'qna-replacement', ['ask_user'])];
    const untrusted = await plan(box, { reviews, extraExtensions: [replacement] });
    assert.equal(untrusted.admitted, true);
    assert.equal(untrusted.resources.extensions.some((resource) => resource.path === projectQna), false);
    const trusted = await plan(box, { reviews, extraExtensions: [replacement], projectTrusted: true });
    assert.equal(trusted.admitted, true);
    assert.equal(trusted.replaced.length, 2);
    assert.ok(trusted.extensions.includes(kept));
    await assert.rejects(readFile(join(box.cwd, 'original-qna-started')), { code: 'ENOENT' });
    probe = await Probe.start(box, ['--no-extensions', ...trusted.extensions.flatMap((path) => ['-e', path])], box.cwd, { discover: true, trust: true });
    const commands = (await probe.command('get_commands')).commands;
    assert.ok(commands.some((command) => command.name === 'kept-package'));
    assert.ok(commands.some((command) => command.name === 'kept-template'));
    assert.equal(commands.some((command) => command.name === 'original-qna'), false);
    const after = probe.records.length;
    const settled = probe.wait((record) => record.type === 'agent_settled', after);
    await probe.command('prompt', { message: 'qna:{"question":"Native path?","options":[{"label":"Yes"}],"allowCustom":false}' });
    const dialog = await probe.wait((record) => record.type === 'extension_ui_request' && record.method === 'select', after);
    await probe.send({ type: 'extension_ui_response', id: dialog.id, value: dialog.options[0] });
    await settled;
    assert.equal(probe.records.slice(after).find((record) => record.type === 'tool_execution_end').result.details.answer, 'Yes');
    await assert.rejects(readFile(join(box.cwd, 'original-qna-started')), { code: 'ENOENT' });
    await probe.stop();
    assert.equal(await readFile(settingsPath, 'utf8'), before);
  } finally { if (probe) await probe.stop(); await box.cleanup(); }
});

test('inventory includes pinned CLI built-ins and honors configured exclusions without silently treating unknown owners as safe', { timeout: 20000 }, async () => {
  const box = await sandbox();
  const builtinExtensions = ['llama.cpp', 'codemode', 'tool-search', 'mcp'];
  try {
    let result = await plan(box, { builtinExtensions });
    assert.equal(result.admitted, false);
    assert.deepEqual(result.resources.extensions.filter((resource) => resource.enabled).map((resource) => resource.path).sort(), builtinExtensions.map((name) => `builtin:${name}`).sort());
    assert.equal(result.blocked.filter((record) => record.code === 'unreviewed-owner').length, 4);
    const settingsPath = join(box.root, 'agent/settings.json');
    const settings = JSON.parse(await readFile(settingsPath, 'utf8'));
    await writeFile(settingsPath, JSON.stringify({ ...settings, extensions: ['-builtin:mcp', '-builtin:llama.cpp'] }));
    const before = await readFile(settingsPath, 'utf8');
    result = await plan(box, { builtinExtensions, reviews: ['codemode', 'tool-search'].map((name) => ({ path: `builtin:${name}`, contract: 'no-background-work' })) });
    assert.equal(result.admitted, true);
    assert.deepEqual(result.extensions.sort(), ['builtin:codemode', 'builtin:tool-search']);
    assert.equal(await readFile(settingsPath, 'utf8'), before);
  } finally { await box.cleanup(); }
});

test('admission refuses missing packages, unreviewed owners, duplicate tools and source drift without startup or installation', { timeout: 30000 }, async () => {
  const box = await sandbox();
  try {
    const extensionRoot = join(box.root, 'agent/extensions'); await mkdir(extensionRoot);
    const unknown = join(extensionRoot, 'unknown.ts');
    await writeFile(unknown, `import { writeFileSync } from 'node:fs'; export default function() { writeFileSync('unreviewed-started', 'must not execute'); }`);
    let result = await plan(box, { reviews: [] });
    assert.equal(result.admitted, false); assert.ok(result.blocked.some((record) => record.code === 'unreviewed-owner'));
    const first = await review(unknown, 'no-background-work', ['ask_user']);
    const replacement = join(repository, 'extensions/pi-gui-qna.ts');
    result = await plan(box, { reviews: [first, await review(replacement, 'qna-replacement', ['ask_user'])], extraExtensions: [replacement] });
    assert.equal(result.admitted, false); assert.ok(result.blocked.some((record) => record.code === 'duplicate-tool'));
    await writeFile(unknown, '// Changed after review\n' + await readFile(unknown, 'utf8'));
    result = await plan(box, { reviews: [first] });
    assert.ok(result.blocked.some((record) => record.code === 'reviewed-resource-drift'));
    const settingsPath = join(box.root, 'agent/settings.json'); const settings = JSON.parse(await readFile(settingsPath, 'utf8'));
    await writeFile(settingsPath, JSON.stringify({ ...settings, packages: ['npm:uninstalled-controlled-proof@1.0.0'] }));
    const before = await readFile(settingsPath, 'utf8');
    result = await plan(box, { reviews: [] });
    assert.equal(result.admitted, false); assert.ok(result.blocked.some((record) => record.code === 'missing-package'));
    assert.equal(await readFile(settingsPath, 'utf8'), before);
    assert.equal((await readdir(join(box.root, 'agent'))).includes('npm'), false);
    await assert.rejects(readFile(join(box.cwd, 'unreviewed-started')), { code: 'ENOENT' });
  } finally { await box.cleanup(); }
});
