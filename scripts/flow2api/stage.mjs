#!/usr/bin/env node
/**
 * stage.mjs — build the bundled Flow2API runtime for the De Story Tauri app.
 *
 * The Rust supervisor (`src-tauri/src/flow2api_runtime.rs`) resolves everything
 * relative to `<resource_dir>/flow2api` and expects:
 *
 *   flow2api/manifest.json          protocol/target/python/entrypoint/browser_executable/version
 *   flow2api/<manifest.python>      the bundled CPython interpreter
 *   flow2api/<manifest.entrypoint>  embedded_main.py
 *   flow2api/browsers/              PLAYWRIGHT_BROWSERS_PATH
 *   flow2api/                       the Flow2API source tree (src/, static/, config/)
 *
 * and launches:  python -I -u flow2api/embedded_main.py --data-dir <user data dir>/flow2api
 *
 * This script stages that tree in a temp dir and publishes it atomically.
 * Nothing is written to the Flow2API source checkout: it is read-only input.
 *
 * Usage
 * -----
 *   node scripts/flow2api/stage.mjs [options]
 *
 * Options
 *   --target <name>     win-x64 | mac-x64 | mac-arm64 | linux-x64
 *                       (default: detected from process.platform / process.arch)
 *   --allow-cross       Permit staging a target that does NOT match the host OS/arch.
 *                       Cross-built runtimes are NOT runnable on this machine and are
 *                       never a substitute for the real thing: a Windows-built runtime
 *                       must never be shipped as the macOS/Linux one. Without this
 *                       flag, a mismatched --target is refused outright.
 *   --source <dir>      Flow2API source checkout (read-only). Default: $FLOW2API_SOURCE
 *                       or the known `E:\Tools\flow2api` path on Windows.
 *   --out <dir>         Publish directory. Default: <repo>/src-tauri/flow2api.
 *                       Must be `src-tauri/flow2api` for the Tauri resource glob
 *                       `flow2api/**\/*` to land the tree at <resource_dir>/flow2api.
 *   --cache <dir>       Download cache (CPython tarball). Default: <repo>/.flow2api-cache
 *   --skip-browsers     Do NOT stage the Playwright browser runtime. The bundle will
 *                       then fail browser captcha at runtime; only use for CI smoke tests.
 *   --force             Re-stage even when the published tree already matches the
 *                       current fingerprint (source revision + pins + target).
 *   --help              This text.
 *
 * Exit codes: 0 ok, 1 any failure (including a download that cannot be verified).
 */

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..', '..');
// NOTE: the publish location is deliberately inside src-tauri/ and referenced
// from tauri.conf.json WITHOUT a `../` prefix. Tauri rewrites a leading `..` to
// `_up_` in the bundle, which would land the tree at
// <resource_dir>/_up_/flow2api instead of <resource_dir>/flow2api.
const DEFAULT_OUT = path.join(REPO_ROOT, 'src-tauri', 'flow2api');
const DEFAULT_CACHE = path.join(REPO_ROOT, '.flow2api-cache');

// ---------------------------------------------------------------------------
// Pinned provenance. These are the exact bytes that ship; do not use "latest".
// ---------------------------------------------------------------------------

/** python-build-standalone release tag (NOT `latest`). */
const PBS_RELEASE = '20261013';
/** CPython version inside that release. */
const CPYTHON_VERSION = '3.12.15';

/**
 * SHA256 of each `install_only` archive, as published by the
 * astral-sh/python-build-standalone release `20261013`.
 */
const TARGETS = {
  'win-x64': {
    triple: 'x86_64-pc-windows-msvc',
    sha256: 'bbc902d4d187ac4914915bf538196a064b49109a33a104965b5164ced5b80a1e',
    // interpreter path relative to flow2api/
    python: 'python/python.exe',
    // Playwright names the Windows Chromium directory chrome-win64 (64-bit).
    browserExecutable: 'browsers/chromium-*/chrome-win64/chrome.exe',
  },
  'mac-x64': {
    triple: 'x86_64-apple-darwin',
    sha256: '9686ddd8639b855905447ea4a16eb54a804a9ef12afbe7c9ecb1412d9c1fbc98',
    python: 'python/bin/python3',
    browserExecutable: 'browsers/chromium-*/chrome-mac/Chromium.app/Contents/MacOS/Chromium',
  },
  'mac-arm64': {
    triple: 'aarch64-apple-darwin',
    sha256: 'ba32036ad8f4c434d97a1ee7feef8b7edc0fac16777639b97e39c74a108e4440',
    python: 'python/bin/python3',
    browserExecutable: 'browsers/chromium-*/chrome-mac/Chromium.app/Contents/MacOS/Chromium',
  },
  'linux-x64': {
    triple: 'x86_64-unknown-linux-gnu',
    sha256: '5d546fc004fc2435fa142eb8f42ed8de5eb4725963dc22f7e6671014a80eb560',
    python: 'python/bin/python3',
    browserExecutable: 'browsers/chromium-*/chrome-linux/chrome',
  },
};

/**
 * `>=` ranges in Flow2API's requirements.txt are resolved to exact versions
 * here. Anything not listed is a build error, so an upstream change cannot
 * silently float into the bundle.
 */
const RANGE_PINS = {
  'curl-cffi': '0.13.0',
  httpx: '0.28.1',
  playwright: '1.64.0',
  nodriver: '0.50.6',
};

/**
 * Files/dirs copied out of the Flow2API checkout. This is an ALLOWLIST: anything
 * not reachable from here (venv/, .venv/, data/, tmp/, .claude/, docker*,
 * tests/, Dockerfile*, logs.txt, setting.toml, ...) is never copied.
 *
 * `extension/` ships so the optional Chrome extension worker can be loaded
 * unpacked straight out of the installed app ("Buka folder ekstensi" in
 * Settings). It contains no credentials — the key is provisioned per install
 * into the writable data dir.
 */
const APPROVED_PATHS = [
  'embedded_main.py',
  'main.py',
  'requirements.txt',
  'LICENSE',
  'README.md',
  'config/setting_example.toml',
  'src',
  'static',
  'extension',
];

/** Prune rules applied inside the approved paths. */
const PRUNE_DIR_NAMES = new Set(['__pycache__', 'tmp', 'data', 'logs']);
const PRUNE_FILE_SUFFIXES = ['.pyc', '.pyo'];
const PRUNE_FILE_NAMES = new Set(['.DS_Store', 'logs.txt', 'setting.toml']);

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

const log = (...args) => console.log('[flow2api:stage]', ...args);
const warn = (...args) => console.warn('[flow2api:stage] WARN', ...args);

function fail(message) {
  console.error(`\n[flow2api:stage] ERROR: ${message}\n`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: 'inherit', ...options });
  if (result.error) throw new Error(`${command} could not run: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} ${args.join(' ')} exited with ${result.status}`);
  return result;
}

function capture(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.error || result.status !== 0) return null;
  return (result.stdout || '').trim();
}

function sha256File(file) {
  const hash = createHash('sha256');
  hash.update(fs.readFileSync(file));
  return hash.digest('hex');
}

function pathSize(target) {
  let total = 0;
  let files = 0;
  const stack = [target];
  while (stack.length) {
    const current = stack.pop();
    let entries;
    try {
      entries = fs.readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const full = path.join(current, entry.name);
      if (entry.isSymbolicLink()) {
        files += 1;
        continue;
      }
      if (entry.isDirectory()) stack.push(full);
      else {
        files += 1;
        try {
          total += fs.statSync(full).size;
        } catch {
          /* ignore */
        }
      }
    }
  }
  return { bytes: total, files };
}

const mb = (bytes) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

function detectHostTarget() {
  const key = `${process.platform}-${process.arch}`;
  const map = {
    'win32-x64': 'win-x64',
    'darwin-x64': 'mac-x64',
    'darwin-arm64': 'mac-arm64',
    'linux-x64': 'linux-x64',
  };
  return map[key] || null;
}

function parseArgs(argv) {
  const options = {
    target: null,
    allowCross: false,
    source: null,
    out: DEFAULT_OUT,
    cache: DEFAULT_CACHE,
    skipBrowsers: false,
    force: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = () => {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) fail(`${arg} needs a value`);
      i += 1;
      return value;
    };
    switch (arg) {
      case '--target':
        options.target = next();
        break;
      case '--source':
        options.source = next();
        break;
      case '--out':
        options.out = path.resolve(next());
        break;
      case '--cache':
        options.cache = path.resolve(next());
        break;
      case '--allow-cross':
        options.allowCross = true;
        break;
      case '--skip-browsers':
        options.skipBrowsers = true;
        break;
      case '--force':
        options.force = true;
        break;
      case '--help':
      case '-h':
        printHelp();
        process.exit(0);
        break;
      default:
        fail(`unknown argument ${arg} (try --help)`);
    }
  }
  return options;
}

function printHelp() {
  const header = fs.readFileSync(fileURLToPath(import.meta.url), 'utf8');
  const doc = header.slice(header.indexOf('/**') + 3, header.indexOf('*/'));
  console.log(
    doc
      .split('\n')
      .map((line) => line.replace(/^\s?\* ?/, ''))
      .join('\n'),
  );
  console.log('\nAvailable targets:');
  for (const [name, spec] of Object.entries(TARGETS)) {
    console.log(`  ${name.padEnd(10)} ${spec.triple}`);
  }
  console.log(`\nHost target detected: ${detectHostTarget() || 'unsupported combination'}`);
  console.log(
    '\nCross-building: refused unless --allow-cross is passed. A runtime built for a\n' +
      'target other than the host cannot be executed or verified here, and must never\n' +
      'be published as a native runtime for another OS. Build each installer on its\n' +
      'own native OS.',
  );
}

// ---------------------------------------------------------------------------
// Download / verify / extract
// ---------------------------------------------------------------------------

const archiveName = (spec) =>
  `cpython-${CPYTHON_VERSION}+${PBS_RELEASE}-${spec.triple}-install_only.tar.gz`;

const archiveUrl = (spec) =>
  `https://github.com/astral-sh/python-build-standalone/releases/download/${PBS_RELEASE}/${encodeURIComponent(
    archiveName(spec),
  )}`;

async function download(url, destination) {
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) throw new Error(`download failed: HTTP ${response.status} for ${url}`);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  const temporary = `${destination}.part`;
  const buffer = Buffer.from(await response.arrayBuffer());
  fs.writeFileSync(temporary, buffer);
  fs.renameSync(temporary, destination);
  return buffer.length;
}

async function ensureInterpreterArchive(options, spec) {
  const cacheDir = path.join(options.cache, 'cpython');
  const archive = path.join(cacheDir, archiveName(spec));
  const url = archiveUrl(spec);

  if (fs.existsSync(archive)) {
    const digest = sha256File(archive);
    if (digest === spec.sha256) {
      log(`cached CPython archive verified: ${path.basename(archive)}`);
      return { archive, url, digest, downloaded: false };
    }
    warn(`cached archive has the wrong digest (${digest}); re-downloading`);
    fs.rmSync(archive, { force: true });
  }

  log(`downloading ${url}`);
  const size = await download(url, archive);
  log(`downloaded ${mb(size)}`);
  const digest = sha256File(archive);
  if (digest !== spec.sha256) {
    fs.rmSync(archive, { force: true });
    throw new Error(
      `SHA256 mismatch for ${path.basename(archive)}\n  expected ${spec.sha256}\n  actual   ${digest}`,
    );
  }
  log(`SHA256 verified: ${digest}`);
  return { archive, url, digest, downloaded: true };
}

function tarBinary() {
  if (process.platform === 'win32') {
    const systemTar = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe');
    if (fs.existsSync(systemTar)) return systemTar;
  }
  return 'tar';
}

function extractArchive(archive, destination) {
  fs.mkdirSync(destination, { recursive: true });
  log(`extracting into ${destination}`);
  run(tarBinary(), ['-xzf', archive, '-C', destination]);
}

/** Drop debug symbol files — pure build artifacts, never loaded at runtime. */
function pruneInterpreter(root) {
  const pruned = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.pdb')) {
        fs.rmSync(full, { force: true });
        pruned.push(full);
      }
    }
  };
  walk(root);
  return pruned.length;
}

// ---------------------------------------------------------------------------
// Approved source copy
// ---------------------------------------------------------------------------

function isPruned(entryName, isDirectory) {
  if (isDirectory) return PRUNE_DIR_NAMES.has(entryName);
  if (PRUNE_FILE_NAMES.has(entryName)) return true;
  return PRUNE_FILE_SUFFIXES.some((suffix) => entryName.endsWith(suffix));
}

function copyApproved(sourceRoot, destinationRoot) {
  const copied = [];
  const copyOne = (relative) => {
    const from = path.join(sourceRoot, relative);
    if (!fs.existsSync(from)) fail(`approved path missing from the Flow2API source: ${relative}`);
    const to = path.join(destinationRoot, relative);
    const stats = fs.statSync(from);
    if (stats.isDirectory()) {
      fs.mkdirSync(to, { recursive: true });
      for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
        if (isPruned(entry.name, entry.isDirectory())) continue;
        copyOne(path.join(relative, entry.name));
      }
    } else {
      fs.mkdirSync(path.dirname(to), { recursive: true });
      fs.copyFileSync(from, to);
      copied.push(relative.split(path.sep).join('/'));
    }
  };
  for (const relative of APPROVED_PATHS) copyOne(relative);
  return copied;
}

// ---------------------------------------------------------------------------
// Dependency pinning + install
// ---------------------------------------------------------------------------

function readPinnedRequirements(sourceRoot) {
  const file = path.join(sourceRoot, 'requirements.txt');
  const raw = fs.readFileSync(file, 'utf8');
  const lines = [];
  const pinned = [];
  for (const rawLine of raw.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    if (line.includes('==')) {
      lines.push(line);
      continue;
    }
    const name = line.split(/[<>=!~\s\[]/)[0].trim();
    const pin = RANGE_PINS[name];
    if (!pin) {
      fail(
        `requirements.txt line "${line}" is not pinned and has no entry in RANGE_PINS ` +
          `in stage.mjs. Add an explicit pin so the bundle stays reproducible.`,
      );
    }
    lines.push(`${name}==${pin}`);
    pinned.push({ name, requested: line, pinned: pin });
  }
  for (const [name, version] of Object.entries(RANGE_PINS)) {
    if (!lines.some((line) => line.split(/[<>=!~\s\[]/)[0] === name)) {
      lines.push(`${name}==${version}`);
      pinned.push({ name, requested: '(added by stage.mjs)', pinned: version });
    }
  }
  return { text: `${lines.join('\n')}\n`, pinned };
}

function installDependencies(pythonPath, requirementsFile) {
  log('installing pinned Python dependencies (this downloads from PyPI)');
  run(pythonPath, [
    '-I',
    '-m',
    'pip',
    'install',
    '--disable-pip-version-check',
    '--no-warn-script-location',
    '-r',
    requirementsFile,
  ]);
}

function freezeVersions(pythonPath) {
  const raw = capture(pythonPath, ['-I', '-m', 'pip', 'freeze']);
  if (!raw) return {};
  const resolved = {};
  for (const line of raw.split(/\r?\n/)) {
    const match = /^([^=<>!\s]+)==([^\s;]+)/.exec(line.trim());
    if (match) resolved[match[1]] = match[2];
  }
  return resolved;
}

// ---------------------------------------------------------------------------
// Browser runtime
// ---------------------------------------------------------------------------

/**
 * Resolve a bundled browser executable from a pattern like
 * `browsers/chromium-*&#47;chrome-win/chrome.exe`, where the `*` segment is the
 * version-stamped directory Playwright creates.
 */
function findBrowserExecutable(stageDir, pattern) {
  const segments = pattern.split('/');
  const wildcard = segments.findIndex((segment) => segment.includes('*'));
  if (wildcard === -1) fail(`browser executable pattern has no wildcard: ${pattern}`);
  const before = segments.slice(0, wildcard);
  const after = segments.slice(wildcard + 1);
  const prefix = segments[wildcard].replace('*', '');

  const parentDir = path.join(stageDir, ...before);
  if (!fs.existsSync(parentDir)) return null;
  const candidates = fs
    .readdirSync(parentDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.startsWith(prefix))
    .map((entry) => entry.name)
    .sort();
  for (const candidate of candidates.reverse()) {
    const relative = [...before, candidate, ...after].join('/');
    if (fs.existsSync(path.join(stageDir, ...relative.split('/')))) return relative;
  }
  return null;
}

function stageBrowsers(pythonPath, stageDir) {
  const browsersDir = path.join(stageDir, 'browsers');
  fs.mkdirSync(browsersDir, { recursive: true });
  log(`installing Playwright chromium into ${browsersDir}`);
  const result = spawnSync(pythonPath, ['-I', '-m', 'playwright', 'install', 'chromium'], {
    stdio: 'inherit',
    env: { ...process.env, PLAYWRIGHT_BROWSERS_PATH: browsersDir },
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      'Playwright browser staging failed. The bundle would have no browser runtime for ' +
        'browser captcha, so this is fatal (pass --skip-browsers only if you accept a ' +
        'bundle without browser captcha).',
    );
  }
  const executables = fs.readdirSync(browsersDir).filter((name) => name.startsWith('chromium-'));
  if (executables.length === 0) {
    throw new Error(`no chromium-* directory appeared under ${browsersDir} after install`);
  }
}

// ---------------------------------------------------------------------------
// Notices
// ---------------------------------------------------------------------------

function generateThirdPartyNotices(stageDir, sourceRoot, manifest) {
  const sitePackages = path.join(stageDir, 'python', 'Lib', 'site-packages');
  const lines = [];
  lines.push('De Story — bundled Flow2API runtime — third-party notices (original: Bang Story)');
  lines.push('='.repeat(72));
  lines.push('');
  lines.push('This bundle embeds the following third-party components.');
  lines.push('');
  lines.push('--- CPython interpreter ---');
  lines.push(`CPython ${CPYTHON_VERSION} (python-build-standalone release ${PBS_RELEASE})`);
  lines.push(`Source: ${archiveUrl(TARGETS[manifest.target])}`);
  lines.push('License: PSF-2.0 — full text: python/LICENSE.txt');
  lines.push('');
  lines.push('--- Flow2API ---');
  lines.push(`Source revision: ${manifest.version}`);
  lines.push('License: MIT — full text: LICENSE');
  lines.push('');
  lines.push('--- Bundled Python packages ---');
  if (!fs.existsSync(sitePackages)) {
    lines.push('(site-packages not found)');
  } else {
    const entries = fs
      .readdirSync(sitePackages)
      .filter((name) => name.endsWith('.dist-info'))
      .sort();
    for (const entry of entries) {
      const metadata = path.join(sitePackages, entry, 'METADATA');
      if (!fs.existsSync(metadata)) continue;
      const text = fs.readFileSync(metadata, 'utf8');
      const field = (key) => {
        const match = new RegExp(`^${key}: (.*)$`, 'm').exec(text);
        return match ? match[1].trim() : '';
      };
      const classifiers = [...text.matchAll(/^Classifier: License :: (.*)$/gm)].map((m) => m[1].trim());
      const license = field('License') || classifiers.join('; ') || 'see package metadata';
      lines.push(`${field('Name') || entry} ${field('Version')}`);
      lines.push(`  License: ${license}`);
      const homepage = field('Home-page') || field('Project-URL');
      if (homepage) lines.push(`  URL: ${homepage}`);
      lines.push(`  Metadata: python/Lib/site-packages/${entry}/METADATA`);
    }
  }
  lines.push('');
  lines.push('--- Playwright browser runtime ---');
  lines.push('Chromium, fetched by `python -m playwright install chromium` into browsers/.');
  lines.push('License: BSD-3-Clause (Chromium) and others — see browsers/*/LICENSE* .');
  lines.push('');
  const destination = path.join(stageDir, 'THIRD_PARTY_NOTICES.txt');
  fs.writeFileSync(destination, `${lines.join('\n')}\n`);
  return destination;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const options = parseArgs(process.argv.slice(2));

  const hostTarget = detectHostTarget();
  const target = options.target || hostTarget;
  if (!target) {
    fail(
      `cannot detect a supported target from ${process.platform}/${process.arch}. ` +
        `Supported: ${Object.keys(TARGETS).join(', ')}. Pass --target explicitly.`,
    );
  }
  if (!TARGETS[target]) {
    fail(`unsupported --target ${target}. Supported: ${Object.keys(TARGETS).join(', ')}.`);
  }
  if (!hostTarget) {
    fail(
      `${process.platform}/${process.arch} is not a supported host combination; ` +
        `refusing to guess. Supported hosts: win32-x64, darwin-x64, darwin-arm64, linux-x64.`,
    );
  }
  if (target !== hostTarget && !options.allowCross) {
    fail(
      `refusing to stage ${target} on a ${hostTarget} host.\n` +
        `  A runtime built here would not be runnable or verifiable on this machine, and a\n` +
        `  Windows-built runtime must never be presented as the macOS/Linux one. Build each\n` +
        `  installer on its own native OS, or pass --allow-cross if you really mean to\n` +
        `  produce (unverified) foreign-platform bytes.`,
    );
  }
  if (target !== hostTarget) {
    warn(`cross-staging ${target} on ${hostTarget}: the result CANNOT be executed here.`);
  }

  const repoFlowSrc = path.join(REPO_ROOT, 'flow2api-src');
  const sourceRoot = path.resolve(
    options.source ||
      process.env.FLOW2API_SOURCE ||
      (fs.existsSync(repoFlowSrc) ? repoFlowSrc : (process.platform === 'win32' ? 'E:\\Tools\\flow2api' : '')),
  );
  if (!sourceRoot || !fs.existsSync(path.join(sourceRoot, 'embedded_main.py'))) {
    fail(
      `Flow2API source not found at ${sourceRoot || '(unset)'}. ` +
        `Pass --source <dir> or set FLOW2API_SOURCE.`,
    );
  }
  if (!fs.existsSync(path.join(sourceRoot, 'requirements.txt'))) {
    fail(`${sourceRoot} does not look like a Flow2API checkout (no requirements.txt).`);
  }

  const spec = TARGETS[target];
  const outDir = path.resolve(options.out);
  const fingerprintPath = `${outDir}.stage-fingerprint.json`;

  log(`host=${hostTarget} target=${target} triple=${spec.triple}`);
  log(`source=${sourceRoot}`);
  log(`out=${outDir}`);

  // ---- fingerprint: skip an unchanged rebuild ------------------------------
  const { text: pinnedRequirements, pinned } = readPinnedRequirements(sourceRoot);
  const copiedFiles = [];
  {
    const collect = (relative) => {
      const from = path.join(sourceRoot, relative);
      const stats = fs.statSync(from);
      if (stats.isDirectory()) {
        for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
          if (isPruned(entry.name, entry.isDirectory())) continue;
          collect(path.join(relative, entry.name));
        }
      } else {
        copiedFiles.push(relative.split(path.sep).join('/'));
      }
    };
    for (const relative of APPROVED_PATHS) collect(relative);
    copiedFiles.sort();
  }
  const sourceRevision =
    capture('git', ['-C', sourceRoot, 'rev-parse', 'HEAD']) || 'unknown';

  const fingerprintHash = createHash('sha256');
  fingerprintHash.update(
    JSON.stringify({
      script: sha256File(fileURLToPath(import.meta.url)),
      target,
      pbsRelease: PBS_RELEASE,
      cpython: CPYTHON_VERSION,
      sha256: spec.sha256,
      requirements: pinnedRequirements,
      sourceRevision,
      browsers: !options.skipBrowsers,
      files: copiedFiles.map((file) => `${file}:${sha256File(path.join(sourceRoot, file))}`),
    }),
  );
  const fingerprint = fingerprintHash.digest('hex');

  if (!options.force && fs.existsSync(fingerprintPath) && fs.existsSync(path.join(outDir, 'manifest.json'))) {
    try {
      const previous = JSON.parse(fs.readFileSync(fingerprintPath, 'utf8'));
      if (previous.fingerprint === fingerprint) {
        log('published runtime already matches the current fingerprint; nothing to do (use --force)');
        return;
      }
    } catch {
      /* fall through and restage */
    }
  }

  // ---- stage in a temp dir, publish atomically -----------------------------
  const outParent = path.dirname(outDir);
  const outName = path.basename(outDir);
  fs.mkdirSync(outParent, { recursive: true });
  const stageDir = fs.mkdtempSync(path.join(outParent, `${outName}.staging-`));
  const publishedBackup = path.join(outParent, `${outName}.previous-${process.pid}`);

  try {
    // 1. interpreter
    const { archive, url } = await ensureInterpreterArchive(options, spec);
    extractArchive(archive, stageDir);
    const prunedCount = pruneInterpreter(stageDir);
    log(`pruned ${prunedCount} debug-symbol (.pdb) files`);
    const pythonPath = path.join(stageDir, spec.python);
    if (!fs.existsSync(pythonPath)) {
      fail(`the staged interpreter ${spec.python} is missing after extraction`);
    }
    if (target === hostTarget) {
      const version = capture(pythonPath, ['-I', '-c', 'import sys; print(sys.version.split()[0])']);
      log(`staged interpreter reports CPython ${version}`);
    }

    // 2. approved source
    const written = copyApproved(sourceRoot, stageDir);
    log(`copied ${written.length} approved source files`);

    // 3. dependencies
    const requirementsFile = path.join(stageDir, 'requirements.pinned.txt');
    fs.writeFileSync(requirementsFile, pinnedRequirements);
    installDependencies(pythonPath, requirementsFile);
    const resolved = freezeVersions(pythonPath);
    log(`resolved ${Object.keys(resolved).length} distributions`);

    // 4. browsers
    let browserExecutable = null;
    if (options.skipBrowsers) {
      warn('--skip-browsers: the bundle has NO browser runtime; browser captcha will fail.');
    } else {
      if (target !== hostTarget) {
        warn(
          'cross-staging browsers: `playwright install chromium` downloads host-native binaries ' +
            'from the toolchain shipped with the resolved wheel, which may not match the target.',
        );
      }
      stageBrowsers(pythonPath, stageDir);
      browserExecutable = findBrowserExecutable(stageDir, spec.browserExecutable);
      log(browserExecutable ? `browser executable: ${browserExecutable}` : 'browser executable not located (browser_executable omitted)');
    }

    // 5. manifest + provenance + notices
    const manifest = {
      protocol: 1,
      target,
      python: spec.python,
      entrypoint: 'embedded_main.py',
      version: sourceRevision.length === 40 ? sourceRevision.slice(0, 12) : sourceRevision,
    };
    if (browserExecutable) manifest.browser_executable = browserExecutable;
    fs.writeFileSync(path.join(stageDir, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);

    const provenance = {
      staged_at: new Date().toISOString(),
      host: { platform: process.platform, arch: process.arch, release: os.release(), node: process.version },
      target,
      target_triple: spec.triple,
      cross_staged: target !== hostTarget,
      cpython: {
        version: CPYTHON_VERSION,
        release: PBS_RELEASE,
        archive: archiveName(spec),
        url,
        sha256: spec.sha256,
        flavor: 'install_only',
      },
      flow2api: { source: sourceRoot, revision: sourceRevision },
      dependencies: {
        pinned: Object.fromEntries(
          Object.entries(RANGE_PINS).map(([name, version]) => [name, version]),
        ),
        range_overrides: pinned,
        resolved,
      },
      browsers: options.skipBrowsers
        ? null
        : { provider: 'playwright', command: 'playwright install chromium', browsers_path: 'browsers/' },
      pruning: ['**/*.pdb'],
    };
    fs.writeFileSync(path.join(stageDir, 'provenance.json'), `${JSON.stringify(provenance, null, 2)}\n`);
    generateThirdPartyNotices(stageDir, sourceRoot, manifest);

    // 6. publish
    if (fs.existsSync(publishedBackup)) fs.rmSync(publishedBackup, { recursive: true, force: true });
    const hadPrevious = fs.existsSync(outDir);
    if (hadPrevious) fs.renameSync(outDir, publishedBackup);
    try {
      fs.renameSync(stageDir, outDir);
    } catch (error) {
      if (hadPrevious && !fs.existsSync(outDir)) fs.renameSync(publishedBackup, outDir);
      throw error;
    }
    if (hadPrevious) fs.rmSync(publishedBackup, { recursive: true, force: true });

    const revisionForFingerprint =
      capture('git', ['-C', sourceRoot, 'rev-parse', '--short', 'HEAD']) || 'unknown';
    fs.writeFileSync(
      fingerprintPath,
      `${JSON.stringify({ fingerprint, target, sourceRevision: revisionForFingerprint, stagedAt: new Date().toISOString() }, null, 2)}\n`,
    );

    const size = pathSize(outDir);
    log(`published ${outDir} (${size.files} files, ${mb(size.bytes)})`);
    log(`manifest: ${JSON.stringify(manifest)}`);
  } catch (error) {
    fs.rmSync(stageDir, { recursive: true, force: true });
    throw error;
  }
}

main().catch((error) => {
  fail(error && error.stack ? error.stack : String(error));
});

