// Exercise the real store without a browser or a Tauri database.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import vm from 'node:vm'
import ts from 'typescript'

const require = createRequire(import.meta.url)
const source = readFileSync(new URL('../src/renderer/src/store/project.ts', import.meta.url), 'utf8')
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText
function store(save) {
  const exports = {}
  vm.runInNewContext(js, {
    exports, require: (name) => name === '../lib/format' ? { errorText: (e) => e.message } : require(name),
    window: { api: { projects: { save } } }, setTimeout, clearTimeout, crypto: globalThis.crypto
  })
  const s = exports.useProject
  s.getState().load({ project: { id: 'fixture', editor: {}, updatedAt: 0 }, characters: [], clips: [], assets: [], jobs: [] })
  return s
}
const tick = () => new Promise((resolve) => setImmediate(resolve))
const pending = []
const snapshots = []
const s = store((snapshot) => {
  snapshots.push(snapshot)
  return new Promise((resolve) => pending.push(resolve))
})
s.getState().updateEditor({ captionColor: '#123456' })
const flushing = s.getState().flush()
assert.equal(snapshots.length, 1)
s.getState().updateEditor({ captionScale: 1.4 })
pending.shift()({ updatedAt: 1 })
await tick()
assert.equal(snapshots.length, 2)
s.getState().updateEditor({ overlays: [{ kind: 'text', text: 'Newest' }] })
pending.shift()({ updatedAt: 2 })
await tick()
assert.equal(snapshots.length, 3)
pending.shift()({ updatedAt: 3 })
await flushing
assert.equal(s.getState().saveState, 'saved')
assert.equal(snapshots[2].project.editor.captionColor, '#123456')
assert.equal(snapshots[2].project.editor.captionScale, 1.4)
assert.equal(snapshots[2].project.editor.overlays[0].text, 'Newest')
s.getState().unload()

let attempts = 0
const failed = store(async () => { attempts++; throw new Error('disk full') })
failed.getState().updateEditor({ captionScale: 1.2 })
let exportStarted = false
await assert.rejects(async () => {
  await failed.getState().flush()
  exportStarted = true
}, /disk full/)
assert.equal(exportStarted, false)
assert.equal(attempts, 2) // initial save, then one retry (never an endless loop)
failed.getState().unload()

let recovered = false
const retry = store(async () => {
  if (!recovered) throw new Error('temporary')
  return { updatedAt: 4 }
})
retry.getState().updateEditor({ captionScale: 1.6 })
await retry.getState().save()
assert.equal(retry.getState().saveState, 'error')
recovered = true
await retry.getState().flush()
assert.equal(retry.getState().saveState, 'saved')
retry.getState().unload()
console.log('PASS: pending edits drained, save failure blocks export, failed save can recover')
