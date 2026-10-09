import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { StringDecoder } from 'node:string_decoder';
import { assertRuntimeBuild } from './build-lock.mjs';

export const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
export async function piPackage() {
  if (process.env.PI_GUI_PI_PACKAGE) return resolve(process.env.PI_GUI_PI_PACKAGE);
  const found = spawnSync('which', ['pi'], { encoding: 'utf8' });
  assert.equal(found.status, 0, 'Install Pi 1.1.0 or set PI_GUI_PI_PACKAGE to its package directory');
  let directory = dirname(await realpath(found.stdout.trim()));
  for (let depth = 0; depth < 5; depth++, directory = dirname(directory)) {
    try {
      const pkg = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'));
      if (pkg.name === '@earendil-works/pi-coding-agent') return directory;
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  throw new Error('Cannot locate the Pi package; set PI_GUI_PI_PACKAGE');
}

export async function sandbox(settings = {}) {
  const root = await mkdtemp(join(tmpdir(), 'pi-gui-proof-'));
  for (const path of ['home', 'agent', 'project', 'config', 'cache', 'sessions', 'tmp']) await mkdir(join(root, path));
  await writeFile(join(root, 'agent/settings.json'), JSON.stringify({ retry: { enabled: false }, compaction: { enabled: false }, cacheWarming: 'off', defaultProjectTrust: 'never', ...settings }));
  // Allowlist, not process.env: do not inherit provider keys, proxy, runtime hooks,
  // session markers, desktop buses or extension settings. This is not a sandbox.
  const env = { PATH: process.env.PATH, TMPDIR: join(root, 'tmp'), HOME: join(root, 'home'), XDG_CONFIG_HOME: join(root, 'config'), XDG_CACHE_HOME: join(root, 'cache'), PI_CODING_AGENT_DIR: join(root, 'agent'), PI_CODING_AGENT_SESSION_DIR: join(root, 'sessions'), PI_OFFLINE: '1', PI_SKIP_VERSION_CHECK: '1', PI_TELEMETRY: '0', LANG: 'C.UTF-8' };
  return { root, env, cwd: join(root, 'project'), cleanup: () => rm(root, { recursive: true, force: true }) };
}

/** Probe-only JSONL client; not the future Rust connector. Retention is bounded. */
export class Probe {
  static async start(box, extra = [], cwd = box.cwd, { discover = false, trust = false, guardianPackage, guardianFrontend = false, configuredProvider = false } = {}) {
    const pkg = await piPackage();
    const metadata = JSON.parse(await readFile(join(pkg, 'package.json'), 'utf8'));
    assert.equal(metadata.version, '1.1.0', 'Protocol baseline changed: review before repinning');
    assert.equal(process.version, 'v24.13.0', 'Review Node runtime drift before repinning');
    await assertRuntimeBuild(pkg, '@earendil-works/pi-coding-agent');
    const piEntry = join(pkg, typeof metadata.bin === 'string' ? metadata.bin : metadata.bin.pi);
    const launch = guardianPackage ? [join(repository, 'scripts/pi-compat/fixtures/guarded-cli.mjs'), piEntry, guardianPackage, box.env.PI_SUBAGENTS_TEMP_ROOT] : [piEntry];
    if (guardianFrontend) launch.unshift(join(repository, 'scripts/pi-compat/fixtures/frontend-driver.mjs'));
    const probe = new Probe(spawn(process.execPath, [...launch, '--mode', 'rpc', '--offline', ...(discover ? [] : ['--no-extensions', '--no-skills', '--no-prompt-templates']), '--no-mcp', '--no-themes', '--no-context-files', trust ? '--approve' : '--no-approve', '--session-dir', join(box.root, 'sessions'), ...(configuredProvider ? [] : ['--provider', 'gui-fixture', '--model', 'controlled']), '--tools', 'ask_user', '-e', join(repository, 'scripts/pi-compat/fixtures/provider.ts'), '-e', join(repository, 'extensions/pi-gui-qna.ts'), ...extra], { cwd, env: box.env, stdio: ['pipe', 'pipe', 'pipe'] }));
    try {
      const state = await probe.command('get_state');
      assert.equal(state.model?.provider, configuredProvider ? 'gui-loopback' : 'gui-fixture', 'Never prompt a non-fixture provider');
      assert.equal(state.model?.id, 'controlled');
      if (configuredProvider) {
        assert.ok(box.loopbackBaseUrl && new URL(box.loopbackBaseUrl).hostname === '127.0.0.1');
        assert.equal(state.model.baseUrl, box.loopbackBaseUrl, 'Never prompt a configured provider outside the owned loopback listener');
      }
      return probe;
    }
    catch (error) { await probe.stop(); throw error; }
  }
  constructor(child) {
    this.child = child;
    this.records = [];
    this.recordBytes = 0;
    this.waiters = new Set();
    this.nextId = 0;
    this.stderr = '';
    this.failure = undefined;
    this.exit = new Promise((resolve) => child.once('close', (code, signal) => {
      this.closed = { code, signal };
      this.fail(new Error('Owned Pi process exited'));
      resolve(this.closed);
    }));
    child.on('error', (error) => this.fail(error));
    child.stdin.on('error', (error) => this.fail(error));
    child.stderr.on('data', (bytes) => { this.stderr = (this.stderr + bytes.toString()).slice(-8192); });
    const decoder = new StringDecoder('utf8');
    let pending = '';
    child.stdout.on('data', (bytes) => {
      pending += decoder.write(bytes);
      let boundary;
      while ((boundary = pending.indexOf('\n')) >= 0) {
        const line = pending.slice(0, boundary).replace(/\r$/, '');
        pending = pending.slice(boundary + 1);
        try {
          const bytes = Buffer.byteLength(line, 'utf8');
          if (bytes > 2 * 1024 * 1024) throw new Error('Probe frame budget exceeded');
          if (this.records.length >= 10000 || this.recordBytes + bytes > 16 * 1024 * 1024) throw new Error('Probe record budget exceeded');
          const record = JSON.parse(line);
          this.recordBytes += bytes;
          this.records.push(record);
          for (const waiter of this.waiters) waiter.check();
        } catch (error) { this.fail(error); }
      }
      if (Buffer.byteLength(pending, 'utf8') > 2 * 1024 * 1024) { this.fail(new Error('Probe frame budget exceeded')); pending = ''; }
    });
    child.stdout.on('end', () => {
      if (pending || decoder.end()) this.fail(new Error('Incomplete final JSONL record'));
    });
  }
  fail(error) {
    this.failure ??= error;
    for (const waiter of this.waiters) waiter.check();
  }
  async raw(text) {
    if (this.failure) throw this.failure;
    await new Promise((resolve, reject) => this.child.stdin.write(text, (error) => error ? reject(error) : resolve()));
  }
  send(record) { return this.raw(`${JSON.stringify(record)}\n`); }
  wait(predicate, after = 0, timeout = 10000) {
    return new Promise((resolve, reject) => {
      const finish = (error, value) => { clearTimeout(timer); this.waiters.delete(waiter); error ? reject(error) : resolve(value); };
      const waiter = { check: () => {
        const value = this.records.slice(after).find(predicate);
        if (value) finish(undefined, value);
        else if (this.failure) finish(this.failure);
      } };
      const timer = setTimeout(() => finish(new Error('Probe deadline exceeded')), timeout);
      this.waiters.add(waiter);
      waiter.check();
    });
  }
  async command(type, fields = {}) {
    if (this.failure) throw this.failure;
    const id = `probe-${++this.nextId}`;
    const response = this.wait((r) => r.type === 'response' && r.id === id);
    // An exit/write failure can reject the waiter before the write is awaited.
    void response.catch(() => {});
    await this.send({ ...fields, type, id });
    const result = await response;
    assert.equal(result.command, type);
    assert.equal(result.success, true, result.error);
    return result.data;
  }
  async stop() {
    if (this.closed) return this.closed;
    this.child.stdin.end();
    const term = setTimeout(() => this.child.kill('SIGTERM'), 3000);
    const kill = setTimeout(() => this.child.kill('SIGKILL'), 5000);
    try { return await this.exit; } finally { clearTimeout(term); clearTimeout(kill); }
  }
}

export async function withProbe(run, extra = [], settings = {}) {
  const box = await sandbox(settings);
  let probe;
  try { probe = await Probe.start(box, extra); await run(probe, box); }
  finally { if (probe) await probe.stop(); await box.cleanup(); }
}
