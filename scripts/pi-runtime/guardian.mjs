import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, realpath, writeFile, readFile, open, rename } from 'node:fs/promises';
import { join } from 'node:path';
import { StringDecoder } from 'node:string_decoder';

// Retained production process owner around the ordinary Pi CLI.
// It owns no model/tool workflow. A separate Node process survives frontend pipe
// loss, stops private-scope package runners, closes Pi, then reconciles again.
const [piEntry, subagentsPackage, root, ...args] = process.argv.slice(2);
assert.equal(await realpath(root), process.env.PI_SUBAGENTS_TEMP_ROOT);
await mkdir(root, { recursive: true, mode: 0o700 });
async function linuxIdentity(pid) {
  const fields = (await readFile(`/proc/${pid}/stat`, 'utf8')).split(') ')[1].split(' ');
  return { pid, startTicks: fields[19], bootId: (await readFile('/proc/sys/kernel/random/boot_id', 'utf8')).trim() };
}
const owner = { version: 1, scope: root, frontend:JSON.parse(process.env.RPGUI_FRONTEND_ID ?? 'null'), owner: await linuxIdentity(process.pid), pi: null };
async function persistOwner(phase) {
  const temporary = join(root, 'guardian-owner.json.next');
  const file = await open(temporary, 'wx', 0o600);
  try { await file.writeFile(JSON.stringify({ ...owner, phase })); await file.sync(); } finally { await file.close(); }
  await rename(temporary, join(root, 'guardian-owner.json'));
  const directory = await open(root, 'r');
  try { await directory.sync(); } finally { await directory.close(); }
}
await persistOwner('launching'); // Persist scope/owner identity before Pi can start.
const pi = spawn(process.execPath, [piEntry, ...args], { env: process.env, cwd: process.cwd(), stdio: ['pipe', 'pipe', 'pipe'] });
pi.stdin.on('error', () => {});
const closed = new Promise((resolve) => pi.once('close', (code, signal) => resolve({ code, signal })));
owner.pi = await linuxIdentity(pi.pid);
await persistOwner('running');
// Frontend pressure never pauses the owned Pi drain. Overflow is a protocol failure.
pi.stdout.on('data', (bytes) => {
  if (stopping || process.stdout.destroyed) return;
  if (process.stdout.writableLength + bytes.length > 4 * 1024 * 1024) { void stop('frontend-output-overflow'); return; }
  process.stdout.write(bytes);
});
pi.stderr.on('data', () => {}); // Drain diagnostics without leaking credentials or tool data.
process.stdin.pipe(pi.stdin, { end: false }); // Expose input only after identity persists.
let stopping;
process.stdout.on('error', () => { pi.stdout.unpipe(process.stdout); pi.stdout.resume(); void stop('frontend-pipe-loss'); });
process.stderr.on('error', () => { pi.stderr.unpipe(process.stderr); });
process.stdout.write(JSON.stringify({ type:'rpgui_owner_ready', owner:owner.owner, pi:owner.pi, subagents:subagentsPackage !== '-', safeCommands:JSON.parse(process.env.RPGUI_SAFE_COMMANDS??'[]') }) + '\n');
const heartbeat = setInterval(() => { if (!stopping && !process.stdout.destroyed && process.stdout.writableLength < 65536) process.stdout.write(JSON.stringify({type:'rpgui_owner_alive', owner:owner.owner}) + '\n'); },1000);
const replies = new Map();
const decoder = new StringDecoder('utf8');
let pending = '';
pi.stdout.on('data', (bytes) => {
  pending += decoder.write(bytes);
  let boundary;
  while ((boundary = pending.indexOf('\n')) >= 0) {
    const line = pending.slice(0, boundary); pending = pending.slice(boundary + 1);
    try {
      const record = JSON.parse(line);
      if (record.type === 'response') { if (record.success === false && record.id?.startsWith('rpgui-owner-')) replies.get(record.id)?.({ ...record, ownerFailed:true }); else replies.get(record.id)?.(record); }
    } catch { /* The frontend validates actual RPC output independently. */ }
  }
  if (pending.length > 2 * 1024 * 1024) pending = ''; // No unbounded auxiliary framing.
});
async function command(type) {
  const id = `rpgui-owner-${type}`;
  let timer;
  try {
    const reply = await new Promise((resolve, reject) => {
      replies.set(id, resolve);
      timer = setTimeout(() => reject(new Error(`Guardian ${type} deadline`)), 3000);
      pi.stdin.write(JSON.stringify({ id, type }) + '\n');
    });
    if (reply.success !== true) throw new Error('Owner command refused');
    return reply;
  } finally { clearTimeout(timer); replies.delete(id); }
}
async function observeScope() {
  if (subagentsPackage === '-') return { version: 1, runners: [], wholeRunProof: 'no-admitted-detached-owner' };
  const observer = spawn(process.execPath, [join(import.meta.dirname, 'scope-stop.mjs'), subagentsPackage, root], { env: process.env, stdio: ['ignore', 'pipe', 'pipe'] });
  let output = ''; let diagnostics = '';
  const deadline = setTimeout(() => observer.kill('SIGKILL'), 15000);
  observer.stdout.on('data', (bytes) => { output += bytes; if (output.length > 65536) observer.kill('SIGKILL'); });
  observer.stderr.on('data', (bytes) => { diagnostics = (diagnostics + bytes).slice(-8192); });
  try {
    return await new Promise((resolve) => observer.once('close', (code) => {
      try { resolve(code === 0 ? JSON.parse(output) : { uncertain: true, diagnostic: diagnostics }); }
      catch { resolve({ uncertain: true, diagnostic: 'Invalid guardian scope proof' }); }
    }));
  } finally { clearTimeout(deadline); }
}
let persistenceUncertain=false;
async function persistStop(phase){try{await persistOwner(phase);}catch{persistenceUncertain=true;}}
async function stop(reason) {
  if (stopping) return stopping;
  stopping = (async () => {
    clearInterval(heartbeat);
    process.stdin.unpipe(pi.stdin);
    // Do not let a dead or stalled frontend hold the authoritative parser's
    // drain state hostage while the retained owner awaits stop responses.
    pi.stdout.unpipe(process.stdout); pi.stdout.resume();
    await persistStop('clearing-admission');
    let admissionCleared = false;
    if (pi.exitCode === null && pi.signalCode === null) {
      try {
        await command('clear_queue');
        await command('abort');
        admissionCleared = true;
      } catch { /* Preserve uncertainty in the final owned-process outcome. */ }
    }
    await persistStop('initial-scope-stop');
    const initialScope = await observeScope();
    await persistStop('closing-pi');
    pi.stdin.end();
    const term = setTimeout(() => pi.kill('SIGTERM'), 3000);
    const kill = setTimeout(() => pi.kill('SIGKILL'), 5000);
    const parent = await closed;
    clearTimeout(term); clearTimeout(kill);
    // In-flight extension RPC launch handlers are not model queues. Closing
    // Pi first closes their parent-side launch authority; reconcile again to
    // include sidecars created after the first scan. Never infer whole-run
    // cleanup from this bounded runner observation.
    await persistStop('final-scope-stop');
    const scope = await observeScope();
    const descendants=await new Promise(resolve=>{const timer=setTimeout(()=>resolve({uncertain:true,reason:'Retained descendant scope deadline'}),7000);const receive=message=>{if(message?.type==='retained-children-result'){clearTimeout(timer);process.off('message',receive);resolve(message.result);}};process.on('message',receive);if(process.connected)process.send({type:'stop-retained-children'});else{clearTimeout(timer);process.off('message',receive);resolve({uncertain:true,reason:'Retained descendant scope lost'});}});
    const result = { descendants, type: 'rpgui_owner_done', reason, admissionCleared, initialScope, scope, parent };
    result.uncertain = Boolean(persistenceUncertain || descendants.uncertain || scope.uncertain || parent.code !== 0 || parent.signal || (!admissionCleared && reason !== 'pi-exit'));
    try{const proofFile=await open(join(root,'guardian-proof.json'),'w',0o600);try{await proofFile.writeFile(JSON.stringify(result));await proofFile.sync();}finally{await proofFile.close();}}catch{persistenceUncertain=true;}
    const uncertain = persistenceUncertain || descendants.uncertain || scope.uncertain || parent.code !== 0 || parent.signal || (!admissionCleared && reason !== 'pi-exit');
    result.uncertain = Boolean(uncertain);
    await persistStop(uncertain ? 'uncertain' : 'runner-observation-complete');
    if(persistenceUncertain)result.uncertain=true;
    if (!process.stdout.destroyed) process.stdout.write(JSON.stringify(result) + '\n');
    process.exitCode = result.uncertain ? 1 : 0;
    process.stdin.destroy();if(process.connected)process.disconnect();
  })();
  return stopping;
}
process.stdin.once('end', () => void stop('frontend-eof'));
pi.once('close', () => void stop('pi-exit'));
process.once('SIGTERM', () => void stop('guardian-sigterm'));
