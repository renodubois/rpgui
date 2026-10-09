import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, readdir, readlink } from 'node:fs/promises';
import { join } from 'node:path';

// Hash supplied artifact bytes, including locked dependency content, without
// importing code. Paths are relative; symlink targets are hashed, never followed.
export async function durableFingerprint(root) {
  const hash = createHash('sha256');
  for (const name of ['package.json', 'package-lock.json']) hash.update(name + '\0').update(await readFile(join(root, name))).update('\0');
  async function walk(relative) {
    const entries = (await readdir(join(root, relative), { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name, 'en'));
    for (const entry of entries) {
      const path = join(relative, entry.name);
      if (entry.isDirectory()) await walk(path);
      else {
        hash.update(path + '\0');
        hash.update(entry.isSymbolicLink() ? await readlink(join(root, path)) : await readFile(join(root, path)));
        hash.update('\0');
      }
    }
  }
  await walk('node_modules');
  return hash.digest('hex');
}
export async function assertDurableRoot(root) {
  const lock = JSON.parse(await readFile(new URL('./durable-lock.json', import.meta.url), 'utf8'));
  assert.equal(process.version, lock.node);
  for (const [name, version] of Object.entries(lock.packages)) assert.equal(JSON.parse(await readFile(join(root, 'node_modules', name, 'package.json'), 'utf8')).version, version);
  assert.equal(await durableFingerprint(root), lock.sha256, 'Durable/dependency artifact drift: inspect before loading');
}
