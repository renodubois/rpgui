import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { assertRuntimeBuild } from './build-lock.mjs';

// Milestone-0 adapter, separate from the future Rust connector. Resource
// mechanics come from Pi's public resolver; reviews are explicit admission
// policy, not automatic claims about extension source/tool/process ownership.
export async function planResources({ piRoot, cwd, agentDir, projectTrusted = false, reviews = [], extraExtensions = [], builtinExtensions = ['llama.cpp', 'codemode', 'tool-search', 'mcp'] }) {
  assert.equal(process.env.PI_OFFLINE, '1', 'Resource inventory must run in the isolated offline preflight process');
  assert.equal(JSON.parse(await readFile(join(piRoot, 'package.json'), 'utf8')).version, '1.1.0');
  await assertRuntimeBuild(piRoot, '@earendil-works/pi-coding-agent');
  const { DefaultPackageManager, SettingsManager } = await import(pathToFileURL(join(piRoot, 'dist/index.js')).href);
  const settings = SettingsManager.create(cwd, agentDir, { projectTrusted });
  const errors = settings.drainErrors();
  if (errors.length) return { version: 1, admitted: false, blocked: errors.map((error) => ({ code: 'settings-error', scope: error.scope })), extensions: [] };
  const manager = new DefaultPackageManager({ cwd, agentDir, settingsManager: settings, builtinExtensions });
  const blocked = [];
  for (const pkg of manager.listConfiguredPackages()) {
    const installed = manager.getInstalledPath(pkg.source, pkg.scope);
    if (!installed) blocked.push({ source: pkg.source, code: 'missing-package' });
    else if (pkg.source.startsWith('npm:')) {
      const manifest = JSON.parse(await readFile(join(installed, 'package.json'), 'utf8'));
      const requestedVersion = /^npm:(?:@[^/]+\/[^@]+|[^@]+)(?:@(.+))?$/.exec(pkg.source)?.[1];
      // No unreviewed ranges, version mismatch or silent offline omission.
      const review = reviews.find((item) => item.packageName === manifest.name && item.packageVersion === manifest.version);
      if (!review || (requestedVersion && requestedVersion !== review.packageVersion)) blocked.push({ source: pkg.source, code: 'unreviewed-package-version' });
    }
  }
  if (blocked.length) return { version: 1, admitted: false, blocked, extensions: [] };
  const resources = await manager.resolve(async () => 'error'); // Never install.
  const candidates = [...resources.extensions.filter((resource) => resource.enabled).map((resource) => resource.path), ...extraExtensions];
  const selected = [];
  const replaced = [];
  const tools = new Map();
  for (const path of new Set(candidates)) {
    const review = reviews.find((item) => item.path === path);
    if (!review || !['no-background-work', 'private-scope-stop', 'qna-replacement', 'qna-original'].includes(review.contract)) {
      blocked.push({ path, code: 'unreviewed-owner' }); continue;
    }
    if (!path.startsWith('builtin:')) {
      const hash = createHash('sha256').update(await readFile(path)).digest('hex');
      if (hash !== review.sha256) { blocked.push({ path, code: 'reviewed-resource-drift' }); continue; }
    }
    if (review.contract === 'qna-original') { replaced.push({ path, reason: 'explicit-native-qna-replacement' }); continue; }
    for (const name of review.toolNames ?? []) {
      if (tools.has(name)) blocked.push({ path, code: 'duplicate-tool', toolName: name });
      else tools.set(name, path);
    }
    selected.push(path);
  }
  if (replaced.length && !selected.some((path) => reviews.find((review) => review.path === path)?.contract === 'qna-replacement')) blocked.push({ code: 'missing-qna-replacement' });
  return { version: 1, admitted: blocked.length === 0, blocked, extensions: blocked.length ? [] : selected, replaced, resources };
}
