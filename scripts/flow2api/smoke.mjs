// Smoke test for a staged Flow2API runtime. Launches the staged interpreter
// exactly as src-tauri/src/flow2api_runtime.rs does and walks the full
// handshake: stdin launch token -> BANG_FLOW_READY -> authenticated
// /_bang/ready probe -> 401 without auth -> shutdown command -> exit 0.
//
//   node scripts/flow2api/smoke.mjs [--runtime <dir>]
//
// Default runtime is src-tauri/flow2api. The data dir is a fresh temp dir,
// wiped on exit.
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..', '..');

function argValue(flag) {
  const index = process.argv.indexOf(flag);
  return index !== -1 ? process.argv[index + 1] : undefined;
}

const runtime = path.resolve(argValue('--runtime') ?? path.join(REPO_ROOT, 'src-tauri', 'flow2api'));
const manifest = JSON.parse(fs.readFileSync(path.join(runtime, 'manifest.json'), 'utf8'));
const python = path.join(runtime, manifest.python);
const entrypoint = path.join(runtime, manifest.entrypoint);
if (!fs.existsSync(python)) throw new Error(`missing interpreter: ${python}`);
if (!fs.existsSync(entrypoint)) throw new Error(`missing entrypoint: ${entrypoint}`);

const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'flow2api-smoke-'));
const token = crypto.randomUUID();
const instance = crypto.randomUUID();

const env = {
  ...process.env,
  FLOW2API_DATA_DIR: dataDir,
  FLOW2API_EMBEDDED: '1',
  PYTHONUNBUFFERED: '1',
  PYTHONIOENCODING: 'utf-8',
  PLAYWRIGHT_BROWSERS_PATH: path.join(runtime, 'browsers'),
};
if (manifest.browser_executable) {
  const browser = path.join(runtime, manifest.browser_executable);
  if (fs.existsSync(browser)) env.BROWSER_EXECUTABLE_PATH = browser;
}

const child = spawn(python, ['-I', '-u', entrypoint, '--data-dir', dataDir], {
  cwd: dataDir,
  env,
  stdio: ['pipe', 'pipe', 'pipe'],
});

const stderrLines = [];
child.stderr.on('data', (chunk) => {
  for (const line of chunk.toString().split('\n')) {
    if (line.trim()) stderrLines.push(line.slice(0, 500));
  }
});

let settled = false;
function finish(code, message) {
  if (settled) return;
  settled = true;
  const elapsed = ((Date.now() - startedAt) / 1000).toFixed(1);
  console.log(message);
  console.log(`exit code: ${code} (after ${elapsed}s)`);
  if (stderrLines.length) {
    console.log('--- last stderr lines ---');
    for (const line of stderrLines.slice(-8)) console.log(`  ${line}`);
  }
  try { fs.rmSync(dataDir, { recursive: true, force: true }); } catch {}
  process.exitCode = code;
}

const startedAt = Date.now();
let ready = null;

// Wait for the readiness line, then run the HTTP checks, then shut down.
const timeout = setTimeout(() => {
  child.kill();
  finish(1, 'FAIL: no BANG_FLOW_READY within 90s');
}, 90_000);

child.stdout.on('data', async (chunk) => {
  for (const line of chunk.toString().split('\n')) {
    const trimmed = line.trim();
    if (!trimmed.startsWith('BANG_FLOW_READY ')) continue;
    ready = JSON.parse(trimmed.slice('BANG_FLOW_READY '.length));
    clearTimeout(timeout);
    console.log(`ready line: BANG_FLOW_READY ${JSON.stringify(ready)}`);

    const base = `http://127.0.0.1:${ready.port}`;
    const checks = [];
    try {
      const unauth = await fetch(`${base}/_bang/ready`);
      checks.push(`unauthenticated GET /_bang/ready -> HTTP ${unauth.status} (expect 401)`);

      const authed = await fetch(`${base}/_bang/ready`, {
        headers: { authorization: `Bearer ${token}` },
      });
      const body = await authed.json();
      checks.push(
        `authenticated GET /_bang/ready -> HTTP ${authed.status} ` +
          `protocol=${body.protocol} instance=${body.instance} ready=${body.ready}`,
      );
      const ok =
        unauth.status === 401 &&
        authed.status === 200 &&
        body.protocol === 1 &&
        body.instance === instance &&
        ready.instance === instance &&
        ready.protocol === 1;
      if (!ok) {
        child.stdin.write('{"command":"shutdown"}\n');
        return finish(1, `FAIL:\n  ${checks.join('\n  ')}`);
      }
      console.log(checks.join('\n'));

      child.stdin.write('{"command":"shutdown"}\n');
      const code = await new Promise((resolve) => child.once('exit', resolve));
      finish(code === 0 ? 0 : 1, `shutdown command accepted, child exited with ${code}`);
    } catch (error) {
      child.stdin.write('{"command":"shutdown"}\n');
      finish(1, `FAIL: ${error}\n  ${checks.join('\n  ')}`);
    }
  }
});

child.once('exit', (code) => {
  if (!ready) {
    clearTimeout(timeout);
    finish(1, `FAIL: child exited (code ${code}) before reporting ready`);
  }
});

child.stdin.write(`${JSON.stringify({ token, instance })}\n`);
console.log(`launched: ${python} -I -u ${entrypoint} --data-dir ${dataDir}`);
console.log(`handshake sent: token=${token} instance=${instance}`);

