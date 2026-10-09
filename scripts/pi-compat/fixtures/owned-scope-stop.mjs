import assert from 'node:assert/strict';
import { lstat, readFile, readdir, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertRuntimeBuild } from '../build-lock.mjs';

// Probe-only guardian coordination. The package's public control export performs
// cancellation; no replacement executor, process search or terminal parsing.
const [pkg, requestedRoot] = process.argv.slice(2);
const root = await realpath(requestedRoot);
assert.equal(root, resolve(process.env.PI_SUBAGENTS_TEMP_ROOT));
assert.equal((await lstat(requestedRoot)).isSymbolicLink(), false);
const manifest = JSON.parse(await readFile(join(pkg, 'package.json'), 'utf8'));
assert.equal(manifest.version, '0.76.1');
assert.equal(manifest.exports['./control-channel'].default, './src/api/control-channel.js');
await assertRuntimeBuild(pkg, 'pi-subagents');
const { requestAsyncStop } = await import(pathToFileURL(join(pkg, manifest.exports['./control-channel'].default)).href);

async function scan() {
  const results = [];
  let visited = 0;
  async function visit(dir) {
    assert.ok(++visited <= 2048, 'Owned artifact directory budget exceeded');
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      assert.equal(entry.isSymbolicLink(), false, 'Never follow an artifact symlink');
      if (entry.isDirectory()) await visit(path);
      else if (entry.name === 'status.json') {
        assert.ok((await lstat(path)).size <= 1024 * 1024);
        const status = JSON.parse(await readFile(path, 'utf8'));
        assert.equal(status.lifecycleArtifactVersion, 3);
        assert.ok(typeof status.runId === 'string' && Number.isSafeInteger(status.pid) && status.pid > 0);
        results.push({ dir, runId: status.runId, pid: status.pid, state: status.state });
      }
    }
  }
  await visit(root);
  return results;
}
async function identity(pid) {
  try {
    const fields = (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].split(' ');
    return { terminal: fields[0] === 'Z', start: fields[19] };
  } catch (error) { if (error.code === 'ENOENT') return { terminal: true }; throw error; }
}
const seen = new Map();
const requested = new Set();
const deadline = Date.now() + 10000;
let quietSince;
while (Date.now() < deadline) {
  const records = await scan();
  for (const record of records) {
    const observed = await identity(record.pid);
    const previous = seen.get(record.runId);
    if (previous?.start && observed.start) assert.equal(observed.start, previous.start, 'Recorded PID identity changed');
    seen.set(record.runId, { ...record, start: previous?.start ?? observed.start, terminal: observed.terminal });
    if (record.state === 'running' && !observed.terminal && !requested.has(record.runId)) {
      requestAsyncStop(record.dir, { source: 'private-scope-guardian-probe' });
      requested.add(record.runId);
    }
  }
  if ([...seen.values()].every((record) => record.terminal)) {
    quietSince ??= Date.now();
    if (Date.now() - quietSince >= 500) {
      process.stdout.write(JSON.stringify({ version: 1, scope: root, stopped: [...requested], runners: [...seen.values()].map(({ runId, terminal }) => ({ runId, terminal })), wholeRunProof: 'not-inferred' }) + '\n');
      process.exit(0);
    }
  } else quietSince = undefined;
  await new Promise((resolve) => setTimeout(resolve, 50));
}
throw new Error('Private scope stop deadline: retain uncertain ownership, do not claim success');
