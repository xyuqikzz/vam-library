import assert from 'node:assert/strict'
import { after, test } from 'node:test'
import { JSDOM } from 'jsdom'
import { webcrypto } from 'node:crypto'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
for (const key of ['window','document','Element','HTMLElement','SVGElement','Node']) globalThis[key] = dom.window[key]
Object.defineProperty(window,'crypto',{value:webcrypto})
globalThis.requestAnimationFrame = callback => setTimeout(callback,0)
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
const { createApp, h, nextTick } = await import('vue')
const { createPinia, defineStore, disposePinia } = await import('pinia')
const events = new Map()
globalThis.__dedupEvents = events
globalThis.__dedupThumbs = []
const load=componentLoader({
  'vue-router': moduleUrl('export const useRoute=()=>({path:"/deduplication"});export const useRouter=()=>({push(){}})'),
  '@/stores/app': moduleUrl('export const useAppStore=()=>globalThis.__dedupApp'),
  '@/stores/localLibrary': moduleUrl('export const useLocalLibraryStore=()=>globalThis.__dedupLibrary'),
  '@/composables/useNotification': moduleUrl('export const useNotification=()=>({success(){},error(){}})'),
  '@tauri-apps/api/event': moduleUrl('export async function listen(name,callback){globalThis.__dedupEvents.set(name,callback);return ()=>globalThis.__dedupEvents.delete(name)}'),
  '@tauri-apps/plugin-dialog': moduleUrl('export async function open(){return null}'),
  'vue-i18n': moduleUrl('export const useI18n=()=>({t:(key,args)=>key+(args?JSON.stringify(args):"" )})'),
  // Thumbnails and the shared details modal are covered elsewhere; record what the view asks for.
  '@/utils/resourceCache': moduleUrl('export async function loadPackageThumbnails(items,apply){globalThis.__dedupThumbs.push(...items.map(i=>i.id));for(const i of items)apply(i.id,"thumb://"+i.id)}'),
  '@/components/packages/PackageDetails.vue': moduleUrl(`import { h } from ${JSON.stringify(import.meta.resolve('vue'))}; export default { props: ['item'], setup(props) { return () => h('div', { id: 'package-details' }, props.item.id) } }`),
})
const {default: View}=await import(load('src/views/DeduplicationView.vue'))
const tick=async()=>{await nextTick();await new Promise(resolve=>setImmediate(resolve));await nextTick()}
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b});return {promise,resolve,reject}}
const button=text=>[...document.querySelectorAll('button')].find(b=>b.textContent.trim()===text)
const summary={scan_id:'scan',root:'/fixture',archive_count:0,warnings:[],duplicate_groups:0,total_wasted_bytes:0,safe_to_clean_count:0,total_files:2,scanned_files:2,mode:'standard',cache_hits:2,bytes_read:0,elapsed_ms:12}
async function mount(t,handler){
  const pinia=createPinia()
  globalThis.__dedupApp=defineStore('app',{state:()=>({vamRootPath:'/fixture',isScanning:false})})(pinia)
  globalThis.__dedupLibrary=defineStore('library',{state:()=>({revision:0,packages:[{id:'Author.Look.1',name:'Look',creator:'Author',resource_types:['appearance']}]}),actions:{async ensureLoaded(){}}})(pinia)
  const requests=[]
  mockIPC((command,args)=>{requests.push({command,args});if(command==='list_cleanup_trash')return [];return handler(command,args)})
  const app=createApp({render:()=>h(View)});app.config.globalProperties.$t=key=>key;app.use(pinia);app.mount('#app');await tick()
  t.after(()=>{app.unmount();disposePinia(pinia);clearMocks();events.clear();globalThis.__dedupThumbs.length=0})
  return {requests,appStore:globalThis.__dedupApp}
}
after(()=>{dom.window.close();for(const key of ['window','document','Element','HTMLElement','SVGElement','Node','requestAnimationFrame','getComputedStyle','__dedupApp','__dedupLibrary','__dedupEvents','__dedupThumbs'])delete globalThis[key]})

test('standard mode is default, deep mode starts a separate request without exposing engineering statistics',async t=>{
  const {requests}=await mount(t,(cmd)=>cmd==='scan_for_duplicates'?summary:[])
  button('deduplication.scanButton').click();await tick()
  assert.equal(requests.find(r=>r.command==='scan_for_duplicates').args.mode,'standard')
  assert.doesNotMatch(document.body.textContent,/deduplication.analysisStats|deduplication.introTitle/)
  assert.match(document.body.textContent,/deduplication.noDuplicatesScannedDesc/)
  button('deduplication.deepMode').click();await tick()
  assert.doesNotMatch(document.body.textContent,/deduplication.analysisStats/)
  button('deduplication.scanButton').click();await tick()
  const scans=requests.filter(r=>r.command==='scan_for_duplicates')
  assert.equal(scans[1].args.mode,'deep');assert.notEqual(scans[0].args.requestId,scans[1].args.requestId)
  assert.ok(requests.some(r=>r.command==='get_duplicate_groups'&&r.args.scanId==='scan'))
})
test('progress is request scoped and cancellation never publishes partial groups',async t=>{
  const work=deferred();let id
  await mount(t,(cmd,args)=>{if(cmd==='scan_for_duplicates'){id=args.requestId;return work.promise}if(cmd==='cancel_dedup_scan'){assert.equal(args.requestId,id);work.reject('DEDUP_CANCELLED');return}throw Error(cmd)})
  button('deduplication.scanButton').click();await tick()
  events.get('dedup-progress')({payload:{request_id:'old',stage:'content',current:1,total:2}});await tick()
  assert.doesNotMatch(document.body.textContent,/deduplication.stages.content/)
  events.get('dedup-progress')({payload:{request_id:id,stage:'hashing',current:1,total:2}});await tick()
  assert.match(document.body.textContent,/deduplication.stages.hashing/)
  button('deduplication.stopAnalysis').click();await tick()
  assert.match(document.body.textContent,/deduplication.cancelled/)
  assert.doesNotMatch(document.body.textContent,/deduplication.analysisStats/)
  assert.equal(events.size,0)
})
test('changing game folder discards late results and analysis errors stay readable',async t=>{
  const work=deferred();let second=false
  const {appStore}=await mount(t,(cmd)=>{if(cmd==='cancel_dedup_scan')return;if(cmd==='scan_for_duplicates')return second?Promise.reject('fixture read error'):work.promise;return []})
  button('deduplication.scanButton').click();await tick()
  appStore.vamRootPath='/other';await tick();work.resolve(summary);await tick()
  assert.doesNotMatch(document.body.textContent,/deduplication.analysisStats/)
  second=true;button('deduplication.scanButton').click();await tick()
  assert.match(document.body.textContent,/deduplication.analysisFailed/)
  assert.match(document.querySelector('details').textContent,/fixture read error/)
})

test('a stop after analysis finishes still discards groups awaiting delivery', async t => {
  const groups = deferred()
  await mount(t, command => {
    if (command === 'scan_for_duplicates') return summary
    if (command === 'get_duplicate_groups') return groups.promise
    if (command === 'cancel_dedup_scan') return
    throw Error(command)
  })
  button('deduplication.scanButton').click(); await tick()
  button('deduplication.stopAnalysis').click(); await tick()
  groups.resolve([]); await tick()
  assert.match(document.body.textContent, /deduplication.cancelled/)
  assert.doesNotMatch(document.body.textContent, /deduplication.analysisStats/)
})

test('the cleanup plan shows previews, opens resource details and can keep a file instead', async t => {
  const instance = (file_path, keep, extra = {}) => ({ package_id: 'Author.Look.1', file_path, size_bytes: 1024, is_recommended_keep: keep, source_type: 'var', link_type: null, modified_time: '2026-01-01T00:00:00Z', version: 1, referenced_by: [], archive_destination: null, ...extra })
  const groups = [{ id: 'g1', strategy: 'exact', resource_path: 'Author.Look.1', total_wasted_bytes: 2048, file_count: 3, instances: [
    instance('/fixture/a/Author.Look.1.var', true),
    instance('/fixture/b/Author.Look.1.var', false),
    instance('/fixture/c/Author.Look.1.var', false),
  ] }]
  const { requests } = await mount(t, command => command === 'scan_for_duplicates' ? { ...summary, duplicate_groups: 1 } : command === 'get_duplicate_groups' ? groups : [])
  button('deduplication.scanButton').click(); await tick()
  assert.ok(document.querySelector('.dup-file.keep') && document.querySelectorAll('.dup-file.recycle').length === 2)
  assert.deepEqual([...new Set(globalThis.__dedupThumbs)], ['Author.Look.1'])
  assert.match(document.querySelector('.dedup-planbar').textContent, /deduplication.planBarRecycle\{"count":2\}/)
  button('deduplication.reviewPlan').click(); await tick()
  const plan = document.querySelector('.ui-modal')
  assert.equal(plan.querySelectorAll('.plan-item img[data-resource-preview]').length, 2)
  plan.querySelector('.plan-item .dup-thumb').click(); await tick()
  assert.equal(document.querySelector('#package-details').textContent, 'Author.Look.1')
  ;[...plan.querySelectorAll('button')].find(b => b.textContent.trim() === 'deduplication.planKeepInstead').click(); await tick()
  assert.equal(plan.querySelectorAll('.plan-item').length, 1)
  ;[...plan.querySelectorAll('button')].find(b => b.textContent.trim().startsWith('deduplication.organizeCount')).click(); await tick()
  const cleanup = requests.find(r => r.command === 'execute_cleanup')
  assert.deepEqual(cleanup.args.filePaths, ['/fixture/c/Author.Look.1.var'])
})


const fixtureInstance = (id, path, keep, extra = {}) => ({ package_id: id, file_path: path, size_bytes: 1024, is_recommended_keep: keep, source_type: 'var', link_type: null, modified_time: '2026-10-13T00:00:00Z', version: 1, referenced_by: [], archive_destination: null, ...extra })
const fixtureGroup = (n, copies = 2, protectedOnly = false) => ({ id: `g${n}`, strategy: 'exact', resource_path: `Author.Look${n}`, total_wasted_bytes: (copies - 1) * 1024, file_count: copies, instances: Array.from({length:copies}, (_,i) => fixtureInstance(`Author.Look${n}.1`, `/fixture/${i}/Author.Look${n}.1.var`, protectedOnly || i === 0)) })

test('two-pane groups filter and search independently of keep choices', async t => {
  const groups = [fixtureGroup(1,3), fixtureGroup(2), fixtureGroup(3,2,true)]
  await mount(t, cmd => cmd === 'scan_for_duplicates' ? summary : groups)
  button('deduplication.scanButton').click(); await tick()
  assert.equal(document.querySelectorAll('.dedup-group-row').length, 2)
  assert.equal(document.querySelector('.comparison-title').textContent, 'Author.Look1')
  assert.equal(document.querySelectorAll('.dup-file').length, 3)
  const check = document.querySelectorAll('.comparison-file-actions input')[1]
  check.click(); await tick()
  assert.match(document.querySelector('.dedup-planbar').textContent, /"count":2/)
  assert.equal(document.querySelectorAll('.dedup-group-row').length, 2, 'keeping an extra copy must not make its group disappear')
  document.querySelectorAll('.dedup-group-row')[1].click(); await tick()
  assert.equal(document.querySelector('.comparison-title').textContent, 'Author.Look2')
  document.querySelector('.dedup-group-row').click(); await tick()
  assert.equal(document.querySelectorAll('.comparison-file-actions input')[1].checked, true)
  button('deduplication.retainedGroups1').click(); await tick()
  assert.equal(document.querySelector('.comparison-title').textContent, 'Author.Look3')
  assert.ok([...document.querySelectorAll('.comparison-file-actions input')].every(input=>input.disabled && input.checked))
  button('deduplication.cleanableGroups2').click(); await tick()
  const search = document.querySelector('.dedup-search input');search.value='missing';search.dispatchEvent(new window.Event('input',{bubbles:true}));await tick()
  assert.equal(document.querySelectorAll('.dup-file').length,0)
  assert.match(document.body.textContent,/deduplication.noMatchingGroups/)
  assert.match(document.querySelector('.dedup-planbar').textContent,/"count":2/)
})

test('large results paginate groups and progressively show every copy', async t => {
  const groups = Array.from({length:25},(_,i)=>fixtureGroup(i,i===0?30:2))
  await mount(t,cmd=>cmd==='scan_for_duplicates'?summary:groups)
  button('deduplication.scanButton').click(); await tick()
  assert.equal(document.querySelectorAll('.dedup-group-row').length,20)
  assert.equal(document.querySelectorAll('.dup-file').length,12)
  button('deduplication.planShowMore{"count":12}').click();await tick()
  assert.equal(document.querySelectorAll('.dup-file').length,24)
  button('deduplication.planShowMore{"count":6}').click();await tick()
  assert.equal(document.querySelectorAll('.dup-file').length,30)
  document.querySelector('[aria-label="deduplication.nextPage"]').click();await tick()
  assert.equal(document.querySelectorAll('.dedup-group-row').length,5)
  assert.equal(document.querySelector('.comparison-title').textContent,'Author.Look20')
  assert.equal(document.querySelectorAll('.dup-file').length,2)
})

test('cleanup progress belongs to its scan and is removed after completion', async t => {
  const work=deferred();let cleaned=false
  const {requests}=await mount(t,cmd=>{
    if(cmd==='scan_for_duplicates')return summary
    if(cmd==='get_duplicate_groups')return cleaned?[]:[fixtureGroup(1)]
    if(cmd==='execute_cleanup')return work.promise
    throw Error(cmd)
  })
  button('deduplication.scanButton').click();await tick()
  button('deduplication.reviewPlan').click();await tick()
  button('deduplication.organizeCount{"count":1}').click();await tick()
  const event=events.get('dedup-cleanup-progress');assert.ok(event)
  event({payload:{scan_id:'old',stage:'moving',current:1,total:2,file_name:'old.var'}});await tick()
  assert.doesNotMatch(document.querySelector('.ui-modal').textContent,/old.var/)
  event({payload:{scan_id:'scan',stage:'verifying',current:1,total:2,file_name:'Author.Look1.1.var'}});await tick()
  assert.match(document.querySelector('.cleanup-progress').textContent,/cleanupStages.verifying/)
  assert.match(document.querySelector('.cleanup-progress').textContent,/1 \/ 2/)
  assert.equal(document.querySelectorAll('.plan-item').length,0)
  assert.equal(requests.filter(r=>r.command==='execute_cleanup').length,1)
  cleaned=true;work.resolve({cleaned:1,archived:0,removed_directories:0,errors:[]});await tick();await tick()
  assert.equal(document.querySelector('.ui-modal'),null)
  assert.equal(events.has('dedup-cleanup-progress'),false)
  assert.match(document.body.textContent,/deduplication.noDuplicates/)
})

test('cleanup failure closes the consumed plan and shows a readable recovery message', async t => {
  await mount(t,cmd=>cmd==='scan_for_duplicates'?summary:cmd==='get_duplicate_groups'?[fixtureGroup(1)]:Promise.reject('fixture file changed'))
  button('deduplication.scanButton').click();await tick()
  button('deduplication.reviewPlan').click();await tick()
  button('deduplication.organizeCount{"count":1}').click();await tick()
  assert.equal(document.querySelector('.ui-modal'),null)
  assert.match(document.querySelector('[role="alert"]').textContent,/deduplication.cleanupFailed/)
  assert.match(document.querySelector('details').textContent,/fixture file changed/)
  assert.equal(events.has('dedup-cleanup-progress'),false)
  assert.ok(button('deduplication.scanButton'))
})
