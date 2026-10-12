// Disposable frontend process for genuine frontend-SIGKILL acceptance. It owns
// transport pipes only; the production native subreaper/guardian stays retained.
import {spawn} from 'node:child_process';
const [binary,...args]=process.argv.slice(2);
const owner=spawn(binary,args,{env:process.env,stdio:['pipe','pipe','pipe']});
process.stdin.pipe(owner.stdin);owner.stdout.pipe(process.stdout);owner.stderr.on('data',()=>{});
owner.stdin.on('error',()=>{});process.stdout.on('error',()=>owner.stdin.end());
owner.once('close',code=>{process.exitCode=code??1;process.stdin.destroy();});
