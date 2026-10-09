import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';

// Fingerprint the installed package's own runtime modules, not its README,
// source maps, dependencies or mutable user files. Never imports package code.
export async function runtimeFingerprint(root, directory) {
  const hash = createHash('sha256');
  const paths = (await readdir(join(root, directory), { recursive: true })).filter((path) => path.endsWith('.js')).sort();
  assert.ok(paths.length);
  for (const path of paths) {
    hash.update(`${directory}/${path}\0`);
    hash.update(await readFile(join(root, directory, path)));
    hash.update('\0');
  }
  return hash.digest('hex');
}
const verified = new Set();
export async function assertRuntimeBuild(root, name) {
  const key = `${name}:${root}`;
  if (verified.has(key)) return;
  const locks = JSON.parse(await readFile(new URL('./build-lock.json', import.meta.url), 'utf8'));
  const lock = locks[name];
  assert.ok(lock, `No reviewed runtime baseline for ${name}`);
  assert.equal(await runtimeFingerprint(root, lock.directory), lock.sha256, `${name} runtime build drift: review before loading`);
  verified.add(key);
}
