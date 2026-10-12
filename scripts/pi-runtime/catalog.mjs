// Explicit audited entry/build and configuration contracts. Unknown selections are refused, never disabled.
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { assertRuntimeBuild } from './build-lock.mjs';
export const subagentDisabled = ['agent-management', 'watchdog', 'panes', 'missions', 'lane-management', 'spawn-budget-grants', 'extension-bindings', 'external-machines', 'workflow-scripts', 'gates'];
async function config(path) { try { return JSON.parse(await readFile(path, 'utf8')); } catch (e) { if (e.code === 'ENOENT') return {}; throw e; } }
export function validateSubagents(value) {
  const allowed = ['toolActivation', 'defaultSubagentContext', 'scheduledRuns', 'artifactDir', 'maxSubagentDepth', 'disabledFeatures', 'artifactConfig', 'worktree', 'missions', 'intercomBridge', 'timeoutMs', 'maxActiveAsyncRunsPerSession', 'maxSubagentSpawnsPerRun'];
  if (Object.keys(value).some(key => !allowed.includes(key))) throw new Error('Unsupported subagents configuration owner/workflow');
  if (value.scheduledRuns?.enabled !== false || value.artifactDir !== 'session' || value.defaultSubagentContext !== 'fresh' || value.worktree === true || value.missions?.enabled !== false || value.artifactConfig?.cleanupDays !== 0 || !subagentDisabled.every(key => value.disabledFeatures?.includes(key))) throw new Error('Subagents requires explicit local-only, fresh, schedules/missions/cleanup disabled configuration; unsupported selections are not changed');
}
export function validateWeb(value) {
  const nested=(name,allowed)=>{if(!value[name] || Object.keys(value[name]).some(key=>!allowed.includes(key)))throw new Error(`Unaudited web ${name} configuration`);};
  nested('fetchRouting',['providers','allowRemoteHostedProviders']);nested('fetch',['defaultMode','allowedModes']);nested('githubClone',['enabled','clonePath']);nested('webSearch',['allowedProviders']);
  if(!Array.isArray(value.fetch.allowedModes)||!value.fetch.allowedModes.length||!Array.isArray(value.fetchRouting.providers)||!value.fetchRouting.providers.length||value.fetchRouting.allowRemoteHostedProviders!==false)throw new Error('Explicit audited HTTP routes and extraction modes are required');

  if (value.workflow !== 'none' || value.autoOpenBrowser !== false || value.allowBrowserCookies !== false || value.githubClone?.enabled !== false || value.fetchRouting?.providers?.some(route => route !== 'http') || !['raw', 'readable'].includes(value.fetch?.defaultMode) || value.fetch?.allowedModes?.some(mode => !['raw', 'readable'].includes(mode)) || ['websearch','curator','google-account'].some(name => value.commands?.[name]?.enabled !== false)) throw new Error('Unsupported web browser/cookie/curator/clone/extraction workflow. Select core HTTP workflows explicitly in ordinary configuration; GUI does not change these selections');
  if (!Array.isArray(value.webSearch?.allowedProviders) || !value.webSearch.allowedProviders.length || value.webSearch.allowedProviders.some(name => !['searxng'].includes(name)) || value.provider !== 'searxng') throw new Error('Only audited SearXNG core search is currently admitted');
  const allowed = ['toolActivation','workflow','autoOpenBrowser','allowBrowserCookies','provider','webSearch','searxngBaseUrl','ssrf','fetch','fetchRouting','githubClone','commands'];
  if (Object.keys(value).some(key => !allowed.includes(key))) throw new Error('Unaudited web configuration selection');
}
export async function reviewedCatalog({ agentDir, repository, subagentsPackage, webPackage }) {
  const replacement = join(repository, 'extensions/pi-gui-qna.ts');
  const guard = join(repository, 'extensions/pi-gui-guard.ts');
  const reviews = [
    { path: replacement, contract:'qna-replacement', toolNames:['ask_user'], sha256:'d7f171d86ee4c31201edb5be2c825b5146b9dae0e48e540450ef4e931d975229' },
    { path:join(agentDir,'extensions/qna-ui.ts'), contract:'qna-original', toolNames:['ask_user'], sha256:'aa78ef5954b1f0b2fd7e6daa42548163d2562c221988c635992ce53ca30a655e' },
    { path:'builtin:tool-search', contract:'no-background-work' },
  ];
  // Guard source is project-owned, bound to the release-local catalog fingerprint.
  const guardHash = JSON.parse(await readFile(new URL('./extension-lock.json', import.meta.url),'utf8')).guard;
  reviews.push({ path:guard, contract:'no-background-work', sha256:guardHash });
  // Missing attestations refuse selected resources in admission; cached unselected builds are not owners.
  if (subagentsPackage) {try{
    const manifest = await config(join(subagentsPackage,'package.json'));
    if (manifest.name !== 'pi-subagents' || manifest.version !== '0.76.1') throw new Error('Subagents version drift');
    await assertRuntimeBuild(subagentsPackage,'pi-subagents');
    reviews.push({ path:join(subagentsPackage,'index.js'), contract:'private-scope-stop', packageName:manifest.name, packageVersion:manifest.version, sha256:'3b6bd20de05a9c4aa92ba79c6a69bdbfccc3254362a75ecb0bed668e9cb27007' });
  }catch{/* Selected unknown/drifted builds have no admission review. */}}
  if (webPackage) {try{
    const manifest = await config(join(webPackage,'package.json'));
    if (manifest.name !== 'pi-web-access' || manifest.version !== '0.37.0') throw new Error('Web version drift');
    await assertRuntimeBuild(webPackage,'pi-web-access');
    for (const path of [join(webPackage,'dist'),join(webPackage,'dist/index.js')]) reviews.push({ path, contract:'private-scope-stop', packageName:manifest.name, packageVersion:manifest.version, sha256:'42e4354f6f3b5fa8db8228e96b81e7c314d974aafebdfde437eff8873502bb0e' });
  }catch{/* Selected unknown/drifted builds have no admission review. */}}
  return { reviews, replacement, guard };
}
export async function validateSelected({ extensions, agentDir, subagentsPackage, webPackage }) {
  if (extensions.some(path => path === join(subagentsPackage ?? '', 'index.js'))) validateSubagents(await config(join(agentDir,'extensions/subagent/config.json')));
  if (webPackage && extensions.some(path => path === join(webPackage,'dist') || path === join(webPackage,'dist/index.js'))) validateWeb(await config(join(agentDir,'web-search.json')));
}
