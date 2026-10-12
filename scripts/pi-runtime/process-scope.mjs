// The native launcher sets PR_SET_CHILD_SUBREAPER before exec. Orphans remain
// children of this dedicated thread launcher, including daemonized Bash jobs.
// Signals use a native pidfd helper which checks actual PPID and start identity.
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
async function children(excluded) {
  const pids=(await readFile(`/proc/${process.pid}/task/${process.pid}/children`,'utf8')).trim().split(/\s+/).filter(Boolean).map(Number);
  const result=[];
  for(const pid of pids) {
    if(pid===excluded)continue;
    try {const fields=(await readFile(`/proc/${pid}/stat`,'utf8')).split(') ').at(-1).split(' ');if(fields[0]!=='Z' && Number(fields[1])===process.pid)result.push({pid,startTicks:fields[19]});}catch(error){if(error.code!=='ENOENT')throw error;}
  }
  return result;
}
async function signal(child,kind) {
  const helper=spawn(process.env.RPGUI_OWNER_HELPER,['--rpgui-signal-child',String(child.pid),child.startTicks,kind],{stdio:'ignore'});
  await new Promise(resolve=>{helper.once('error',resolve);helper.once('close',resolve);});
}
export async function stopRetainedChildren(guardian) {
  const start=Date.now();let count=0;const sent=new Set();
  try {
    while(Date.now()-start<5000) {
      const owned=await children(guardian);
      if(!owned.length)return {uncertain:false,proof:'retained-linux-subreaper-pidfd',observedProcesses:count};
      if(owned.length>128)return {uncertain:true,reason:'Retained child count exceeds bounded stop budget'};
      for(const child of owned){const key=`${child.pid}:${child.startTicks}`;if(!sent.has(key)||Date.now()-start>1000){if(!sent.has(key)){sent.add(key);count++;}await signal(child,Date.now()-start>1000?'KILL':'TERM');}}
      await new Promise(resolve=>setTimeout(resolve,20));
    }
    return {uncertain:true,reason:'Retained children did not reach kernel terminal state'};
  }catch{return {uncertain:true,reason:'Retained descendant scope could not be inspected'};}
}
