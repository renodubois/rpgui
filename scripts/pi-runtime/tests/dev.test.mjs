import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import test from 'node:test';

// Test the shipped stop helper with disposable Node processes, not a native GUI.
test('development frontend stop preserves inherited-group retained owners', async () => {
  const source = await readFile(new URL('../../../dev.sh', import.meta.url), 'utf8');
  const helper = source.match(/^stop_client\(\) \{\n[\s\S]*?^\}/m)?.[0];
  assert.ok(helper, 'production frontend-specific stop helper');
  assert.equal((source.match(/stop_client "\$client_pid"/g) ?? []).length, 2);
  assert.ok(!source.includes('stop_process "$client_pid"'));
  const frontend = spawn(process.execPath, ['-e', `
    const {spawn}=require('node:child_process');
    const owner=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});
    console.log(owner.pid);
    setInterval(()=>{},1000);
  `], { detached: true, stdio: ['ignore', 'pipe', 'ignore'] });
  let owner;
  try {
    const [bytes] = await once(frontend.stdout, 'data');
    owner = Number(String(bytes).trim());
    assert.ok(Number.isInteger(owner) && owner > 0);
    const stop = spawn('bash', ['-c', `${helper}\nstop_client "$1"`, 'test', String(frontend.pid)], { stdio: 'ignore' });
    const [code] = await once(stop, 'close');
    assert.equal(code, 0);
    assert.ok(frontend.exitCode !== null || frontend.signalCode !== null);
    assert.doesNotThrow(() => process.kill(owner, 0), 'retained owner must survive frontend stop');
  } finally {
    if (frontend.exitCode === null && frontend.signalCode === null) frontend.kill('SIGKILL');
    if (owner) { try { process.kill(owner, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; } }
  }
});
