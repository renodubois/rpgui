// Production preflight + retained owner. Does not install packages or alter ordinary Pi settings.
import { readFile, realpath, mkdir } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { stopRetainedChildren } from './process-scope.mjs';
import { planResources } from './admission.mjs';
import { reviewedCatalog, validateSelected } from './catalog.mjs';

const [requestPath] = process.argv.slice(2);
let preflightScope;
async function refuseMarker() { if(preflightScope) { const file = await import('node:fs/promises'); await file.writeFile(join(preflightScope,'owner-refusal.json'),JSON.stringify({version:1,admitted:false}),{mode:0o600}); } }
try {
  if (process.version !== 'v24.13.0') throw new Error('Unsupported Node runtime; reviewed baseline is v24.13.0');
  if(!process.env.RPGUI_OWNER_HELPER)throw new Error('Unsupported launch without retained Linux descendant scope');
  const request = JSON.parse(await readFile(requestPath, 'utf8'));
  const { piRoot, cwd, agentDir, sessionDir, session, scope } = request;
  for (const path of [piRoot, cwd, agentDir, sessionDir, scope]) {
    if (typeof path !== 'string' || !path.startsWith('/')) throw new Error('Invalid launch path');
    if (await realpath(path) !== path) throw new Error('Launch paths must be canonical');
  }
  preflightScope = scope;
  const repository = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
  const subagentsPackage = process.env.PI_GUI_SUBAGENTS_PACKAGE ?? join(agentDir,'npm/node_modules/pi-subagents');
  const webPackage = process.env.PI_GUI_WEB_PACKAGE ?? join(agentDir,'npm/node_modules/pi-web-access');
  const exists = async path => { try { await readFile(join(path,'package.json')); return path; } catch(e) { if(e.code === 'ENOENT') return undefined; throw e; } };
  const subagents = await exists(subagentsPackage); const web = await exists(webPackage);
  const { reviews, replacement, guard } = await reviewedCatalog({ agentDir, repository, subagentsPackage:subagents, webPackage:web });
  process.env.PI_OFFLINE = '1';
  const inventory = await planResources({ piRoot, cwd, agentDir, projectTrusted: request.projectTrusted === true, reviews, extraExtensions: [replacement, guard] });
  if (!inventory.admitted) {
    process.stdout.write(JSON.stringify({ type: 'rpgui_admission_refused', blocked: inventory.blocked }) + '\n');
    await refuseMarker();
    process.exitCode = 1;
  } else {
    await validateSelected({ extensions:inventory.extensions, agentDir, subagentsPackage:subagents, webPackage:web });
    const hasSubagents = subagents && inventory.extensions.includes(join(subagents,'index.js'));
    await mkdir(join(scope, 'web-cache'), { recursive:true, mode:0o700 });
    await mkdir(join(scope, 'tmp'), { recursive: true, mode: 0o700 });
    const args = [join(piRoot, 'dist/bundle/cli.js'), hasSubagents ? subagents : '-', scope, '--mode', 'rpc', '--session-dir', sessionDir,
      request.projectTrusted === true ? '--approve' : '--no-approve', '--no-extensions', '--no-mcp', ...inventory.extensions.flatMap(path => ['-e', path])];
    if (session) args.push('--session', session);
    const child = spawn(process.execPath, [join(fileURLToPath(new URL('.', import.meta.url)), 'guardian.mjs'), ...args], {
      cwd, env: { ...process.env, PI_CODING_AGENT_DIR: agentDir, PI_SUBAGENTS_TEMP_ROOT: scope, TMPDIR: join(scope, 'tmp'), PI_WEB_ACCESS_CACHE_ROOT:join(scope,'web-cache'), RPGUI_SUBAGENTS_PACKAGE:hasSubagents ? subagents : '', RPGUI_ADMITTED_EXTENSIONS:JSON.stringify(inventory.extensions), RPGUI_FRONTEND_ID:JSON.stringify(request.frontend ?? null),RPGUI_SAFE_COMMANDS:JSON.stringify([{name:'rpgui-subagents',root:guard},{name:'*',root:'builtin:tool-search'},...(web && inventory.extensions.some(path=>path.startsWith(web+'/'))?[{name:'search',root:web}]:[])]) },
      stdio: ['pipe', 'pipe', 'pipe','ipc'],
    });
    child.on('message',async message=>{if(message?.type==='stop-retained-children'){const result=await stopRetainedChildren(child.pid);if(child.connected)child.send({type:'retained-children-result',result});}});
    // The launcher is a transport relay; the guardian retains execution ownership after relay/frontend loss.
    process.stdin.pipe(child.stdin); child.stdout.pipe(process.stdout); child.stderr.on('data', () => {});
    child.stdin.on('error', () => {});
    process.stdout.on('error', () => { child.stdout.resume(); child.stdin.end(); });
    child.on('error', () => { process.exitCode = 1; process.stdin.destroy(); });
    child.on('close',async code=>{if(code!==0)await stopRetainedChildren(0);process.exitCode=code??1;process.stdin.destroy();});
  }
} catch (error) {
  await refuseMarker().catch(() => {});
  process.stdout.write(JSON.stringify({ type: 'rpgui_admission_refused', blocked: [{ code: 'preflight-failed', explanation: /^(Unsupported|Only audited|Subagents requires|Unaudited|Web version drift|Subagents version drift)/.test(error?.message ?? '') ? error.message : 'Invalid runtime, settings, or resource build. Refusing launch before extensions start.' }] }) + '\n');
  process.exitCode = 1;
}
