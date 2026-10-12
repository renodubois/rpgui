import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validateSubagents, validateWeb, subagentDisabled } from '../catalog.mjs';

test('production subagent admission is explicit local ownership, no silent disabled selection', () => {
  const config = {defaultSubagentContext:'fresh', scheduledRuns:{enabled:false}, missions:{enabled:false}, artifactDir:'session', artifactConfig:{cleanupDays:0}, disabledFeatures:subagentDisabled};
  assert.doesNotThrow(() => validateSubagents(config));
  for (const patch of [{scheduledRuns:{enabled:true}}, {worktree:true}, {worktreeSetupHook:'unsafe'}, {artifactConfig:{cleanupDays:7}}, {disabledFeatures:[]}]) assert.throws(() => validateSubagents({...config,...patch}));
});
test('production web admission refuses unsupported workflows and preserves input', () => {
  const config = {workflow:'none',autoOpenBrowser:false,allowBrowserCookies:false,provider:'searxng',webSearch:{allowedProviders:['searxng']},fetch:{defaultMode:'raw',allowedModes:['raw','readable']},fetchRouting:{providers:['http'],allowRemoteHostedProviders:false},githubClone:{enabled:false},commands:{websearch:{enabled:false},curator:{enabled:false},'google-account':{enabled:false}}};
  const original = JSON.stringify(config);
  assert.doesNotThrow(() => validateWeb(config));
  for (const patch of [{workflow:'curator'},{allowBrowserCookies:true},{fetchRouting:{providers:['browser'],allowRemoteHostedProviders:false}},{fetchRouting:{default:['http'],overrides:{example:['browser']}}},{fetch:{defaultMode:'raw',allowedModes:['raw'],browserFallback:true}},{provider:'google'},{githubClone:{enabled:true}}]) assert.throws(() => validateWeb({...config,...patch}));
  assert.equal(JSON.stringify(config),original);
});
test('unselected cached package drift is not a startup owner and receives no attestation',async()=>{
 const{mkdtemp,mkdir,writeFile,rm}=await import('node:fs/promises');const{tmpdir}=await import('node:os');const{join,resolve}=await import('node:path');const{reviewedCatalog}=await import('../catalog.mjs');const root=await mkdtemp(join(tmpdir(),'rpgui-catalog-'));
 try{const pkg=join(root,'cached');await mkdir(pkg);await writeFile(join(pkg,'package.json'),JSON.stringify({name:'pi-subagents',version:'future-unreviewed'}));const{reviews}=await reviewedCatalog({agentDir:root,repository:resolve('.'),subagentsPackage:pkg});assert.ok(!reviews.some(review=>review.packageName==='pi-subagents'));}finally{await rm(root,{recursive:true,force:true});}
});
