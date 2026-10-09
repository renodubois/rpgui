import { spawn } from 'node:child_process';

// A disposable frontend transport owner. Killing this process really closes
// every frontend pipe; its retained guardian must independently finish cleanup.
const guardian = spawn(process.execPath, process.argv.slice(2), { env: process.env, cwd: process.cwd(), stdio: ['pipe', 'pipe', 'pipe'] });
process.stdin.pipe(guardian.stdin);
guardian.stdout.pipe(process.stdout);
guardian.stderr.pipe(process.stderr);
guardian.stdin.on('error', () => {});
guardian.once('close', (code) => { process.exitCode = code ?? 1; process.stdin.destroy(); });
