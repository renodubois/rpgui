import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

// Disposable work invoked through Pi's production bash execution. Finite even
// if cancellation fails; no remote service, accounts or daemonized processes.
const child = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 8000)'], { stdio: 'ignore' });
writeFileSync(process.argv[2], JSON.stringify({ parent: process.pid, child: child.pid }));
await new Promise((resolve) => child.once('close', resolve));
