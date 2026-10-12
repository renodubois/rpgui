import type { ExtensionAPI, ExtensionContext } from '@earendil-works/pi-coding-agent';
import { join } from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';

/** Native ownership guard, inherited through the package's public required-child-extension contract. */
export default async function guard(pi: ExtensionAPI) {
  const pkg = process.env.RPGUI_SUBAGENTS_PACKAGE;
  const allowedExtensions: string[] = JSON.parse(process.env.RPGUI_ADMITTED_EXTENSIONS ?? '[]');
  let context: ExtensionContext | undefined;
  let registration: { dispose(): void } | undefined;
  const subscriptions: (() => void)[] = [];
  const publish=(kind:string,data:any)=>{let body=JSON.stringify({bridge:'rpgui-subagents',version:1,observedAt:Date.now(),kind,data});if(Buffer.byteLength(body)>32768) body=JSON.stringify({bridge:'rpgui-subagents',version:1,observedAt:Date.now(),kind,data:{requestId:data?.requestId,success:data?.success,data:{guiOmitted:true,reason:'Presentation exceeded 32 KiB; no ownership proof inferred.'}}});context?.ui.notify(body,'info');};
  const text = (value: unknown) => typeof value === 'string' ? value.slice(0, 8192) : undefined;
  const project = (value: any, depth = 0): any => {
    if (!value || typeof value !== 'object' || depth > 4) return undefined;
    const result: any = {};
    // Deliberately omit route capabilities, intercom targets, inboxes and private control metadata.
    for (const key of ['id','key','kind','label','role','effort','goal','runId','parentRunId','agent','model','thinking','state','depth','index','sessionFile','sessionId','currentTool','lastActivityAt','startedAt','endedAt','error','reason','text','totalTokens','totalCost','totalActive','totalRecent','version','generatedAt','updatedAt','currentToolStartedAt','turnCount','toolCount','input','output','total','window','windowPeak','used','limit','omitted','truncated']) {
      const item = value[key];
      if (typeof item === 'string') result[key] = text(item);
      if (typeof item === 'number' || typeof item === 'boolean') result[key] = item;
    }
    for (const key of ['runs','nested','steps','entries','results','children']) if (Array.isArray(value[key])) { result[key] = value[key].slice(0,128).map((item: any) => project(item,depth+1)); if (value[key].length > 128) result.omitted = true; }
    if (value.capabilities?.fleetStatus?.version === 1) result.capabilities={fleetStatus:{version:1}};
    for (const group of ['caps','omitted']) {if(value[group] && typeof value[group]==='object'){const numbers:any={};for(const key of ['maxRuns','maxChildrenPerNode','maxDepth','maxStringLength','maxSerializedBytes','runs','children','byteLimitExceeded']){if(typeof value[group][key]==='number'||typeof value[group][key]==='boolean')numbers[key]=value[group][key];}result[group]=numbers;}}
    for (const key of ['fleet','asyncSnapshot','activity','tokens','topLevelAsyncCapacity']) if (value[key]) result[key] = project(value[key],depth+1);
    return result;
  };
  let required: any; let preflight: any;
  if (pkg) {
    required = await import(pathToFileURL(join(pkg,'src/api/required-child-extensions.js')).href);
    preflight = await import(pathToFileURL(join(pkg,'src/api/preflight.js')).href);
    for (const event of ['subagent:async-started','subagent:async-complete','subagent:process-terminal','subagent:child-status','subagent:foreground-complete']) subscriptions.push(pi.events.on(event, (data) => publish(event,project(data))));
  }
  pi.on('session_start', (_event, ctx) => {
    context = ctx;
    if (required) {
      registration?.dispose();
      registration = required.registerRequiredChildExtensions({ sessionId:ctx.sessionManager.getSessionId(), requireForAllRunners:true, extensions:[{ id:'rpgui-ownership-guard', path:fileURLToPath(import.meta.url) }] });
    }
  });
  pi.on('tool_call', async (event, ctx) => {try{
    if (event.toolName === 'subagent') {
      const input: any = event.input;
      const safe = ['agent','task','tasks','chain','async','context','model','cwd','action','id','view','lines','timeoutMs','maxRuntimeMs','output','outputMode'];
      if (Object.keys(input).some(key => !safe.includes(key)) || (input.action && !['list','guide','status','stop','cost'].includes(input.action))) return { block:true, reason:'GUI refuses unaudited subagent workflow/owner options (external, schedules, scripts, panes, host gates, management and Git cleanup).' };
      if(input.context && input.context!=='fresh')return {block:true,reason:'GUI child admission supports fresh contexts only; branching and inherited context are not admitted.'};
      const requests = input.tasks ?? input.chain ?? (input.agent ? [input] : []);
      for (const request of requests) {
        if(Object.keys(request).some(key=>!safe.includes(key)) || (request.context && request.context!=='fresh'))return {block:true,reason:'Unaudited per-child workflow override refused.'};
        if (!request.agent || typeof request.task !== 'string') return { block:true, reason:'GUI requires an explicit named ordinary Pi child and task.' };
        const resolved = await preflight?.resolveSubagentLaunchContract({ agent:request.agent, cwd:request.cwd ?? input.cwd ?? ctx.cwd, task:request.task, context:'fresh', parentSessionId:ctx.sessionManager.getSessionId(), parentSessionFile:ctx.sessionManager.getSessionFile() });
        if (!resolved?.ok) return { block:true, reason:resolved?.message ?? 'Subagent ownership preflight unavailable' };
        const contract = resolved.contract;
        if (!contract.tools.disableAmbientExtensions || [...contract.tools.configuredExtensions,...contract.tools.runtimeExtensions].some((path: string) => !allowedExtensions.includes(path) && !(pkg && path.startsWith(join(pkg,'src')+'/'))) || contract.tools.effectiveMcpTools.length) return { block:true, reason:'Child requests unaudited extensions or MCP ownership; execution refused, not silently filtered.' };
      }
    }
    if(event.toolName==='web_search'&&(Object.keys(event.input).some(key=>!['queries','provider','numResults','includeContent','workflow'].includes(key))||((event.input as any).provider && (event.input as any).provider!=='searxng')))return {block:true,reason:'Unaudited web search override refused; only core SearXNG is admitted.'};
    if(event.toolName==='fetch_content'&&Object.keys(event.input).some(key=>!['url','urls','mode'].includes(key)))return {block:true,reason:'Unaudited fetch override refused (clone, authenticated/proxy/model and media helpers are unsupported).'};
    if (event.toolName === 'web_search' && (event.input as any).workflow && (event.input as any).workflow !== 'none') return { block:true, reason:'Browser/terminal curator workflow is unsupported in the native GUI.' };
    if (event.toolName === 'fetch_content' && (event.input as any).mode && !['raw','readable'].includes((event.input as any).mode)) return { block:true, reason:'Only audited raw/readable HTTP extraction is admitted.' };
  }catch{return {block:true,reason:'Ownership preflight failed; tool execution refused.'};}
  });
  pi.registerCommand('rpgui-subagents', {
    handler:async (args, ctx) => {
      context = ctx;
      const request = JSON.parse(args);
      if (!pkg || request.version !== 1 || typeof request.requestId !== 'string' || request.requestId.length > 128 || !['ping','status','stop'].includes(request.method)) throw new Error('Unsupported native subagent request');
      if (request.params && (Object.keys(request.params).some(key => !['id','view','lines'].includes(key)) || (request.params.lines && request.params.lines > 100))) throw new Error('Invalid native subagent projection request');
      await new Promise<void>((resolve, reject) => {
        const event = `subagents:rpc:v1:reply:${request.requestId}`;
        const timer = setTimeout(() => { off(); reject(new Error('Subagent inspection deadline')); },10000);
        const off = pi.events.on(event,(reply: any) => { clearTimeout(timer); off(); publish('reply',{ requestId:request.requestId, success:reply.success, data:project(reply.data), error:text(reply.error?.message) }); resolve(); });
        pi.events.emit('subagents:rpc:v1:request',request);
      });
    },
  });
  pi.on('session_shutdown', () => { registration?.dispose(); for (const dispose of subscriptions) dispose(); context = undefined; });
}
