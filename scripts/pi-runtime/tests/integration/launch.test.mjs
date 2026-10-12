import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { Probe, sandbox, piPackage, repository } from '../../../pi-compat/support.mjs';
import { subagentDisabled } from '../../catalog.mjs';

async function setup({subagents=false,web=false}={}) {
  const box = await sandbox({defaultProvider:'gui-loopback',defaultModel:'controlled',defaultThinkingLevel:'off',extensions:['-builtin:mcp','-builtin:llama.cpp','-builtin:codemode']});
  const requests=[]; const sockets=new Set();
  const server=createServer(async(req,res) => {
    if(req.url.startsWith('/search')) { res.setHeader('Content-Type','application/json'); res.end(JSON.stringify({results:[{title:'Persisted source',url:`http://127.0.0.1:${server.address().port}/page`,content:'loopback evidence'}]})); return; }
    if(req.url === '/page') { res.setHeader('Content-Type','text/html'); res.end('<article>Persisted local evidence</article>'); return; }
    let bytes=''; for await(const chunk of req) { bytes+=chunk; assert.ok(bytes.length<2*1024*1024); }
    assert.equal(req.url,'/v1/chat/completions'); assert.equal(req.headers.authorization,'Bearer disposable-loopback-fixture');
    const input=JSON.parse(bytes); assert.equal(input.model,'controlled'); requests.push(input);
    const last=input.messages.at(-1); const text=typeof last.content==='string'?last.content:JSON.stringify(last.content);
    if(last.role==='user' && (text.includes('hold-child')||text.includes('hold-top-level'))) { res.writeHead(200,{'Content-Type':'text/event-stream'}); res.write(': owned child waiting\n\n'); return; }
    let tool;
    if(last.role==='user' && text.includes('ask-qna')) tool={name:'ask_user',arguments:JSON.stringify({question:'Hidden native question?',options:[{label:'Yes',description:'local only'}],allowCustom:false})};
    if(last.role==='user' && text.includes('escape-local-bash')) tool={name:'bash',arguments:JSON.stringify({command:`nohup sh -c 'trap \"\" TERM; echo $$ > ${box.root}/escaped.pid; sleep 120' </dev/null >/dev/null 2>&1 &`})};
    if(last.role==='user' && text.includes('spawn-child')) tool={name:'subagent',arguments:JSON.stringify({agent:'gui-worker',task:'hold-child',async:true})};
    if(last.role==='user' && text.includes('spawn-nested')) tool={name:'subagent',arguments:JSON.stringify({agent:'gui-nested',task:'make-leaf',async:true})};
    if(last.role==='user' && text.includes('make-leaf')) tool={name:'subagent',arguments:JSON.stringify({agent:'gui-worker',task:'hold-child',async:true})};
    if(last.role==='user' && text.includes('fetch-web'))tool={name:'fetch_content',arguments:JSON.stringify({url:`http://127.0.0.1:${server.address().port}/page`,mode:'raw'})};
    if(last.role==='user' && text.includes('search-web')) tool={name:'web_search',arguments:JSON.stringify({queries:['local evidence'],includeContent:false,workflow:'none'})};
    res.writeHead(200,{'Content-Type':'text/event-stream'});
    const delta=tool?{role:'assistant',tool_calls:[{index:0,id:`call-${requests.length}`,type:'function',function:tool}]}:{role:'assistant',content:'Production launcher proof'};
    res.write(`data: ${JSON.stringify({id:'fake',object:'chat.completion.chunk',choices:[{index:0,delta,finish_reason:null}]})}\n\n`);
    res.write(`data: ${JSON.stringify({id:'fake',object:'chat.completion.chunk',choices:[{index:0,delta:{},finish_reason:tool?'tool_calls':'stop'}],usage:{prompt_tokens:1,completion_tokens:1,total_tokens:2}})}\n\n`); res.end('data: [DONE]\n\n');
  });
  server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const base=`http://127.0.0.1:${server.address().port}`;
  await writeFile(join(box.root,'agent/models.json'),JSON.stringify({providers:{'gui-loopback':{baseUrl:`${base}/v1`,api:'openai-completions',apiKey:'disposable-loopback-fixture',models:[{id:'controlled',name:'Disposable local API',reasoning:false,input:['text','image'],contextWindow:200000,maxTokens:4096,cost:{input:0,output:0,cacheRead:0,cacheWrite:0}}]}}}));
  const settings=JSON.parse(await readFile(join(box.root,'agent/settings.json'),'utf8'));
  const subPkg=process.env.PI_GUI_SUBAGENTS_PACKAGE ?? join(process.env.HOME,'.pi/agent/npm/node_modules/pi-subagents');
  const webPkg=process.env.PI_GUI_WEB_PACKAGE ?? join(process.env.HOME,'.pi/agent/npm/node_modules/pi-web-access');
  if(subagents) {
    settings.packages=[subPkg]; await mkdir(join(box.root,'agent/extensions/subagent'),{recursive:true}); await mkdir(join(box.root,'agent/agents'));
    await writeFile(join(box.root,'agent/extensions/subagent/config.json'),JSON.stringify({toolActivation:'eager',defaultSubagentContext:'fresh',scheduledRuns:{enabled:false},missions:{enabled:false},artifactDir:'session',artifactConfig:{cleanupDays:0},disabledFeatures:subagentDisabled,maxSubagentDepth:2}));
    await writeFile(join(box.root,'agent/agents/gui-nested.md'),'---\nname: gui-nested\ndescription: Local nested child\nmodel: gui-loopback/controlled\ntools: subagent\nextensions: \n---\nExecute the task.\n');
    await writeFile(join(box.root,'agent/agents/gui-worker.md'),'---\nname: gui-worker\ndescription: Local child\nmodel: gui-loopback/controlled\ntools: \nextensions: \n---\nExecute the task.\n');
  }
  if(web) {
    settings.packages=[...(settings.packages??[]),webPkg];
    await writeFile(join(box.root,'agent/web-search.json'),JSON.stringify({toolActivation:'eager',workflow:'none',autoOpenBrowser:false,allowBrowserCookies:false,provider:'searxng',webSearch:{allowedProviders:['searxng']},searxngBaseUrl:base,ssrf:{allowRanges:['127.0.0.1/32'],trustEnvProxy:false},fetch:{defaultMode:'raw',allowedModes:['raw','readable']},fetchRouting:{providers:['http'],allowRemoteHostedProviders:false},githubClone:{enabled:false,clonePath:join(box.root,'clones')},commands:{websearch:{enabled:false},curator:{enabled:false},'google-account':{enabled:false}}}));
  }
  await writeFile(join(box.root,'agent/settings.json'),JSON.stringify(settings));
  const probes=[];
  async function start(label,session,frontend=false) {
    const scope=join(box.root,`owner-${label}`); const sessionDir=join(box.root,`sessions-${label}`); await mkdir(scope); await mkdir(sessionDir);
    const request=join(scope,'launch.json'); await writeFile(request,JSON.stringify({piRoot:await piPackage(),cwd:box.cwd,agentDir:join(box.root,'agent'),sessionDir,scope,session}));
    const binary=join(repository,'target/debug/rpgui');const args=['--rpgui-retained-node',join(repository,'scripts/pi-runtime/launch.mjs'),request];
    const probe=new Probe(spawn(frontend?process.execPath:binary,frontend?[join(import.meta.dirname,'frontend.mjs'),binary,...args]:args,{cwd:box.cwd,env:{...box.env,PI_GUI_SUBAGENTS_PACKAGE:subPkg,PI_GUI_WEB_PACKAGE:webPkg},stdio:['pipe','pipe','pipe']})); probes.push(probe);
    await probe.wait(r=>r.type==='rpgui_owner_ready'); const state=await probe.command('get_state'); assert.equal(state.model.provider,'gui-loopback'); assert.equal(state.model.baseUrl,`${base}/v1`); return {probe,scope};
  }
  async function cleanup() { for(const p of probes) await p.stop(); for(const s of sockets)s.destroy(); await new Promise(resolve=>server.close(resolve)); await box.cleanup(); }
  return {box,requests,start,cleanup};
}

 test('production launcher: concurrent threads, hidden Q+A, ordinary settings, queues and passive history', {timeout:45000}, async()=>{
  const fixture=await setup(); try {
    const {probe:a}=await fixture.start('a'); const {probe:b}=await fixture.start('b');
    const before=await readFile(join(fixture.box.root,'agent/settings.json'),'utf8');
    const waitA=a.wait(r=>r.type==='agent_settled'); await a.command('prompt',{message:'hello'}); await waitA;
    const from=b.records.length; await b.command('prompt',{message:'ask-qna'}); const dialog=await b.wait(r=>r.type==='extension_ui_request'&&r.method==='select',from);
    assert.ok(!a.records.some(r=>r.id===dialog.id)); await b.send({type:'extension_ui_response',id:dialog.id,value:dialog.options[0]}); await b.wait(r=>r.type==='agent_settled',from);
    const session=await a.command('get_state'); await a.stop(); const {probe:reopened}=await fixture.start('reopen',session.sessionFile);
    const messages=await reopened.command('get_messages'); assert.ok(JSON.stringify(messages).includes('Production launcher proof')); assert.ok(!reopened.records.some(r=>r.type==='agent_start'));
    assert.equal(await readFile(join(fixture.box.root,'agent/settings.json'),'utf8'),before);
  } finally { await fixture.cleanup(); }
});

test('production admitted subagents: structured observation and retained scoped shutdown', {timeout:60000}, async()=>{
  const fixture=await setup({subagents:true}); try {
    const {probe,scope}=await fixture.start('children'); const from=probe.records.length; await probe.command('prompt',{message:'spawn-child'});
    await probe.wait(r=>r.type==='tool_execution_end'&&r.toolName==='subagent',from);
    await probe.command('prompt',{message:'/rpgui-subagents '+JSON.stringify({version:1,requestId:'inspect-1',method:'status'})});
    const status=await probe.wait(r=>r.type==='extension_ui_request'&&r.method==='notify'&&r.message.includes('inspect-1'),from); const parsed=JSON.parse(status.message); assert.equal(parsed.bridge,'rpgui-subagents');assert.ok(status.message.includes('gui-worker'),'real child role is present in public projection');assert.ok(parsed.data.data.asyncSnapshot.caps);assert.ok(parsed.data.data.asyncSnapshot.omitted);assert.ok(Number.isInteger(parsed.observedAt)); assert.ok(!status.message.includes('intercomTarget'));
    const deadline=Date.now()+10000; while(!fixture.requests.some(request=>JSON.stringify(request.messages.at(-1)).includes('hold-child'))) { assert.ok(Date.now()<deadline,'child actual API dispatch'); await new Promise(resolve=>setTimeout(resolve,20)); }
    await probe.stop(); const proof=JSON.parse(await readFile(join(scope,'guardian-proof.json'),'utf8')); assert.equal(proof.parent.signal,null); assert.equal(proof.scope.uncertain,false, JSON.stringify(proof)); assert.equal(proof.scope.wholeRunProof,'package-observed');
  } finally { await fixture.cleanup(); }
});

test('production admitted web core output persists and passively reopens', {timeout:45000}, async()=>{
  const fixture=await setup({web:true}); try {
    const {probe}=await fixture.start('web'); const from=probe.records.length; await probe.command('prompt',{message:'search-web'}); await probe.wait(r=>r.type==='agent_settled',from);
    const outcome=probe.records.slice(from).find(r=>r.type==='tool_execution_end'&&r.toolName==='web_search'); assert.equal(outcome.isError,false);
    const fetchFrom=probe.records.length;await probe.command('prompt',{message:'fetch-web'});await probe.wait(r=>r.type==='agent_settled',fetchFrom);const fetched=probe.records.slice(fetchFrom).find(r=>r.type==='tool_execution_end'&&r.toolName==='fetch_content');assert.equal(fetched.isError,false,JSON.stringify(fetched));assert.ok(JSON.stringify(fetched).includes('Persisted local evidence'));
    const state=await probe.command('get_state'); await probe.stop(); const {probe:reopened}=await fixture.start('web-reopen',state.sessionFile);
    const entries=await reopened.command('get_entries'); assert.ok(JSON.stringify(entries).includes('web-search-results'));assert.ok(JSON.stringify(entries).includes('Persisted local evidence')); assert.ok(!reopened.records.some(r=>r.type==='agent_start'));
  } finally {await fixture.cleanup();}
});


test('production guardian survives frontend SIGKILL and stops separately detached nested work', {timeout:60000},async()=>{
 const fixture=await setup({subagents:true});try{
  const {probe,scope}=await fixture.start('nested-crash',undefined,true);await probe.command('prompt',{message:'spawn-nested'});
  const deadline=Date.now()+15000;while(!fixture.requests.some(request=>JSON.stringify(request.messages.at(-1)).includes('hold-child'))){assert.ok(Date.now()<deadline,'nested leaf actual dispatch');await new Promise(resolve=>setTimeout(resolve,20));}
  probe.child.kill('SIGKILL');await probe.exit;
  let proof;const stopDeadline=Date.now()+30000;while(!proof){try{proof=JSON.parse(await readFile(join(scope,'guardian-proof.json'),'utf8'));}catch{}assert.ok(Date.now()<stopDeadline,'retained proof after frontend death');if(!proof)await new Promise(resolve=>setTimeout(resolve,30));}
  assert.ok(proof.scope.runners.length>=2);assert.ok(proof.scope.runners.every(run=>run.terminal));assert.equal(proof.parent.signal,null);
  // Missing whole-run writer proof remains uncertainty rather than being upgraded from PID death.
  assert.equal(proof.uncertain,Boolean(proof.scope.uncertain));
 }finally{await fixture.cleanup();}
});


test('production target eight ordinary threads: busy explicit queues, clearing, abort and distinct session identities', {timeout:60000},async()=>{
 const fixture=await setup();try{
  const launched=await Promise.all(Array.from({length:8},(_,i)=>fixture.start(`parallel-${i}`)));const probes=launched.map(item=>item.probe);
  await Promise.all(probes.map(probe=>probe.command('prompt',{message:'hold-top-level'})));
  const deadline=Date.now()+10000;while(fixture.requests.length<8){assert.ok(Date.now()<deadline);await new Promise(resolve=>setTimeout(resolve,20));}
  await probes[0].command('prompt',{message:'steer only',streamingBehavior:'steer'});await probes[0].command('prompt',{message:'follow-up only',streamingBehavior:'followUp'});
  assert.equal((await probes[0].command('get_state')).pendingMessageCount,2);
  const states=await Promise.all(probes.map(probe=>probe.command('get_state')));assert.equal(new Set(states.map(state=>state.sessionId)).size,8);assert.ok(states.every(state=>state.isStreaming));
  await Promise.all(probes.map(async probe=>{await probe.command('clear_queue');await probe.command('abort');}));assert.equal((await probes[0].command('get_state')).pendingMessageCount,0);assert.equal(fixture.requests.length,8);
 }finally{await fixture.cleanup();}
});

 test('production retained Linux scope stops daemonized ordinary Bash descendants without PID/name search',{timeout:45000},async()=>{
 const fixture=await setup();try{
 const {probe,scope}=await fixture.start('escaped-bash');const from=probe.records.length;await probe.command('prompt',{message:'escape-local-bash'});await probe.wait(r=>r.type==='agent_settled',from);
 const pid=Number((await readFile(join(fixture.box.root,'escaped.pid'),'utf8')).trim());assert.ok(pid>1);const statBefore=await readFile(`/proc/${pid}/stat`,'utf8');assert.ok(!statBefore.split(') ').at(-1).startsWith('Z '));
 await probe.stop();const proof=JSON.parse(await readFile(join(scope,'guardian-proof.json'),'utf8'));assert.equal(proof.descendants.uncertain,false,JSON.stringify(proof));assert.ok(proof.descendants.observedProcesses>=1);
 try{const stat=await readFile(`/proc/${pid}/stat`,'utf8');assert.ok(stat.split(') ').at(-1).startsWith('Z '),'escaped owned process reached kernel terminal state');}catch(error){if(error.code!=='ENOENT')throw error;}
 }finally{await fixture.cleanup();}
 });
test('production stop still closes owned Pi when durable phase writing fails',{timeout:45000},async()=>{
 const fixture=await setup();try{const{probe,scope}=await fixture.start('journal-failure');await probe.command('prompt',{message:'hold-top-level'});await mkdir(join(scope,'guardian-owner.json.next'));await probe.stop();const proof=JSON.parse(await readFile(join(scope,'guardian-proof.json'),'utf8'));assert.equal(proof.parent.code,0);assert.equal(proof.descendants.uncertain,false);assert.equal(proof.uncertain,true,'persistence failure cannot be upgraded to successful ownership proof');}finally{await fixture.cleanup();}
});
